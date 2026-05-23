//! Themed lighting scenes for Barbeleuchtung.
//!
//! Each phase lives in its own file.  Common colour math and LFO helpers
//! are defined here so they can be shared without repetition.

pub mod cheshire_moon;
pub mod closed;
pub mod garden_of_flowers;
pub mod golden_afternoon;
pub mod mad_hatter;
pub mod police;
pub mod stained_glass;
pub mod sunlight;
pub mod tea_garden;
pub mod fx_off;
pub mod fx_looking_glass;
pub mod fx_cheshire_grin;
pub mod fx_white_rabbit;

use crate::theme::Theme;

// ─── Shared constants ─────────────────────────────────────────────────────

/// Golden angle in radians: 2π × (2 − φ).
/// Multiplying a spot index by this gives maximally-spread phase offsets
/// with no visible periodicity, even for small fixture counts.
pub(crate) const GOLDEN_ANGLE: f64 = 2.399_963_229_728_65;

/// Golden ratio φ – used as a secondary frequency multiplier so that the two
/// LFO components are incommensurate (their ratio is irrational).
pub(crate) const PHI: f64 = 1.618_033_988_749_895;

/// Number of individually-addressable spots in each ceiling fixture
/// (LOUNGE_DECKE / BAR_DECKE both carry 10 × RGBW spots as one DMX device).
pub(crate) const NUM_SPOTS: usize = 10;

// ─── RGBW colour type ─────────────────────────────────────────────────────

/// Normalised RGBW colour (each component in [0.0, 1.0]).
/// This is local to the themes module; the engine only sees flat `Vec<f32>`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rgbw {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub w: f32,
}

impl Rgbw {
    pub const fn new(r: f32, g: f32, b: f32, w: f32) -> Self {
        Self { r, g, b, w }
    }

    /// Scale all components by `s`.
    pub fn scale(self, s: f32) -> Self {
        Self {
            r: self.r * s,
            g: self.g * s,
            b: self.b * s,
            w: self.w * s,
        }
    }

    /// Linear interpolation from `self` toward `other` by `t ∈ [0, 1]`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            w: self.w + (other.w - self.w) * t,
        }
    }

    /// Append [R, G, B, W] to `buf` – the canonical output order of DECKE_CHANNELS.
    pub fn push_to(self, buf: &mut Vec<f32>) {
        buf.push(self.r);
        buf.push(self.g);
        buf.push(self.b);
        buf.push(self.w);
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            r: self.r + other.r,
            g: self.g + other.g,
            b: self.b + other.b,
            w: self.w + other.w,
        }
    }
}

// ─── Shared LFO helpers ───────────────────────────────────────────────────

/// Organic per-spot LFO returning a value in [-1.0, 1.0].
///
/// Combines two sine waves at `freq_a` and `freq_a * PHI` Hz.  The golden
/// phase seed (`spot * GOLDEN_ANGLE`) ensures each spot has a unique, widely-
/// separated starting phase, so no two spots look synchronised.
///
/// The PHI frequency multiplier makes the two components incommensurate: the
/// resulting waveform never exactly repeats (the period would be infinite).
pub(crate) fn organic_lfo(t: f64, spot: usize, freq_a: f64) -> f32 {
    let phase = spot as f64 * GOLDEN_ANGLE;
    let freq_b = freq_a * PHI;
    let a = (t * freq_a + phase).sin();
    let b = (t * freq_b + phase * PHI).sin();
    // Weight: 65 % primary, 35 % secondary so the combined range stays in [-1, 1].
    (0.65 * a + 0.35 * b) as f32
}

/// Shaped LFO that spends most of its time near its extremes and transitions
/// quickly between them – approximating "hold + fade" behaviour.
///
/// Uses `tanh(k · sin(ωt + phase))` normalised to [0, 1].
/// With k = 3 and a 28-second period this yields ≈ 4 s fade / 10 s hold,
/// which matches the "Rabbit Hole" spec exactly.
pub(crate) fn shaped_lfo(t: f64, spot: usize, period_s: f64, k: f64) -> f32 {
    let phase = spot as f64 * GOLDEN_ANGLE;
    let omega = std::f64::consts::TAU / period_s;
    let sine = (omega * t + phase).sin();
    (0.5 + 0.5 * (k * sine).tanh()) as f32
}

/// Build a flat 40-channel Vec from a per-spot colour closure.
/// `f(spot_index) -> Rgbw` is called for each of the 10 spots.
pub(crate) fn build_ceiling(f: impl Fn(usize) -> Rgbw) -> Vec<f32> {
    let mut out = Vec::with_capacity(NUM_SPOTS * 4);
    for spot in 0..NUM_SPOTS {
        f(spot).push_to(&mut out);
    }
    out
}

/// Build a static 40-channel constant (same colour for every spot).
pub(crate) fn uniform_ceiling(color: Rgbw) -> Vec<f32> {
    build_ceiling(|_| color)
}

// ─── Decke theme registry ─────────────────────────────────────────────────

pub fn all_themes() -> Vec<Theme> {
    vec![
        mad_hatter::theme(),         // 0 – 11:00–17:00
        golden_afternoon::theme(),   // 1 – 17:00–19:00
        cheshire_moon::theme(),      // 2 – 19:00–close
        closed::theme(),             // 3 – 02:00 onward
        police::theme(),             // 4 – manual override
        sunlight::theme(),           // 5 – manual override
        garden_of_flowers::theme(),  // 6 – manual override
        stained_glass::theme(),      // 7 – manual override
        tea_garden::theme(),         // 8 – manual override
    ]
}

pub fn theme_names() -> Vec<&'static str> {
    vec![
        mad_hatter::NAME,
        golden_afternoon::NAME,
        cheshire_moon::NAME,
        closed::NAME,
        police::NAME,
        sunlight::NAME,
        garden_of_flowers::NAME,
        stained_glass::NAME,
        tea_garden::NAME,
    ]
}

// ─── FX theme registry ────────────────────────────────────────────────────

pub fn all_fx_themes() -> Vec<Theme> {
    vec![
        fx_off::theme(),             // 0 – all FX fixtures dark
        fx_looking_glass::theme(),   // 1 – daytime: glinting gems + amber pars
        fx_cheshire_grin::theme(),     // 2 – evening: cheshire grin + vortex beams
        fx_white_rabbit::theme(),    // 3 – late night: digital rain + chaos beams
    ]
}

pub fn fx_theme_names() -> Vec<&'static str> {
    vec![
        fx_off::NAME,
        fx_looking_glass::NAME,
        fx_cheshire_grin::NAME,
        fx_white_rabbit::NAME,
    ]
}
