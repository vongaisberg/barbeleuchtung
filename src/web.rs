use std::sync::{Arc, Mutex};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_ws::AggregatedMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::fixtures;
use crate::state::{AppState, ReactiveLive};
use crate::theme::{
    all_fx_themes, all_reactive_themes, all_themes, fx_theme_names, theme_names, Transition,
};

// ---------------------------------------------------------------------------
// Broadcast channel – lets the server push state updates to all connected
// WebSocket clients whenever the state changes.
// ---------------------------------------------------------------------------

/// Server → client state snapshot.
#[derive(Serialize, Clone, Debug)]
pub struct StateSnapshot {
    #[serde(rename = "type")]
    msg_type: &'static str,
    theme: usize,
    fx_theme: usize,
    reactive_theme: usize,
    faders: Vec<f32>,
    blackout: bool,
    theme_names: Vec<&'static str>,
    fx_theme_names: Vec<&'static str>,
    reactive_theme_names: Vec<&'static str>,
    reactive_auto: bool,
    reactive_master: f32,
    reactive_sensitivity: f32,
    reactive_strobe: f32,
    reactive_tier_lock: i8,
    reactive_palette_lock: i8,
    /// Generative tier/palette currently on screen (Auto mode).
    reactive_live_tier: u8,
    reactive_live_palette: u8,
    reactive_tier_names: Vec<&'static str>,
    reactive_palette_names: Vec<&'static str>,
    fader_labels: Vec<&'static str>,
    fog_enabled: bool,
    fog_interval_min: f32,
    fog_duration_s: f32,
    fog_level: f32,
    /// Patched universes + their current mute state, in `PATCHED_UNIVERSES`
    /// order.  The frontend renders one pill per entry in the status bar.
    universes: Vec<UniverseInfo>,
    /// "Artist – Title" Spotify reports as currently playing, or `null`.
    now_playing: Option<String>,
    /// Whether the Spotify integration is configured at all.
    spotify_available: bool,
    /// Whether Spotify OAuth has been completed (a refresh token exists).
    spotify_connected: bool,
    /// True while the Spotify sync currently owns the FX bank.
    spotify_controlling: bool,
}

/// One patched DMX universe as exposed to the frontend.
#[derive(Serialize, Clone, Debug)]
pub struct UniverseInfo {
    universe: u16,
    label: &'static str,
    muted: bool,
}

impl StateSnapshot {
    fn from_state(state: &AppState, live: &ReactiveLive) -> Self {
        let (_, live_tier, live_palette) = live.snapshot();
        // During a crossfade the engine still blends from active_theme_id, but
        // the UI should immediately highlight the destination so the button
        // reflects what the user just pressed.
        let theme = state
            .crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(state.active_theme_id);
        let fx_theme = state
            .fx_crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(state.active_fx_theme_id);
        let reactive_theme = state
            .reactive_crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(state.active_reactive_theme_id);
        let universes = fixtures::PATCHED_UNIVERSES
            .iter()
            .map(|&(universe, label)| UniverseInfo {
                universe,
                label,
                muted: state
                    .universe_muted
                    .get(universe as usize)
                    .copied()
                    .unwrap_or(false),
            })
            .collect();
        Self {
            msg_type: "state",
            theme,
            fx_theme,
            reactive_theme,
            faders: state.fader_values.clone(),
            blackout: state.blackout,
            theme_names: theme_names(),
            fx_theme_names: fx_theme_names(),
            reactive_theme_names: crate::theme::reactive_theme_names(),
            reactive_auto: state.reactive_auto,
            reactive_master: state.reactive_controls.master,
            reactive_sensitivity: state.reactive_controls.sensitivity,
            reactive_strobe: state.reactive_controls.strobe,
            reactive_tier_lock: state.reactive_controls.tier_lock,
            reactive_palette_lock: state.reactive_controls.palette_lock,
            reactive_live_tier: live_tier,
            reactive_live_palette: live_palette,
            reactive_tier_names: crate::themes::reactive::TIER_NAMES.to_vec(),
            reactive_palette_names: crate::themes::reactive::PALETTE_NAMES.to_vec(),
            fader_labels: fixtures::FADER_LABELS.to_vec(),
            fog_enabled: state.fog_enabled,
            fog_interval_min: state.fog_interval_min,
            fog_duration_s: state.fog_duration_s,
            fog_level: state.fog_level,
            universes,
            now_playing: state.now_playing.clone(),
            spotify_available: state.spotify_available,
            spotify_connected: state.spotify_connected,
            spotify_controlling: state.spotify_controlling,
        }
    }
}

