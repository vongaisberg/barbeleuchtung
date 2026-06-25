//! Four-on-the-floor groove detection.
//!
//! Of the "musical element" cues explored, this is the one that proved reliable
//! and is actually used: a continuous **state** for whether a kick is landing
//! consistently on the beat. (Per-onset event detection — individual kick/snare/
//! crash hits — was unreliable on dense, loudness-flat masters and was removed;
//! rhythmic gestures ride the beat grid gated by this state instead.)
//!
//! Fed the RAW low-band magnitude (onset detection needs the transients, not the
//! AGC-smoothed bands).

const NB: usize = 8;

/// Adaptive flux→onset detector for the low band.
struct GroupOnset {
    prev: f32,
    mean: f32,
    var: f32,
    frames_since: u32,
    min_gap: u32,
}

impl GroupOnset {
    fn new(min_gap: u32) -> Self {
        Self { prev: 0.0, mean: 0.0, var: 0.0, frames_since: 1_000, min_gap }
    }
    /// Returns true on a frame where a low-band onset (kick) fires.
    fn update(&mut self, level: f32, sensitivity: f32) -> bool {
        let flux = (level - self.prev).max(0.0);
        self.prev = level;
        let a = 0.1;
        let d = flux - self.mean;
        self.mean += a * d;
        self.var = (1.0 - a) * (self.var + a * d * d);
        let thr = self.mean + sensitivity * self.var.sqrt();
        self.frames_since = self.frames_since.saturating_add(1);
        if flux > thr && self.frames_since >= self.min_gap {
            self.frames_since = 0;
            true
        } else {
            false
        }
    }
}

/// Tracks the four-on-the-floor groove state from low-band onsets vs the beat.
pub struct ElementTracker {
    lo: GroupOnset,
    fof: f32,
    frames_since_kick: u32,
    on_beat_window: u32,
}

impl ElementTracker {
    pub fn new(_hop_s: f32, analysis_hz: f32) -> Self {
        let min_gap = ((0.06 * analysis_hz) as u32).max(1); // ~60 ms debounce
        Self {
            lo: GroupOnset::new(min_gap),
            fof: 0.0,
            frames_since_kick: 1_000,
            on_beat_window: ((0.10 * analysis_hz) as u32).max(1), // kick within 100 ms of beat
        }
    }

    /// Feed one frame's raw band magnitudes and the beat flag; returns the
    /// current four-on-the-floor level (`0..1`).
    pub fn update(&mut self, band_raw: &[f32; NB], beat_now: bool) -> f32 {
        let low = 0.5 * (band_raw[0] + band_raw[1]);
        let kick = self.lo.update(low, 1.8);
        self.frames_since_kick = if kick { 0 } else { self.frames_since_kick.saturating_add(1) };
        if beat_now {
            let on_beat = if self.frames_since_kick <= self.on_beat_window { 1.0 } else { 0.0 };
            self.fof += 0.25 * (on_beat - self.fof); // ~4-beat memory
        }
        self.fof.clamp(0.0, 1.0)
    }
}
