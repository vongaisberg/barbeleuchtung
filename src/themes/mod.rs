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
pub mod fx_off_with_their_heads;
pub mod fx_jabberwocky;
pub mod reactive;
pub mod timecode;

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

// ─── SC-X50 MkII DMX channel constants ───────────────────────────────────
//
// All values are normalised to [0.0, 1.0] (= DMX value / 255).
// The 11-channel order is:
//   [0] pan  [1] tilt  [2] color  [3] shutter  [4] dimmer
//   [5] gobo  [6] gobo-rot  [7] prism  [8] focus  [9] functions  [10] programs

/// Shutter: fully open (DMX 4–7 or 216–255 = open; we use 216).
pub(crate) const SC_SHUTTER_OPEN: f32  = 216.0 / 255.0;
/// Shutter: blackout / closed (DMX 0–3).
pub(crate) const SC_SHUTTER_CLOSED: f32 = 0.0;
/// Shutter: medium-fast strobe (DMX ≈ 120 out of the 8–215 range).
pub(crate) const SC_SHUTTER_STROBE_MED: f32 = 120.0 / 255.0;

// Color wheel positions (centre of each clean-color band):
pub(crate) const SC_COLOR_WHITE:     f32 =   3.0 / 255.0; // 0–6
pub(crate) const SC_COLOR_YELLOW:    f32 =  10.0 / 255.0; // 7–13
pub(crate) const SC_COLOR_PINK:      f32 =  17.0 / 255.0; // 14–20
pub(crate) const SC_COLOR_GREEN:     f32 =  24.0 / 255.0; // 21–27
pub(crate) const SC_COLOR_RED:       f32 =  31.0 / 255.0; // 28–34
pub(crate) const SC_COLOR_BLUE:      f32 =  38.0 / 255.0; // 35–41
pub(crate) const SC_COLOR_KGREEN:    f32 =  45.0 / 255.0; // 42–48 kelly-green
pub(crate) const SC_COLOR_SALMON:    f32 =  52.0 / 255.0; // 49–55 lachsrot / salmon
pub(crate) const SC_COLOR_DKBLUE:    f32 =  59.0 / 255.0; // 56–63 dark blue
/// Slow positive rainbow spin.
pub(crate) const SC_COLOR_RAINBOW:   f32 = 148.0 / 255.0; // 128–191

// Gobo wheel positions (centre of each band, 8 ch mode & 11 ch mode identical):
pub(crate) const SC_GOBO_OPEN:       f32 =   3.0 / 255.0; // 0–7
pub(crate) const SC_GOBO_1:          f32 =  11.0 / 255.0; // 8–15
pub(crate) const SC_GOBO_2:          f32 =  19.0 / 255.0; // 16–23
pub(crate) const SC_GOBO_3:          f32 =  27.0 / 255.0; // 24–31
pub(crate) const SC_GOBO_4:          f32 =  35.0 / 255.0; // 32–39
pub(crate) const SC_GOBO_5:          f32 =  43.0 / 255.0; // 40–47
pub(crate) const SC_GOBO_6:          f32 =  51.0 / 255.0; // 48–55
pub(crate) const SC_GOBO_7:          f32 =  59.0 / 255.0; // 56–63
/// Slow positive gobo rainbow spin (128–191).
pub(crate) const SC_GOBO_SPIN:       f32 = 148.0 / 255.0;

// Gobo rotation speeds:
/// No rotation.
pub(crate) const SC_GOBOROT_NONE:    f32 =  30.0 / 255.0; // 0–63
/// Disabled due to creaky gear
pub(crate) const SC_GOBOROT_MED_POS: f32 = SC_GOBOROT_NONE;
/// Disabled due to creaky gear
pub(crate) const SC_GOBOROT_FAST_POS: f32 = SC_GOBOROT_NONE;

// Prism:
/// Prism off (0–3 = unused).
pub(crate) const SC_PRISM_OFF:    f32 =   0.0 / 255.0;
/// Prism: medium positive rotation (midpoint of 4–127).
pub(crate) const SC_PRISM_ROT:    f32 =  32.0 / 255.0;
/// Prism: static (252–255).
pub(crate) const SC_PRISM_STATIC: f32 = 254.0 / 255.0;

// Focus – midpoint is typically in focus for stage-mounted scanners.
pub(crate) const SC_FOCUS_MID: f32 = 0.5;

// Functions channel – 0 = none / no auto-blackout during movement.
pub(crate) const SC_FUNC_NONE: f32 = 0.0;

