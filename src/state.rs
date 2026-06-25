use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Instant;

use crate::engine::NUM_UNIVERSES;
use crate::fixtures;

/// Lock-free snapshot of what the generative engine is outputting right now.
/// The DMX engine writes; the audio-meter thread and web snapshot read.
#[derive(Default)]
pub struct ReactiveLive {
    pub auto_mode: AtomicBool,
    pub tier: AtomicU8,
    pub palette: AtomicU8,
}

impl ReactiveLive {
    pub fn store(&self, auto_mode: bool, tier: u8, palette: u8) {
        self.auto_mode.store(auto_mode, Ordering::Relaxed);
        self.tier.store(tier, Ordering::Relaxed);
        self.palette.store(palette, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (bool, u8, u8) {
        (
            self.auto_mode.load(Ordering::Relaxed),
            self.tier.load(Ordering::Relaxed),
            self.palette.load(Ordering::Relaxed),
        )
    }
}

/// Live, operator-tunable controls for the reactive bank, surfaced in the web
/// UI. Stored in [`AppState`] and copied into each tick's context so the
/// stateless reactive effects can read them.
#[derive(Clone, Copy, Debug)]
pub struct ReactiveControls {
    /// Master brightness for the whole reactive bank, `0.0..=1.0` (applied as
    /// an output scale by the engine).
    pub master: f32,
    /// Audio sensitivity, `~0.5..=2.0` — multiplies perceived energy/onset so
    /// the operator can make the show more or less reactive to a given level.
    pub sensitivity: f32,
    /// Strobe amount, `0.0..=1.0` — scales (and at 0 disables) every white
    /// blinder/strobe gesture.
    pub strobe: f32,
    /// Operator tier override for Auto mode: `-1` = automatic (follow the
    /// music), `0..=3` = force that energy tier (calm / groove / club / peak).
    pub tier_lock: i8,
    /// Operator palette override for Auto mode: `-1` = automatic rotation,
    /// `0..=3` = force that palette (Neon / Warm / Cool / Acid).
    pub palette_lock: i8,
}

impl Default for ReactiveControls {
    fn default() -> Self {
        Self { master: 1.0, sensitivity: 1.0, strobe: 1.0, tier_lock: -1, palette_lock: -1 }
    }
}

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

/// Operator state captured when Spotify seizes the rig for a timecoded show.
#[derive(Clone, Copy, Debug)]
pub struct SpotifySavedState {
    pub fx_theme_id: usize,
    pub reactive_auto: bool,
    pub reactive_theme_id: usize,
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
    /// Operator selection saved when Spotify first takes over a timecoded show;
    /// restored (FX theme + reactive Auto/manual) when the song ends.
    pub spotify_saved: Option<SpotifySavedState>,
    /// Index into the reactive theme registry of the active reactive theme.
    /// The reactive bank renders additively from live audio analysis.
    pub active_reactive_theme_id: usize,
    /// If a reactive crossfade is in progress this holds its descriptor.
    pub reactive_crossfade: Option<CrossfadeState>,
    /// When true, a background selector picks the reactive look automatically
    /// from the music's energy (Dark→Groove→Club, with Strobe on peaks).
    /// A manual look selection clears this so the operator's choice is pinned.
    pub reactive_auto: bool,
    /// Live operator controls for the reactive bank.
    pub reactive_controls: ReactiveControls,
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
            spotify_saved: None,
            active_reactive_theme_id: 0,
            reactive_crossfade: None,
            reactive_auto: false,
            reactive_controls: ReactiveControls::default(),
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

    /// Request a reactive theme change, with the same crossfade logic as
    /// `request_theme` but operating on the reactive bank fields.
    pub fn request_reactive_theme(&mut self, new_id: usize, duration_ms: Option<u64>) {
        if new_id == self.active_reactive_theme_id {
            return;
        }
        match duration_ms {
            Some(dur) if dur > 0 => {
                let from = self
                    .reactive_crossfade
                    .as_ref()
                    .map(|cf| cf.to_theme_id)
                    .unwrap_or(self.active_reactive_theme_id);
                self.reactive_crossfade = Some(CrossfadeState {
                    from_theme_id: from,
                    to_theme_id: new_id,
                    duration_ms: dur,
                    started_at: Instant::now(),
                });
            }
            _ => {
                self.active_reactive_theme_id = new_id;
                self.reactive_crossfade = None;
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
        if let Some(cf) = &self.reactive_crossfade {
            if cf.is_complete() {
                self.active_reactive_theme_id = cf.to_theme_id;
                self.reactive_crossfade = None;
            }
        }
    }

    /// FX theme the operator sees (destination during a crossfade).
    pub fn effective_fx_theme_id(&self) -> usize {
        self.fx_crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(self.active_fx_theme_id)
    }

    /// Reactive theme the operator sees (destination during a crossfade).
    pub fn effective_reactive_theme_id(&self) -> usize {
        self.reactive_crossfade
            .as_ref()
            .map(|cf| cf.to_theme_id)
            .unwrap_or(self.active_reactive_theme_id)
    }

    /// Spotify matched a timecoded show: remember the operator's FX + reactive
    /// selection and silence the reactive bank so it doesn't fight the script.
    pub fn spotify_begin_control(&mut self) {
        if self.spotify_saved.is_none() {
            self.spotify_saved = Some(SpotifySavedState {
                fx_theme_id: self.effective_fx_theme_id(),
                reactive_auto: self.reactive_auto,
                reactive_theme_id: self.effective_reactive_theme_id(),
            });
        }
        self.reactive_auto = false;
        self.reactive_crossfade = None;
        self.active_reactive_theme_id = 0;
    }

    /// Timecoded show ended: hand FX + reactive back to the saved selection.
    pub fn spotify_end_control(&mut self) {
        self.fx_show_clock = None;
        self.spotify_controlling = false;
        let Some(saved) = self.spotify_saved.take() else {
            return;
        };
        self.fx_crossfade = None;
        self.active_fx_theme_id = saved.fx_theme_id;
        self.fx_theme_started_at = Instant::now();
        self.reactive_auto = saved.reactive_auto;
        self.reactive_crossfade = None;
        if saved.reactive_auto {
            // Generative Auto — manual look id is ignored by the engine.
        } else {
            self.active_reactive_theme_id = saved.reactive_theme_id;
        }
    }
}
