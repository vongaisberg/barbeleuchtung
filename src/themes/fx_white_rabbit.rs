//! # FX Theme 3 – "The White Rabbit"
//! **Trigger**: Manual – late night / spontaneous dance.
//!
//! ## Design intent
//! Pseudo-club preset when the mood shifts and people want to move.
//!
//! - **PixStrobes – "Digital Rain"**: A falling-drop animation in green/cyan
//!   across the 2 × 4 pixel grid.  Each column cycles top→bottom at 0.5 Hz,
//!   with a linear phase offset so the wave sweeps from the inside (stage
//!   centre) outward to the edges of the bar.  Intensity 50 %.
//!
//!   **Blinder swell**: The WW strip slowly fades 0 % → 30 % → 0 % over 10 s,
//!   creating an automatic "drop" that lights up the dancefloor, then retreats.
//!
//! - **QuadPhase – "Chaos"**: Fast continuous rotation, colour wheel slowly
//!   fading through its spectrum over 60 s so the beams constantly morph.
//!
//! - **LEDPars (RootPars)**: Deep purple at 40 % with a 4‑step 4/4 chase at
//!   120 BPM so each bar of music walks across the stage.
//!
//! ### Physical pixel layout (PixStrobe)
//! ```text
//! col:       0       1       2       3
//! top (row 0):  px1    px3    px5    px7
//! bot (row 1):  px2    px4    px6    px8
//! ```

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    scanner_frame, SC_COLOR_GREEN, SC_FOCUS_MID, SC_GOBO_4, SC_GOBO_2, SC_GOBOROT_MED_POS,
    SC_PRISM_OFF, SC_SHUTTER_CLOSED, SC_SHUTTER_OPEN,
};


pub const NAME: &str = "The White Rabbit";

// ─── "Digital Rain" pixel parameters ───────────────────────────────────────

/// Color of the falling drops (green-cyan, Matrix-ish).
const RAIN_R: f32 = 0.00;
const RAIN_G: f32 = 1.00;
const RAIN_B: f32 = 0.80;

/// Dim background so pixels are never fully dark (gives a "ghostly trail").
const RAIN_BG: f32 = 0.02;
/// Peak pixel intensity (50 % per spec).
const RAIN_INTENSITY: f32 = 0.50;

/// Drop cycle frequency: one top→bottom→top cycle per 2 s.
const RAIN_FREQ: f64 = 0.5;

/// Phase delay between adjacent columns, as a fraction of one rain cycle.
/// 0.25 → the wave sweeps across all 4 columns in exactly one full period.
const RAIN_COL_PHASE_STEP: f64 = 0.25;

// ─── Blinder swell parameters ────────────────────────────────────────────────

/// Full swell period in seconds: 0 % → 30 % → 0 %.
const BLINDER_PERIOD_S: f64 = 10.0;
/// Peak WW brightness of the swell.
const BLINDER_PEAK: f32 = 0.00;

// ─── QuadPhase DMX + modulation parameters ───────────────────────────────────
// ⚠  Approximate – verify against your fixture's DMX chart.

/// Shutter fully open.
const QP_SHUTTER_OPEN: f32 = 1.0;

/// Color wheel: multi-colour / rainbow position.
/// Often 0 = open white (all colours spinning), or a dedicated rainbow slot.
const QP_COLOR_MULTICOLOR: f32 = 0.0; // ⚠ check fixture manual

/// Rotation speed: medium-fast..
const QP_ROTATION_FAST: f32 = 0.700;
//
/// Slow colour fade period in seconds. One full 0→1→0 sweep of the colour
/// channel completes in this time.
const QP_COLOR_FADE_PERIOD_S: f64 = 60.0;

// ─── LEDPar output / chase parameters ───────────────────────────────────────

const PAR_DIMMER: f32 = 0.40;

/// [Dimmer, Strobe, R, G, B, W, Amber, UV] – deep purple.
const PAR_PURPLE: [f32; 8] = [PAR_DIMMER, 0.0, 0.70, 0.0, 1.0, 0.0, 0.0, 0.0];

