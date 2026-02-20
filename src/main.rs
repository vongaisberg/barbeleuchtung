mod artnet;
mod composition;
mod effect;
mod engine;
mod fixture;
mod fixtures;
mod scheduler;
mod state;
mod theme;
mod themes;
mod web;

use std::sync::{Arc, Mutex};

use actix_web::{web as aweb, App, HttpServer};
use tokio::sync::broadcast;

use scheduler::default_schedule;
use state::AppState;
use web::WebData;

const BIND_ADDR: &str = "0.0.0.0:8080";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let app_state = Arc::new(Mutex::new(AppState::new()));
    let (broadcast_tx, _) = broadcast::channel::<String>(64);

    // Start the DMX engine loop in a dedicated OS thread.
    {
        let state_clone = app_state.clone();
        std::thread::Builder::new()
            .name("dmx-engine".into())
            .spawn(move || {
                engine::run(state_clone, Some(default_schedule()));
            })
            .expect("failed to spawn DMX engine thread");
    }

    let web_data = aweb::Data::new(WebData {
        state: app_state,
        broadcast_tx,
    });

    log::info!("Starting web server on http://{BIND_ADDR}");

    HttpServer::new(move || {
        App::new()
            .app_data(web_data.clone())
            .route("/", aweb::get().to(web::index))
            .route("/static/style.css", aweb::get().to(web::style_css))
            .route("/static/app.js",    aweb::get().to(web::app_js))
            .route("/ws",               aweb::get().to(web::websocket))
    })
    .bind(BIND_ADDR)?
    .run()
    .await
}