// Programs channel – 0 = DMX control (no built-in program).
pub(crate) const SC_PROG_DMX: f32 = 0.0;

/// Build a flat 11-channel Vec for one SC-X50 MkII frame.
///
/// Argument order mirrors the DMX channel order so it's easy to cross-reference
/// with the manual: pan, tilt, color, shutter, dimmer, gobo, gobo_rot,
/// prism, focus, functions, programs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scanner_frame(
    pan: f32,
    tilt: f32,
    color: f32,
    shutter: f32,
    dimmer: f32,
    gobo: f32,
    gobo_rot: f32,
    prism: f32,
    focus: f32,
) -> Vec<f32> {
    vec![pan, tilt, color, shutter, dimmer, gobo, gobo_rot, prism, focus, SC_FUNC_NONE, SC_PROG_DMX]
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

/// FX theme registry index of the all-dark scene.
pub const FX_OFF_ID: usize = 0;
/// FX theme registry index of the timecoded "Would You" song show.
/// Kept in sync with the order in [`all_fx_themes`] / [`fx_theme_names`].
pub const FX_WOULD_YOU_ID: usize = 6;
/// FX theme registry index of the timecoded "Prada" song show.
/// Kept in sync with the order in [`all_fx_themes`] / [`fx_theme_names`].
pub const FX_PRADA_ID: usize = 7;
/// FX theme registry index of the timecoded "DJ Turn It Up" song show.
/// Kept in sync with the order in [`all_fx_themes`] / [`fx_theme_names`].
pub const FX_DJ_TURN_IT_UP_ID: usize = 8;
/// FX theme registry index of the timecoded "Schrei nach Liebe" song show.
/// Kept in sync with the order in [`all_fx_themes`] / [`fx_theme_names`].
pub const FX_SCHREI_NACH_LIEBE_ID: usize = 9;

pub fn all_fx_themes() -> Vec<Theme> {
    vec![
        fx_off::theme(),                    // 0 – all FX fixtures dark
        fx_looking_glass::theme(),          // 1 – daytime: glinting gems + amber pars
        fx_cheshire_grin::theme(),          // 2 – evening: cheshire grin + vortex beams
        fx_white_rabbit::theme(),           // 3 – late night: digital rain + chaos beams + scanner lighthouse
        fx_off_with_their_heads::theme(),   // 4 – late night: hard red/white + scanner stabs
        fx_jabberwocky::theme(),            // 5 – late night: green/purple predator + scanner eyes
        timecode::fx_would_you::theme(),    // 6 – timecoded "Would You" show (Spotify-synced)
        timecode::fx_prada_v2::theme(),     // 7 – timecoded Prada show v2
        timecode::fx_dj_turn_it_up::theme(),// 8 – timecoded "DJ Turn It Up" show
        timecode::fx_schrei_nach_liebe::theme(), // 9 – timecoded "Schrei nach Liebe" show
    ]
}   

pub fn fx_theme_names() -> Vec<&'static str> {
    vec![
        fx_off::NAME,
        fx_looking_glass::NAME,
        fx_cheshire_grin::NAME,
        fx_white_rabbit::NAME,
        fx_off_with_their_heads::NAME,
        fx_jabberwocky::NAME,
        timecode::fx_would_you::NAME,
        timecode::fx_prada_v2::NAME,
        timecode::fx_dj_turn_it_up::NAME,
        timecode::fx_schrei_nach_liebe::NAME,
    ]
}

// ─── Reactive theme registry ──────────────────────────────────────────────
//
// Audio-reactive bank: generative scenes driven live by `ctx.audio`. Index 0
// is always the all-dark "off" scene so the bank stays silent until selected.
// (Manual override looks; Auto renders the generative engine instead.)

pub fn all_reactive_themes() -> Vec<Theme> {
    vec![
        reactive::off(),    // 0 – all FX fixtures dark
        reactive::dark(),   // 1 – sparse / moody
        reactive::groove(), // 2 – colourful, movement-forward
        reactive::club(),   // 3 – full-energy workhorse
        reactive::strobe(), // 4 – white-heavy peak look
    ]
}

pub fn reactive_theme_names() -> Vec<&'static str> {
    vec![
        reactive::OFF_NAME,
        reactive::DARK_NAME,
        reactive::GROOVE_NAME,
        reactive::CLUB_NAME,
        reactive::STROBE_NAME,
    ]
}