/// RootPar chase tempo in beats per minute.
const PAR_CHASE_BPM: f64 = 120.0;
/// Duration of a single beat in seconds at 120 BPM (0.5 s).
const PAR_BEAT_PERIOD_S: f64 = 60.0 / PAR_CHASE_BPM;
/// Number of chase steps in the 4/4 pattern.
const PAR_CHASE_STEPS: u64 = 4;

// ─── PixStrobe "Digital Rain" ───────────────────────────────────────────────

/// Directional digital rain that sweeps from inside (stage centre) outward.
///
/// `inside_at_col0 = true`  → PixStrobe2 (right side): col 0 is the inside edge.
/// `inside_at_col0 = false` → PixStrobe1 (left  side): col 3 is the inside edge.
struct DigitalRain {
    inside_at_col0: bool,
}

impl Effect for DigitalRain {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 32];

        // ── Blinder swell (channels 0–3 = WW, 4–7 = CW stays 0) ──────────
        // Uses (1 − cos(2πt/T)) / 2 so the wave starts at 0, peaks at T/2.
        let ww = BLINDER_PEAK * 0.5
            * (1.0 - (std::f64::consts::TAU * ctx.time / BLINDER_PERIOD_S).cos()) as f32;
        for i in 0..4 {
            out[i] = ww.max(0.0);
        }
        // CW channels 4–7 remain 0.

        // ── RGB pixels (channels 8–31) ────────────────────────────────────
        for pixel in 0..8usize {
            let col = pixel / 2; // physical column 0–3
            let row = pixel % 2; // 0 = top, 1 = bottom

            // Distance from the inside edge: 0 = innermost column (leads),
            // 3 = outermost column (lags by 3 × RAIN_COL_PHASE_STEP).
            let dist_from_inside = if self.inside_at_col0 {
                col as f64          // PixStrobe2: col 0 is inside
            } else {
                (3 - col) as f64    // PixStrobe1: col 3 is inside
            };
            let phase_offset = dist_from_inside * RAIN_COL_PHASE_STEP;
            let drop_phase = (ctx.time * RAIN_FREQ + phase_offset).fract();

            // Top pixel: bright when drop_phase ≈ 0, dark near 0.5.
            // Bottom pixel: bright when drop_phase ≈ 0.5, dark near 0 and 1.
            let cos_base = std::f64::consts::TAU * drop_phase;
            let brightness = if row == 0 {
                ((0.5 + 0.5 * cos_base.cos()) as f32).max(0.0)
            } else {
                ((0.5 + 0.5 * (cos_base + std::f64::consts::PI).cos()) as f32).max(0.0)
            };

            let intensity = RAIN_BG + brightness * RAIN_INTENSITY;
            let ch = 8 + pixel * 3;
            out[ch]     = RAIN_R * intensity;
            out[ch + 1] = RAIN_G * intensity;
            out[ch + 2] = RAIN_B * intensity;
        }

        out
    }
}

// ─── QuadPhase "Chaos" – slow colour fade ────────────────────────────────────

struct QuadPhaseChaos;

impl Effect for QuadPhaseChaos {
    fn channel_count(&self) -> usize {
        4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 4];

        // Colour channel linearly sweeps 0→0.5 over 60 s.
        let phase = (ctx.time / QP_COLOR_FADE_PERIOD_S).rem_euclid(1.0);
        let colour = phase as f32 * 0.5;

        out[0] = colour;
        out[1] = QP_ROTATION_FAST;
        out[2] = 0.0;             // strobe: off
        out[3] = QP_SHUTTER_OPEN; // shutter open

        out
    }
}

// ─── RootPar 4/4 chase at 120 BPM ───────────────────────────────────────────

struct RootParChase {
    /// Which step in the 4‑step chase this fixture represents (0–3).
    step: u64,
}

impl Effect for RootParChase {
    fn channel_count(&self) -> usize {
        8
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        // Integer beat index at the requested BPM.
        let beat = (ctx.time / PAR_BEAT_PERIOD_S).floor().max(0.0) as u64;
        let active_step = beat % PAR_CHASE_STEPS;
        let is_active = active_step == self.step;

        // Active step: full dimmer. Inactive: background level so the wall
        // never goes completely dark.
        let dimmer_scale = if is_active { 1.0 } else { 0.0 };

        let mut out = PAR_PURPLE.to_vec();
        out[0] = PAR_DIMMER * dimmer_scale;
        out
    }
}

