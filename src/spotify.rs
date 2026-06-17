//! Spotify Web API integration.
//!
//! Implements the OAuth 2.0 *Authorization Code* flow so the controller can
//! read the operator's **currently playing track** and lock the timecoded
//! "Would You" FX show to Spotify's `progress_ms`.
//!
//! ## Configuration (environment variables)
//! | Variable                 | Required | Default                                      |
//! |--------------------------|----------|----------------------------------------------|
//! | `SPOTIFY_CLIENT_ID`      | yes      | –                                            |
//! | `SPOTIFY_CLIENT_SECRET`  | yes      | –                                            |
//! | `APP_BASE_URL`           | no       | –  (public URL, e.g. `https://host/licht/`)  |
//! | `SPOTIFY_REDIRECT_URI`   | no       | `{APP_BASE_URL}/spotify/callback` else local |
//! | `SPOTIFY_TOKEN_FILE`     | no       | `spotify_token.txt`                          |
//! | `SPOTIFY_WOULD_YOU_MATCH`| no       | `would you`                                  |
//! | `SPOTIFY_PRADA_MATCH`    | no       | `prada`                                      |
//! | `SPOTIFY_DJ_TURN_IT_UP_MATCH`| no   | `turn it up`                                 |
//! | `SPOTIFY_SCHREI_NACH_LIEBE_MATCH`| no | `schrei nach liebe`                        |
//!
//! If the client id/secret are unset the integration silently disables itself
//! and the rest of the app runs unchanged.
//!
//! ## One-time login
//! Visit `/spotify/login`, approve the `user-read-currently-playing` scope, and
//! Spotify redirects back to `/spotify/callback`.  The refresh token is then
//! persisted to `SPOTIFY_TOKEN_FILE`, so subsequent restarts authenticate
//! automatically.
//!
//! ## Synchronisation
//! A background task polls `GET /v1/me/player/currently-playing` once a second.
//! When the matched song is playing it switches the FX bank to the "Would You"
//! show and installs a [`ShowClock`] anchored at `progress_ms`.  The engine
//! extrapolates that anchor at 40 Hz, so the show stays beat-locked between
//! polls; each poll re-anchors to correct drift.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::state::{AppState, ShowClock};
use crate::themes::{
    FX_DJ_TURN_IT_UP_ID, FX_OFF_ID, FX_PRADA_ID, FX_SCHREI_NACH_LIEBE_ID, FX_WOULD_YOU_ID,
};

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
const CURRENTLY_PLAYING_URL: &str = "https://api.spotify.com/v1/me/player/currently-playing";
const SCOPE: &str = "user-read-currently-playing";

/// How often we poll Spotify and re-anchor the show clock.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Fallback back-off when a 429 response omits a `Retry-After` header.
const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(5);

/// Extra margin added on top of `Retry-After` so we resume just *after* the
/// window, never a hair early (which would earn another 429).
const RETRY_MARGIN: Duration = Duration::from_millis(500);

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

pub struct Config {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    token_file: String,
    /// Song→show rules: when a playing track's lower-cased title contains the
    /// substring, the paired FX theme id is locked to playback.  Evaluated in
    /// order; the first match wins.
    matches: Vec<(String, usize)>,
}

impl Config {
    /// Build config from the environment, or `None` if Spotify is not set up.
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("SPOTIFY_CLIENT_ID").ok()?;
        let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET").ok()?;
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        // The OAuth redirect URI. An explicit SPOTIFY_REDIRECT_URI always wins;
        // otherwise it is derived from the public deployment URL (APP_BASE_URL),
        // falling back to the local dev address.
        let redirect_uri = match std::env::var("SPOTIFY_REDIRECT_URI") {
            Ok(uri) if !uri.is_empty() => uri,
            _ => match std::env::var("APP_BASE_URL") {
                Ok(base) if !base.is_empty() => {
                    format!("{}/spotify/callback", base.trim_end_matches('/'))
                }
                _ => "http://127.0.0.1:8080/spotify/callback".to_string(),
            },
        };
        let token_file =
            std::env::var("SPOTIFY_TOKEN_FILE").unwrap_or_else(|_| "spotify_token.txt".to_string());

