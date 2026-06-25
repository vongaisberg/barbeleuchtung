mod artnet;
mod audio;
mod composition;
mod effect;
mod engine;
mod fixture;
mod fixtures;
mod scheduler;
mod spotify;
mod state;
mod theme;
mod themes;
mod viz;
mod web;

use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex, RwLock};

use actix_web::{web as aweb, App, HttpServer};
use tokio::sync::broadcast;

use artnet::SubscriberTable;
use scheduler::complete_schedule;
use state::AppState;
use state::ReactiveLive;
use web::WebData;

const BIND_ADDR: &str = "0.0.0.0:8080";
const ARTNET_PORT: u16 = 6454;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let reactive_live = Arc::new(ReactiveLive::default());
    let (broadcast_tx, _) = broadcast::channel::<String>(64);

    // Lock-free handle the engine publishes each rendered frame into; the
    // `/visualization` endpoints read it back to colour the front view.
    let frame = viz::new_frame_handle();

    // Discovery uses port 6454 (Art-Net standard) for ArtPoll/ArtPollReply.
    // Engine uses a separate socket (ephemeral port) for ArtDmx only.
    // This avoids contention: discovery blocks in recv_from; engine sends at 40 Hz.
    let artnet_socket_discovery = std::net::UdpSocket::bind(("0.0.0.0", ARTNET_PORT))?;
    artnet_socket_discovery.set_broadcast(true)?;

    let artnet_socket_engine = std::net::UdpSocket::bind("0.0.0.0:0")?;
    artnet_socket_engine.set_broadcast(true)?;
    let subscribers = Arc::new(RwLock::new(SubscriberTable::new()));
    let _broadcast_addr = ("10.255.255.255", ARTNET_PORT)
        .to_socket_addrs()?
        .next()
        .expect("failed to resolve Art-Net broadcast address");

    // Start Art-Net discovery (ArtPoll / ArtPollReply) in a dedicated thread.
    {
     //   let subscribers_clone = subscribers.clone();
     //   std::thread::Builder::new()
     //       .name("artnet-discovery".into())
     //       .spawn(move || run_discovery(artnet_socket_discovery, subscribers_clone, broadcast_addr))
     //       .expect("failed to spawn Art-Net discovery thread");
    }

    // Start realtime audio analysis (loopback if built with `--features
    // loopback`, otherwise a silent handle until a source is wired).
    let audio_handle = init_audio();

    // Broadcast a compact audio-feature snapshot to the web UI (~20 Hz) so the
    // reactive panel can show a live BPM / beat / spectrum meter.
    {
        let audio_for_meter = audio_handle.clone();
        let live_for_meter = reactive_live.clone();
        let tx = broadcast_tx.clone();
        std::thread::Builder::new()
            .name("audio-meter".into())
            .spawn(move || loop {
                let f = audio_for_meter.load();
                let bands: Vec<f32> = f.bands.iter().map(|b| (b * 1000.0).round() / 1000.0).collect();
                let (auto, live_tier, live_palette) = live_for_meter.snapshot();
                let msg = serde_json::json!({
                    "type": "audio",
                    "bpm": f.bpm,
                    "beat_now": f.beat_now,
                    "beat_phase": f.beat_phase,
                    "confidence": f.beat_confidence,
                    "energy": f.energy,
                    "intensity": f.intensity,
                    "four_on_floor": f.four_on_floor,
                    "onset": f.onset,
                    "bands": bands,
                    "reactive_auto": auto,
                    "live_tier": live_tier,
                    "live_palette": live_palette,
                });
                let _ = tx.send(msg.to_string());
                std::thread::sleep(std::time::Duration::from_millis(50));
            })
            .expect("failed to spawn audio meter thread");
    }

    // (Auto mode is now the generative Director, run inside the engine tick —
    // no separate look-selector thread.)

    // Start the DMX engine loop in a dedicated OS thread.
    {
        let state_clone = app_state.clone();
        let audio_for_engine = audio_handle.clone();
        let frame_for_engine = frame.clone();
        let live_for_engine = reactive_live.clone();
        std::thread::Builder::new()
            .name("dmx-engine".into())
            .spawn(move || {
                engine::run(
                    state_clone,
                    Some(complete_schedule()),
                    artnet_socket_engine,
                    subscribers,
                    audio_for_engine,
                    frame_for_engine,
                    live_for_engine,
                );
            })
            .expect("failed to spawn DMX engine thread");
    }

    // Start the Spotify integration (no-op if not configured via env).
    let spotify = spotify::spawn(app_state.clone(), reactive_live.clone(), broadcast_tx.clone());

    let web_data = aweb::Data::new(WebData {
        state: app_state,
        broadcast_tx,
        spotify,
        frame,
        reactive_live,
    });

    log::info!("Starting web server on http://{BIND_ADDR}");

    HttpServer::new(move || {
        App::new()
            .app_data(web_data.clone())
            .route("/", aweb::get().to(web::index))
            .route("/style.css", aweb::get().to(web::style_css))
            .route("/app.js",    aweb::get().to(web::app_js))
            .route("/ws",               aweb::get().to(web::websocket))
            .route("/visualization",             aweb::get().to(viz::index))
            .route("/visualization/layout.json", aweb::get().to(viz::layout))
            .route("/visualization/frame.json",  aweb::get().to(viz::frame))
            .route("/spotify/login",    aweb::get().to(web::spotify_login))
            .route("/spotify/callback", aweb::get().to(web::spotify_callback))
    })
    .bind(BIND_ADDR)?
    .run()
    .await
}

