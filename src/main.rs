mod artnet;
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
mod web;

use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex, RwLock};

use actix_web::{web as aweb, App, HttpServer};
use tokio::sync::broadcast;

use artnet::SubscriberTable;
use scheduler::complete_schedule;
use state::AppState;
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
    let (broadcast_tx, _) = broadcast::channel::<String>(64);

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

    // Start the DMX engine loop in a dedicated OS thread.
    {
        let state_clone = app_state.clone();
        std::thread::Builder::new()
            .name("dmx-engine".into())
            .spawn(move || {
                engine::run(
                    state_clone,
                    Some(complete_schedule()),
                    artnet_socket_engine,
                    subscribers,
                );
            })
            .expect("failed to spawn DMX engine thread");
    }

    // Start the Spotify integration (no-op if not configured via env).
    let spotify = spotify::spawn(app_state.clone(), broadcast_tx.clone());

    let web_data = aweb::Data::new(WebData {
        state: app_state,
        broadcast_tx,
        spotify,
    });

    log::info!("Starting web server on http://{BIND_ADDR}");

    HttpServer::new(move || {
        App::new()
            .app_data(web_data.clone())
            .route("/", aweb::get().to(web::index))
            .route("/style.css", aweb::get().to(web::style_css))
            .route("/app.js",    aweb::get().to(web::app_js))
            .route("/ws",               aweb::get().to(web::websocket))
            .route("/spotify/login",    aweb::get().to(web::spotify_login))
            .route("/spotify/callback", aweb::get().to(web::spotify_callback))
    })
    .bind(BIND_ADDR)?
    .run()
    .await
}