// ─── Scanner "Counter-Rotating Lighthouse" ───────────────────────────────────
//
// The two scanners sweep pan in opposite directions at the same speed so their
// beams are always pointing symmetrically relative to stage-center. A slow
// tilt sine keeps the beams in the upper half of the room.
// On every 4/4 downbeat the shutter fires a single-frame stab.

/// Full pan sweep period (both scanners complete one full 0→1→0 sweep in this time).
const SCAN_PAN_PERIOD_S: f64 = 8.0;
/// Tilt: slowly rocks between mid-tilt and near-ceiling.
const SCAN_TILT_PERIOD_S: f64 = 12.0;
/// Tilt center (0 = full down, 1 = full up / ceiling).
const SCAN_TILT_CENTER: f32 = 0.6;
/// Tilt rock amplitude around the center.
const SCAN_TILT_AMP: f32 = 0.4;
/// Dimmer level (scanners are bright; keep moderate in a dance context).
const SCAN_DIMMER: f32 = 0.75;

/// How long (seconds) the shutter-stab stays open on the downbeat.
const SCAN_STAB_DURATION_S: f64 = 0.04; // ≈ 1.5 frames at 40 Hz

struct ScannerLighthouse {
    /// When true scanner sweeps pan in reverse (creates the counter-rotation).
    reverse_pan: bool,
}

impl Effect for ScannerLighthouse {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        // Pan: smooth 0→1→0 sine, reversed for the second unit.
        let pan_raw = (0.5 + 0.3 * (std::f64::consts::TAU * ctx.time / SCAN_PAN_PERIOD_S).sin()) as f32;
        let pan = if self.reverse_pan { 1.0 - pan_raw } else { pan_raw };

        // Tilt: gentle sine staying high to avoid faces.
        let tilt = SCAN_TILT_CENTER
            + SCAN_TILT_AMP
                * (std::f64::consts::TAU * ctx.time / SCAN_TILT_PERIOD_S).sin() as f32;

        // Beat-synced shutter stab: open for SCAN_STAB_DURATION_S on each downbeat.
        let t_in_beat = ctx.time.rem_euclid(PAR_BEAT_PERIOD_S);
        //let shutter = if t_in_beat < SCAN_STAB_DURATION_S {
        //    SC_SHUTTER_OPEN
        //} else {
        //    SC_SHUTTER_CLOSED
        //};
        let shutter = SC_SHUTTER_OPEN;
        scanner_frame(
            pan,
            tilt,
            SC_COLOR_GREEN,
            shutter,
            SCAN_DIMMER,
            SC_GOBO_2,          // dotted/breakup gobo for matrix-rain feel
            SC_GOBOROT_MED_POS, // slow spin keeps the breakup pattern moving
            SC_PRISM_OFF,       
            SC_FOCUS_MID,
        )
    }
}

// ─── Theme factory ──────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            // PixStrobe1 = left side: inside edge is col 3 (rightmost).
            Binding::single(&fixtures::PIXSTROBE_1, DigitalRain { inside_at_col0: false }),
            // PixStrobe2 = right side: inside edge is col 0 (leftmost).
            Binding::single(&fixtures::PIXSTROBE_2, DigitalRain { inside_at_col0: true }),
            // QuadPhase units share the same colour fade for a cohesive chaos.
            Binding::single(&fixtures::QUADPHASE_1, QuadPhaseChaos),
            Binding::single(&fixtures::QUADPHASE_2, QuadPhaseChaos),
            // RootPars run a simple 4/4 chase at 120 BPM across the bar.
            Binding::single(&fixtures::ROOTPAR_1, RootParChase { step: 0 }),
            Binding::single(&fixtures::ROOTPAR_2, RootParChase { step: 1 }),
            Binding::single(&fixtures::ROOTPAR_3, RootParChase { step: 2 }),
            Binding::single(&fixtures::ROOTPAR_4, RootParChase { step: 3 }),
            // Scanners: counter-rotating lighthouse in green + prism scatter.
            Binding::single(&fixtures::SCANNER_1, ScannerLighthouse { reverse_pan: false }),
            Binding::single(&fixtures::SCANNER_2, ScannerLighthouse { reverse_pan: true }),
        ],
    )
}