        // Song→show rules. Each match string is configurable via env; an empty
        // value disables that rule.
        let mut matches = Vec::new();
        let would_you = std::env::var("SPOTIFY_WOULD_YOU_MATCH")
            .unwrap_or_else(|_| "would you".to_string())
            .to_lowercase();
        if !would_you.is_empty() {
            matches.push((would_you, FX_WOULD_YOU_ID));
        }
        let prada = std::env::var("SPOTIFY_PRADA_MATCH")
            .unwrap_or_else(|_| "prada".to_string())
            .to_lowercase();
        if !prada.is_empty() {
            matches.push((prada, FX_PRADA_ID));
        }
        let dj_turn_it_up = std::env::var("SPOTIFY_DJ_TURN_IT_UP_MATCH")
            .unwrap_or_else(|_| "turn it up".to_string())
            .to_lowercase();
        if !dj_turn_it_up.is_empty() {
            matches.push((dj_turn_it_up, FX_DJ_TURN_IT_UP_ID));
        }
        let schrei_nach_liebe = std::env::var("SPOTIFY_SCHREI_NACH_LIEBE_MATCH")
            .unwrap_or_else(|_| "schrei nach liebe".to_string())
            .to_lowercase();
        if !schrei_nach_liebe.is_empty() {
            matches.push((schrei_nach_liebe, FX_SCHREI_NACH_LIEBE_ID));
        }

        Some(Self {
            client_id,
            client_secret,
            redirect_uri,
            token_file,
            matches,
        })
    }
}

