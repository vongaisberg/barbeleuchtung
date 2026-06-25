//! `AudioFeatures` — the immutable snapshot the DSP publishes and effects read.
//!
//! One of these is produced per analysis frame and published via `ArcSwap`.
//! The engine loads the latest each tick (a single atomic pointer read) and
//! hands it to effects through `TickContext::audio`. All fields are normalised
//! to `0.0..=1.0` unless noted, so effects can map them directly to channels.

use std::sync::Arc;

use arc_swap::ArcSwap;

use super::config::NUM_BANDS;

/// A point-in-time description of the music. Cheap to clone (it's all POD).
///
/// Trimmed to the signals the show actually uses. The earlier experimental
/// detectors (discrete drop/build/breakdown/section, per-onset kick/snare/crash)
/// fired unreliably on dense masters and were removed; what survived are the
/// signals that proved robust: the beat grid, song-relative loudness, a
/// continuous `intensity`, the `four_on_floor` groove state, and per-band levels.
#[derive(Clone, Debug)]
pub struct AudioFeatures {
    /// Engine-clock seconds when this frame was analysed (monotonic).
    pub t_capture: f64,

    /// **Song-relative perceptual loudness**, `0..1`. A-weighted spectral power
    /// normalised within the track's own rolling loudness range (≈45 s).
    pub energy: f32,
    /// Continuous musical **intensity**, `0..1` — the main drive signal.
    /// Arrangement-based: sub-bass presence (dominant) + onset density + highs,
    /// each song-relative. The right "how energetic right now" for modern,
    /// loudness-flat masters.
    pub intensity: f32,
    /// `0..1` — how consistently kicks land on the beat (the four-on-the-floor
    /// groove state). The reliable "is the beat driving" cue.
    pub four_on_floor: f32,
    /// Position of the current beat within the bar, `0` (downbeat) .. `3`, from
    /// best-effort downbeat detection; `-1` when the downbeat isn't confident
    /// (effects then free-run a 4-beat bar from the beat count).
    pub beat_in_bar: i32,

    /// Per-band slow (musical) envelope, post-AGC. Index 0 = lowest band.
    pub bands: [f32; NUM_BANDS],
    /// Per-band fast (transient) envelope, post-AGC.
    pub bands_fast: [f32; NUM_BANDS],
    /// Strength of the most recent onset, decaying toward 0 between onsets.
    pub onset: f32,

    /// True only on the analysis frame a beat lands (a one-frame pulse).
    pub beat_now: bool,
    /// Continuous phase within the current beat, `0.0..1.0`.
    pub beat_phase: f32,
    /// Total beats elapsed since lock (integral of `beat_phase`) — drives
    /// bar-synced choreography. Advances at the locked tempo; 0 until locked.
    pub beat_clock: f64,
    /// Current tempo estimate in BPM (0 until locked).
    pub bpm: f32,
    /// Tempo-lock confidence, `0.0..1.0`.
    pub beat_confidence: f32,
}

impl AudioFeatures {
    /// The all-zero baseline used before audio arrives and wherever a
    /// non-reactive `TickContext` is constructed.
    pub const fn silent() -> Self {
        Self {
            t_capture: 0.0,
            energy: 0.0,
            intensity: 0.0,
            four_on_floor: 0.0,
            beat_in_bar: -1,
            onset: 0.0,
            bands: [0.0; NUM_BANDS],
            bands_fast: [0.0; NUM_BANDS],
            beat_now: false,
            beat_phase: 0.0,
            beat_clock: 0.0,
            bpm: 0.0,
            beat_confidence: 0.0,
        }
    }
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self::silent()
    }
}

/// Lock-free shared handle to the latest features.
///
/// The analysis thread `store`s new snapshots; the engine `load`s them. Cloning
/// the `Arc` is how the engine gets a reader.
pub type FeatureHandle = Arc<ArcSwap<AudioFeatures>>;

/// Build a fresh handle initialised to silence.
pub fn new_handle() -> FeatureHandle {
    Arc::new(ArcSwap::from_pointee(AudioFeatures::silent()))
}

/// A cheap, shared `Arc` to a permanently-silent `AudioFeatures`.
///
/// Used to fill `TickContext::audio` wherever no live audio is in play (e.g.
/// offline timecode-show tests), without allocating a new snapshot each time.
pub fn silent_features() -> Arc<AudioFeatures> {
    use std::sync::OnceLock;
    static SILENT: OnceLock<Arc<AudioFeatures>> = OnceLock::new();
    SILENT
        .get_or_init(|| Arc::new(AudioFeatures::silent()))
        .clone()
}
