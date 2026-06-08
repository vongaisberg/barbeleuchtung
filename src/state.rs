use std::time::Instant;

use crate::engine::NUM_UNIVERSES;
use crate::fixtures;

/// Tracks an in-progress crossfade between two theme IDs.
#[derive(Clone, Debug)]
pub struct CrossfadeState {
    /// Theme we are fading away from.
    pub from_theme_id: usize,
    /// Theme we are fading towards.
    pub to_theme_id: usize,
    /// Crossfade total duration in milliseconds.
    pub duration_ms: u64,
    /// Wall-clock instant when the crossfade started.
    pub started_at: Instant,
}

impl CrossfadeState {
    /// Returns the blend factor `0.0..=1.0` (0 = fully `from`, 1 = fully `to`).
    pub fn factor(&self) -> f32 {
        let elapsed_ms = self.started_at.elapsed().as_millis() as f64;
        (elapsed_ms / self.duration_ms as f64).clamp(0.0, 1.0) as f32
    }

    pub fn is_complete(&self) -> bool {
        self.factor() >= 1.0
    }
}

/// An externally-driven clock for the FX show's `show_time`.
///
/// Used to lock a timecoded FX show to an outside transport (e.g. Spotify's
/// `progress_ms`).  Rather than store the position only at poll time, we keep
/// an *anchor*: the show position `anchor_secs` that was true at the local
/// instant `anchor_at`.  While `playing`, the current position extrapolates
/// forward from that anchor using the local monotonic clock, so playback stays
/// smooth at 40 Hz between the (much sparser) polls.  Each poll re-anchors,
/// correcting any drift.
#[derive(Clone, Debug)]
pub struct ShowClock {
    /// Show position, in seconds, that was true at `anchor_at`.
    pub anchor_secs: f64,
    /// Local monotonic instant the anchor was captured.
    pub anchor_at: Instant,
    /// Whether the transport is currently advancing (paused → frozen).
    pub playing: bool,
}

impl ShowClock {
    /// Current show position in seconds, extrapolated from the anchor.
    pub fn now(&self) -> f64 {
        self.at(Instant::now())
    }

    /// Show position this clock would report at an arbitrary instant `t`.
    /// Used to compare our extrapolated position against a freshly fetched
    /// transport position sampled at the same instant.
    pub fn at(&self, t: Instant) -> f64 {
        if self.playing {
            self.anchor_secs + t.saturating_duration_since(self.anchor_at).as_secs_f64()
        } else {
            self.anchor_secs
        }
    }
}

