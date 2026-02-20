use std::sync::{Arc, Mutex};

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_ws::AggregatedMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::fixtures;
use crate::state::AppState;
use crate::theme::{all_themes, theme_names, Transition};

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
    faders: Vec<f32>,
    blackout: bool,
    theme_names: Vec<&'static str>,
    fader_labels: Vec<&'static str>,
}

impl StateSnapshot {
    fn from_state(state: &AppState) -> Self {
        // During a crossfade the engine still blends from active_theme_id, but
        // the UI should immediately highlight the destination so the button
        // reflects what the user just pressed.
        let theme = state
            .crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(state.active_theme_id);
        Self {
            msg_type: "state",
            theme,
            faders: state.fader_values.clone(),
            blackout: state.blackout,
            theme_names: theme_names(),
            fader_labels: fixtures::FADER_LABELS.to_vec(),
        }
    }
}

/// Client → server messages.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Theme { id: usize },
    Fader { id: usize, value: f32 },
    Blackout { active: bool },
}

// ---------------------------------------------------------------------------
// Shared web data
// ---------------------------------------------------------------------------

pub struct WebData {
    pub state: Arc<Mutex<AppState>>,
    pub broadcast_tx: broadcast::Sender<String>,
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
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Send current state immediately on connect.
    {
        let s = state.lock().unwrap();
        let snapshot = StateSnapshot::from_state(&s);
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
                            handle_client_message(&text, &state, &broadcast_tx);
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
        ClientMessage::Fader { id, value } => {
            if id < s.fader_values.len() {
                s.fader_values[id] = value.clamp(0.0, 1.0);
            }
        }
        ClientMessage::Blackout { active } => {
            s.blackout = active;
            log::info!("Blackout: {active}");
        }
    }

    // Push new state to all connected clients.
    let snapshot = StateSnapshot::from_state(&s);
    drop(s);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        let _ = broadcast_tx.send(json);
    }
}