/// Build the audio-reactive feature handle.
///
/// With `--features loopback` this opens system-audio loopback capture (dev
/// machines). Otherwise — and in the container until a network source is wired
/// — it returns a permanently-silent handle so the reactive bank renders dark.
fn init_audio() -> audio::FeatureHandle {
    // Preferred dev/production path: raw PCM piped to stdin (e.g. from
    // `parec`/`pw-record` tapping the sink monitor). cpal-free, never degrades
    // playback. Enabled by setting BB_PCM_STDIN=1.
    if std::env::var("BB_PCM_STDIN").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false) {
        use audio::AudioSource as _;
        let sample_rate = env_u32("BB_PCM_RATE", 44_100);
        let channels = env_u32("BB_PCM_CHANNELS", 2) as usize;
        let format = std::env::var("BB_PCM_FORMAT")
            .ok()
            .and_then(|s| audio::PcmFormat::from_str(&s))
            .unwrap_or(audio::PcmFormat::F32Le);
        let src = audio::StdinPcmSource { sample_rate, channels, format };
        let cfg = audio::AudioConfig { sample_rate: src.sample_rate(), ..Default::default() };
        log::info!("Audio-reactive: reading PCM from stdin @ {sample_rate} Hz, {channels} ch, {format:?}");
        return audio::spawn(cfg, Box::new(src));
    }

    #[cfg(feature = "loopback")]
    {
        use audio::AudioSource as _;
        match audio::loopback::LoopbackSource::new() {
            Ok(src) => {
                let cfg = audio::AudioConfig {
                    sample_rate: src.sample_rate(),
                    ..Default::default()
                };
                log::info!(
                    "Audio-reactive: loopback capture @ {} Hz",
                    cfg.sample_rate
                );
                return audio::spawn(cfg, Box::new(src));
            }
            Err(e) => {
                log::warn!(
                    "Audio-reactive: loopback unavailable ({e}); reactive bank will be silent"
                );
            }
        }
    }
    log::info!(
        "Audio-reactive: no live source; reactive bank silent \
         (set BB_PCM_STDIN=1 and pipe PCM in, or build with `--features loopback`)"
    );
    audio::new_handle()
}

/// Read an unsigned-int env var, falling back to `default` if unset/invalid.
fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