/// Shared application state, protected externally by `Arc<Mutex<AppState>>`.
pub struct AppState {
    /// Index into the theme registry of the currently active theme.
    pub active_theme_id: usize,
    /// If a crossfade is in progress this holds its descriptor.
    pub crossfade: Option<CrossfadeState>,
    /// Index into the FX theme registry of the currently active FX theme.
    pub active_fx_theme_id: usize,
    /// If an FX crossfade is in progress this holds its descriptor.
    pub fx_crossfade: Option<CrossfadeState>,
    /// Wall-clock instant the active FX theme became active.  Drives
    /// `TickContext::show_time` so timecoded FX shows start from ~0 when
    /// selected.  Reset whenever `active_fx_theme_id` changes.
    pub fx_theme_started_at: Instant,
    /// One value per fader, in `0.0..=1.0`.
    /// Length must equal `fixtures::FADER_FIXTURES.len()`.
    pub fader_values: Vec<f32>,
    /// If true, the engine sends all-zero DMX output regardless of themes.
    pub blackout: bool,
    /// Fog machine automation enabled.
    pub fog_enabled: bool,
    /// Minutes between fog bursts (x).  Range: 1–30.
    pub fog_interval_min: f32,
    /// Seconds the pump runs per burst (y).  Range: 1–30.
    pub fog_duration_s: f32,
    /// Pump output level (z), in `0.0..=1.0`.  Range: 0.01–0.30.
    pub fog_level: f32,
    /// Per-universe DMX output mute, indexed by Art-Net universe number.
    /// When `universe_muted[uni]` is `true`, the engine skips sending the
    /// ArtDmx frame for that universe entirely (it does *not* send all-zero
    /// frames – that is what `blackout` does).  Length always equals
    /// `engine::NUM_UNIVERSES`.
    pub universe_muted: Vec<bool>,
    /// When `Some`, overrides the FX bank's `show_time` with an externally
    /// driven clock (Spotify playback position).  When `None`, the FX show
    /// runs from `fx_theme_started_at` as usual.
    pub fx_show_clock: Option<ShowClock>,
    /// Human-readable "Artist – Title" of the track Spotify reports as
    /// currently playing, or `None` when nothing is playing / Spotify is idle.
    pub now_playing: Option<String>,
    /// Whether the Spotify integration is configured at all (client id/secret
    /// present).  When false the UI hides all Spotify controls.
    pub spotify_available: bool,
    /// Whether a Spotify refresh token is available (i.e. the operator has
    /// completed the OAuth login at least once).  Surfaced to the UI so it can
    /// show a "Connect Spotify" link when needed.
    pub spotify_connected: bool,
    /// True while the Spotify sync currently owns the FX bank (because the
    /// matched song is playing).  Lets us hand control back exactly once when
    /// the song ends instead of fighting manual scene selection.
    pub spotify_controlling: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_theme_id: 0,
            crossfade: None,
            active_fx_theme_id: 0,
            fx_crossfade: None,
            fx_theme_started_at: Instant::now(),
            fader_values: vec![0.0; fixtures::FADER_FIXTURES.len()],
            blackout: false,
            fog_enabled: false,
            fog_interval_min: 1.0,
            fog_duration_s: 5.0,
            fog_level: 0.05,
            universe_muted: vec![false; NUM_UNIVERSES],
            fx_show_clock: None,
            now_playing: None,
            spotify_available: false,
            spotify_connected: false,
            spotify_controlling: false,
        }
    }

    /// Request a theme change. If the incoming theme uses a crossfade
    /// transition this initiates crossfade tracking; otherwise it switches
    /// instantly.
    pub fn request_theme(
        &mut self,
        new_id: usize,
        duration_ms: Option<u64>,
    ) {
        if new_id == self.active_theme_id {
            return;
        }

        match duration_ms {
            Some(dur) if dur > 0 => {
                let from = self
                    .crossfade
                    .as_ref()
                    .map(|cf| cf.to_theme_id)
                    .unwrap_or(self.active_theme_id);
                self.crossfade = Some(CrossfadeState {
                    from_theme_id: from,
                    to_theme_id: new_id,
                    duration_ms: dur,
                    started_at: Instant::now(),
                });
            }
            _ => {
                self.active_theme_id = new_id;
                self.crossfade = None;
            }
        }
    }

    /// Request an FX theme change, with the same crossfade logic as
    /// `request_theme` but operating on the FX bank fields.
    pub fn request_fx_theme(
        &mut self,
        new_id: usize,
        duration_ms: Option<u64>,
    ) {
        if new_id == self.active_fx_theme_id {
            return;
        }

        match duration_ms {
            Some(dur) if dur > 0 => {
                let from = self
                    .fx_crossfade
                    .as_ref()
                    .map(|cf| cf.to_theme_id)
                    .unwrap_or(self.active_fx_theme_id);
                self.fx_crossfade = Some(CrossfadeState {
                    from_theme_id: from,
                    to_theme_id: new_id,
                    duration_ms: dur,
                    started_at: Instant::now(),
                });
            }
            _ => {
                self.active_fx_theme_id = new_id;
                self.fx_crossfade = None;
                self.fx_theme_started_at = Instant::now();
            }
        }
    }

    /// Called by the engine each tick to advance / complete crossfades.
    pub fn tick_crossfade(&mut self) {
        if let Some(cf) = &self.crossfade {
            if cf.is_complete() {
                self.active_theme_id = cf.to_theme_id;
                self.crossfade = None;
            }
        }
        if let Some(cf) = &self.fx_crossfade {
            if cf.is_complete() {
                self.active_fx_theme_id = cf.to_theme_id;
                self.fx_crossfade = None;
                self.fx_theme_started_at = Instant::now();
            }
        }
    }
}