// ---------------------------------------------------------------------------
// Token cache
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Tokens {
    access_token: Option<String>,
    expires_at: Option<Instant>,
    refresh_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct SpotifyClient {
    cfg: Config,
    http: reqwest::Client,
    tokens: Mutex<Tokens>,
}

/// One sampled `currently-playing` response together with the (latency-
/// corrected) local instant the position is assumed to be true.
struct Sample {
    data: Option<CurrentlyPlaying>,
    sampled_at: Instant,
}

/// Why a `currently-playing` fetch failed.
enum FetchError {
    /// HTTP 429 – includes the `Retry-After` delay Spotify asked us to wait.
    RateLimited(Option<Duration>),
    /// Any other failure (network, auth, decode, unexpected status).
    Other(String),
}

impl SpotifyClient {
    pub fn new(cfg: Config) -> Self {
        // Load a previously-persisted refresh token if present.
        let refresh_token = std::fs::read_to_string(&cfg.token_file)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        Self {
            http: reqwest::Client::new(),
            cfg,
            tokens: Mutex::new(Tokens {
                refresh_token,
                ..Default::default()
            }),
        }
    }

    /// True once we have a refresh token (login completed at least once).
    pub fn has_refresh_token(&self) -> bool {
        self.tokens.lock().unwrap().refresh_token.is_some()
    }

    /// Spotify authorize URL the operator visits once to grant access.
    pub fn authorize_url(&self) -> String {
        reqwest::Url::parse_with_params(
            AUTHORIZE_URL,
            &[
                ("client_id", self.cfg.client_id.as_str()),
                ("response_type", "code"),
                ("redirect_uri", self.cfg.redirect_uri.as_str()),
                ("scope", SCOPE),
            ],
        )
        .expect("valid Spotify authorize URL")
        .to_string()
    }

    fn basic_auth(&self) -> String {
        let raw = format!("{}:{}", self.cfg.client_id, self.cfg.client_secret);
        format!("Basic {}", BASE64.encode(raw))
    }

    /// Exchange an authorization `code` for tokens and persist the refresh
    /// token.  Called from the `/spotify/callback` route handler.
    pub async fn exchange_code(&self, code: &str) -> Result<(), String> {
        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", self.basic_auth())
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", self.cfg.redirect_uri.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("code exchange failed: {status}: {body}"));
        }

        let tr: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
        self.store_tokens(tr);
        Ok(())
    }

    fn store_tokens(&self, tr: TokenResponse) {
        let mut t = self.tokens.lock().unwrap();
        t.access_token = Some(tr.access_token);
        t.expires_at = Some(Instant::now() + Duration::from_secs(tr.expires_in));
        if let Some(rt) = tr.refresh_token {
            // Spotify only returns a refresh token on the initial exchange (and
            // occasionally on refresh); persist it whenever we do get one.
            if let Err(e) = std::fs::write(&self.cfg.token_file, &rt) {
                log::warn!("Could not persist Spotify refresh token: {e}");
            }
            t.refresh_token = Some(rt);
        }
    }

    fn invalidate_access_token(&self) {
        let mut t = self.tokens.lock().unwrap();
        t.access_token = None;
        t.expires_at = None;
    }

    /// Return a valid access token, refreshing it if missing or near expiry.
    async fn ensure_access_token(&self) -> Result<String, String> {
        {
            let t = self.tokens.lock().unwrap();
            if let (Some(tok), Some(exp)) = (&t.access_token, t.expires_at) {
                if Instant::now() + Duration::from_secs(60) < exp {
                    return Ok(tok.clone());
                }
            }
        }
        self.refresh().await
    }

    async fn refresh(&self) -> Result<String, String> {
        let refresh_token = self
            .tokens
            .lock()
            .unwrap()
            .refresh_token
            .clone()
            .ok_or_else(|| "no Spotify refresh token; login required".to_string())?;

        let resp = self
            .http
            .post(TOKEN_URL)
            .header("Authorization", self.basic_auth())
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("token refresh failed: {status}: {body}"));
        }

        let tr: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
        let access = tr.access_token.clone();
        self.store_tokens(tr);
        Ok(access)
    }

    /// Fetch the currently playing item, timing the request so we can place the
    /// reported `progress_ms` at the midpoint of the round-trip.
    async fn currently_playing(&self) -> Result<Sample, FetchError> {
        let token = self
            .ensure_access_token()
            .await
            .map_err(FetchError::Other)?;

        let t0 = Instant::now();
        let resp = self
            .http
            .get(CURRENTLY_PLAYING_URL)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| FetchError::Other(e.to_string()))?;
        let t1 = Instant::now();
        let sampled_at = t0 + (t1 - t0) / 2;

        match resp.status().as_u16() {
            // 204: nothing is currently playing.
            204 => Ok(Sample {
                data: None,
                sampled_at,
            }),
            200 => {
                let data: CurrentlyPlaying = resp
                    .json()
                    .await
                    .map_err(|e| FetchError::Other(e.to_string()))?;
                Ok(Sample {
                    data: Some(data),
                    sampled_at,
                })
            }
            401 => {
                self.invalidate_access_token();
                Err(FetchError::Other(
                    "401 Unauthorized (token will be refreshed next poll)".to_string(),
                ))
            }
            429 => {
                // Spotify returns a `Retry-After` header (whole seconds) telling
                // us how long to wait before the next request.
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .map(Duration::from_secs);
                Err(FetchError::RateLimited(retry_after))
            }
            other => Err(FetchError::Other(format!(
                "unexpected status {other} from Spotify"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Response types (only the fields we use)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct CurrentlyPlaying {
    #[serde(default)]
    progress_ms: Option<u64>,
    #[serde(default)]
    is_playing: bool,
    #[serde(default)]
    item: Option<Item>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(default)]
    name: String,
    #[serde(default)]
    artists: Vec<Artist>,
}

#[derive(Deserialize)]
struct Artist {
    #[serde(default)]
    name: String,
}

// ---------------------------------------------------------------------------
// Background sync task
// ---------------------------------------------------------------------------

/// Spawn the Spotify integration if configured.  Returns the shared client so
/// the web layer can drive the OAuth login routes, or `None` when Spotify is
/// not configured.
pub fn spawn(
    state: Arc<Mutex<AppState>>,
    broadcast_tx: broadcast::Sender<String>,
) -> Option<Arc<SpotifyClient>> {
    let cfg = Config::from_env()?;
    let client = Arc::new(SpotifyClient::new(cfg));

    // Reflect initial auth state into the shared snapshot.
    {
        let connected = client.has_refresh_token();
        let mut s = state.lock().unwrap();
        s.spotify_available = true;
        s.spotify_connected = connected;
    }

    log::info!("Spotify: redirect URI = {}", client.cfg.redirect_uri);
    if client.has_refresh_token() {
        log::info!("Spotify: using stored refresh token");
    } else {
        log::info!("Spotify: not yet authorized – visit /spotify/login");
    }

    let poll_client = client.clone();
    actix_web::rt::spawn(async move {
        poll_loop(poll_client, state, broadcast_tx).await;
    });

    Some(client)
}

async fn poll_loop(
    client: Arc<SpotifyClient>,
    state: Arc<Mutex<AppState>>,
    broadcast_tx: broadcast::Sender<String>,
) {
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;

        if !client.has_refresh_token() {
            // Not logged in yet; the /spotify/callback route will populate it.
            continue;
        }

        match client.currently_playing().await {
            Ok(sample) => apply_sample(&client, &state, &broadcast_tx, sample),
            Err(FetchError::RateLimited(retry_after)) => {
                // Honour Spotify's Retry-After: log it and sleep that long
                // (plus a small margin) before the next poll.
                let wait = retry_after.unwrap_or(DEFAULT_RETRY_AFTER) + RETRY_MARGIN;
                log::warn!(
                    "Spotify 429 rate limited: Retry-After = {} – backing off {:.1}s",
                    retry_after
                        .map(|d| format!("{}s", d.as_secs()))
                        .unwrap_or_else(|| "(absent)".to_string()),
                    wait.as_secs_f64()
                );
                tokio::time::sleep(wait).await;
            }
            Err(FetchError::Other(e)) => log::warn!("Spotify poll failed: {e}"),
        }
    }
}

/// Apply one poll result: update the now-playing label and, when the matched
/// song is playing, lock the FX show to its position.
fn apply_sample(
    client: &SpotifyClient,
    state: &Arc<Mutex<AppState>>,
    broadcast_tx: &broadcast::Sender<String>,
    sample: Sample,
) {
    let Sample { data, sampled_at } = sample;

    let cp = data.as_ref();
    let playing = cp.map(|c| c.is_playing).unwrap_or(false);
    let item = cp.and_then(|c| c.item.as_ref());

    let label = item.map(|it| {
        let artists = it
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .filter(|n| !n.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        if artists.is_empty() {
            it.name.clone()
        } else {
            format!("{artists} – {}", it.name)
        }
    });

    // Resolve which timecoded show (if any) the playing track maps to.
    let matched_fx: Option<usize> = if playing {
        item.and_then(|it| {
            let title = it.name.to_lowercase();
            client
                .cfg
                .matches
                .iter()
                .find(|(sub, _)| title.contains(sub.as_str()))
                .map(|(_, id)| *id)
        })
    } else {
        None
    };
    let progress_ms = cp.and_then(|c| c.progress_ms).unwrap_or(0);
    let spotify_secs = progress_ms as f64 / 1000.0;

    let mut s = state.lock().unwrap();

    let prev_label = s.now_playing.clone();
    let prev_controlling = s.spotify_controlling;

    // Drift between our extrapolated show clock and Spotify's freshly-reported
    // position, evaluated at the same instant. Only meaningful while we are
    // already driving this same show (a continuous playback).
    let offset_secs: Option<f64> = match (&s.fx_show_clock, matched_fx) {
        (Some(clock), Some(fx_id))
            if s.spotify_controlling && s.active_fx_theme_id == fx_id =>
        {
            Some(clock.at(sampled_at) - spotify_secs)
        }
        _ => None,
    };

    // Log every fetch, with the clock-vs-Spotify offset when we have one.
    let track_desc = label.as_deref().unwrap_or("(nothing playing)");
    match offset_secs {
        Some(off) => log::info!(
            "Spotify fetch: \"{track_desc}\" playing={playing} spotify={spotify_secs:.3}s \
             clock-vs-spotify offset={off:+.3}s"
        ),
        None => log::info!(
            "Spotify fetch: \"{track_desc}\" playing={playing} spotify={spotify_secs:.3}s \
             (no synced clock)"
        ),
    }

    s.now_playing = label;

    if let Some(fx_id) = matched_fx {
        if s.active_fx_theme_id != fx_id || s.fx_crossfade.is_some() {
            s.request_fx_theme(fx_id, None);
        }
        s.fx_show_clock = Some(ShowClock {
            anchor_secs: progress_ms as f64 / 1000.0,
            anchor_at: sampled_at,
            playing,
        });
        if !s.spotify_controlling {
            log::info!("Spotify: matched song playing – locking FX show {fx_id}");
        }
        s.spotify_controlling = true;
    } else if s.spotify_controlling {
        // The song ended or changed: release the FX bank exactly once, handing
        // it back to whatever the time-of-day schedule dictates (not blackout).
        let target = crate::scheduler::current_scheduled_fx_theme_id().unwrap_or(FX_OFF_ID);
        log::info!("Spotify: matched song stopped – returning FX bank to scheduled show {target}");
        s.fx_show_clock = None;
        s.spotify_controlling = false;
        s.request_fx_theme(target, None);
    }

    let changed = s.now_playing != prev_label || s.spotify_controlling != prev_controlling;
    if changed {
        crate::web::push_state(&s, broadcast_tx);
    }
}