/// Build a state snapshot and broadcast it to all connected WebSocket clients.
/// Used by background tasks (e.g. the Spotify sync) to push updates.
pub fn push_state(
    state: &AppState,
    live: &ReactiveLive,
    broadcast_tx: &broadcast::Sender<String>,
) {
    let snapshot = StateSnapshot::from_state(state, live);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = broadcast_tx.send(json);
    }
}

/// Client → server messages.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Theme { id: usize },
    FxTheme { id: usize },
    ReactiveTheme { id: usize },
    ReactiveAuto { active: bool },
    ReactiveControls { master: f32, sensitivity: f32, strobe: f32 },
    ReactiveTierLock { tier: i8 },
    ReactivePaletteLock { palette: i8 },
    Fader { id: usize, value: f32 },
    Blackout { active: bool },
    FogEnabled { active: bool },
    FogSettings { interval_min: f32, duration_s: f32, level: f32 },
    /// Mute or un-mute ArtDmx transmission for a single universe.
    UniverseOutput { universe: u16, muted: bool },
}

// ---------------------------------------------------------------------------
// Shared web data
// ---------------------------------------------------------------------------

pub struct WebData {
    pub state: Arc<Mutex<AppState>>,
    pub broadcast_tx: broadcast::Sender<String>,
    /// Spotify client for the OAuth login routes; `None` if not configured.
    pub spotify: Option<Arc<crate::spotify::SpotifyClient>>,
    /// Latest rendered DMX frame, published by the engine for the
    /// `/visualization` front view.
    pub frame: crate::viz::FrameHandle,
    /// Lock-free generative tier/palette readout for the reactive UI.
    pub reactive_live: Arc<ReactiveLive>,
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// Serve the single-page frontend application.
pub async fn index() -> impl Responder {
    let html = include_str!("../static/index.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Serve static CSS file.
pub async fn style_css() -> impl Responder {
    let css = include_str!("../static/style.css");
    HttpResponse::Ok()
        .content_type("text/css; charset=utf-8")
        .body(css)
}

/// Serve static JS file.
pub async fn app_js() -> impl Responder {
    let js = include_str!("../static/app.js");
    HttpResponse::Ok()
        .content_type("application/javascript; charset=utf-8")
        .body(js)
}

// ---------------------------------------------------------------------------
// Spotify OAuth routes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    error: Option<String>,
}

/// Redirect the operator to Spotify's consent screen.
pub async fn spotify_login(data: web::Data<WebData>) -> impl Responder {
    match &data.spotify {
        Some(client) => HttpResponse::Found()
            .insert_header(("Location", client.authorize_url()))
            .finish(),
        None => HttpResponse::ServiceUnavailable()
            .body("Spotify is not configured (set SPOTIFY_CLIENT_ID / SPOTIFY_CLIENT_SECRET)."),
    }
}

/// OAuth redirect target: exchange the code for tokens, then bounce home.
pub async fn spotify_callback(
    data: web::Data<WebData>,
    query: web::Query<CallbackQuery>,
) -> impl Responder {
    let Some(client) = &data.spotify else {
        return HttpResponse::ServiceUnavailable().body("Spotify is not configured.");
    };

    if let Some(err) = &query.error {
        return HttpResponse::BadRequest().body(format!("Spotify authorization denied: {err}"));
    }

    let Some(code) = &query.code else {
        return HttpResponse::BadRequest().body("Missing authorization code.");
    };

    match client.exchange_code(code).await {
        Ok(()) => {
            {
                let mut s = data.state.lock().unwrap();
                s.spotify_connected = true;
            }
            // Refresh all clients so the UI drops the "Connect" prompt.
            let s = data.state.lock().unwrap();
            push_state(&s, &data.reactive_live, &data.broadcast_tx);
            drop(s);
            // Bounce back to the app root. Relative "../" resolves against the
            // callback path (/spotify/callback) to "/" — and stays correct when
            // the app is reverse-proxied under a path prefix.
            HttpResponse::Found()
                .insert_header(("Location", "../"))
                .finish()
        }
        Err(e) => {
            log::warn!("Spotify code exchange failed: {e}");
            HttpResponse::BadGateway().body(format!("Spotify token exchange failed: {e}"))
        }
    }
}

/// WebSocket upgrade handler.
pub async fn websocket(
    req: HttpRequest,
    body: web::Payload,
    data: web::Data<WebData>,
) -> actix_web::Result<HttpResponse> {
    let (response, mut session, msg_stream) = actix_ws::handle(&req, body)?;
    let mut msg_stream = msg_stream.aggregate_continuations();

    let state = data.state.clone();
    let broadcast_tx = data.broadcast_tx.clone();
    let reactive_live = data.reactive_live.clone();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Send current state immediately on connect.
    {
        let s = state.lock().unwrap();
        let snapshot = StateSnapshot::from_state(&s, &reactive_live);
        let json = serde_json::to_string(&snapshot).unwrap();
        drop(s);
        let _ = session.text(json).await;
    }

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Incoming message from this client.
                msg = msg_stream.recv() => {
                    match msg {
                        Some(Ok(AggregatedMessage::Text(text))) => {
                            handle_client_message(&text, &state, &reactive_live, &broadcast_tx);
                        }
                        Some(Ok(AggregatedMessage::Ping(ping))) => {
                            let _ = session.pong(&ping).await;
                        }
                        Some(Ok(AggregatedMessage::Close(_))) | None => break,
                        _ => {}
                    }
                }
                // Broadcast from another connection or state update.
                Ok(msg) = broadcast_rx.recv() => {
                    if session.text(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = session.close(None).await;
    });

    Ok(response)
}

fn handle_client_message(
    text: &str,
    state: &Arc<Mutex<AppState>>,
    reactive_live: &ReactiveLive,
    broadcast_tx: &broadcast::Sender<String>,
) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Invalid WS message: {e}: {text}");
            return;
        }
    };

    let mut s = state.lock().unwrap();
    match msg {
        ClientMessage::Theme { id } => {
            let themes = all_themes();
            if id < themes.len() {
                let duration_ms = match themes[id].transition {
                    Transition::Crossfade { duration_ms } => Some(duration_ms),
                    Transition::Instant => None,
                };
                s.request_theme(id, duration_ms);
                log::info!("Theme changed to {id}");
            } else {
                log::warn!("Unknown theme id {id}");
            }
        }
        ClientMessage::FxTheme { id } => {
            let fx_themes = all_fx_themes();
            if id < fx_themes.len() {
                let duration_ms = match fx_themes[id].transition {
                    Transition::Crossfade { duration_ms } => Some(duration_ms),
                    Transition::Instant => None,
                };
                // A manual selection overrides any Spotify-driven sync until
                // the next poll re-evaluates the currently playing track.
                s.fx_show_clock = None;
                s.spotify_controlling = false;
                s.spotify_saved = None;
                s.request_fx_theme(id, duration_ms);
                log::info!("FX theme changed to {id}");
            } else {
                log::warn!("Unknown FX theme id {id}");
            }
        }
        ClientMessage::ReactiveTheme { id } => {
            let reactive_themes = all_reactive_themes();
            if id < reactive_themes.len() {
                let duration_ms = match reactive_themes[id].transition {
                    Transition::Crossfade { duration_ms } => Some(duration_ms),
                    Transition::Instant => None,
                };
                // A manual pick pins the operator's choice and leaves Auto.
                s.reactive_auto = false;
                s.request_reactive_theme(id, duration_ms);
                log::info!("Reactive theme changed to {id} (auto off)");
            } else {
                log::warn!("Unknown reactive theme id {id}");
            }
        }
        ClientMessage::ReactiveAuto { active } => {
            s.reactive_auto = active;
            log::info!("Reactive auto {}", if active { "on" } else { "off" });
        }
        ClientMessage::ReactiveControls { master, sensitivity, strobe } => {
            s.reactive_controls.master = master.clamp(0.0, 1.0);
            s.reactive_controls.sensitivity = sensitivity.clamp(0.3, 2.5);
            s.reactive_controls.strobe = strobe.clamp(0.0, 1.0);
        }
        ClientMessage::ReactiveTierLock { tier } => {
            s.reactive_controls.tier_lock = tier.clamp(-1, 3);
        }
        ClientMessage::ReactivePaletteLock { palette } => {
            s.reactive_controls.palette_lock = palette.clamp(-1, 3);
        }
        ClientMessage::Fader { id, value } => {
            if id < s.fader_values.len() {
                s.fader_values[id] = value.clamp(0.0, 1.0);
            }
        }
        ClientMessage::Blackout { active } => {
            s.blackout = active;
            log::info!("Blackout: {active}");
        }
        ClientMessage::FogEnabled { active } => {
            s.fog_enabled = active;
            log::info!("Fog enabled: {active}");
        }
        ClientMessage::FogSettings { interval_min, duration_s, level } => {
            s.fog_interval_min = interval_min.clamp(1.0, 30.0);
            s.fog_duration_s   = duration_s.clamp(5.0, 25.0);
            s.fog_level        = level.clamp(0.01, 0.30);
        }
        ClientMessage::UniverseOutput { universe, muted } => {
            let idx = universe as usize;
            if idx < s.universe_muted.len() {
                s.universe_muted[idx] = muted;
                log::info!(
                    "Universe {universe} output {}",
                    if muted { "muted" } else { "resumed" }
                );
            } else {
                log::warn!("Universe {universe} out of range, ignoring mute request");
            }
        }
    }

    // Push new state to all connected clients.
    let snapshot = StateSnapshot::from_state(&s, reactive_live);
    drop(s);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = broadcast_tx.send(json);
    }
}
