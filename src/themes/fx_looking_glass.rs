//! # FX Theme 1 – "The Looking Glass"
//! **Hours**: 11:00 – 17:00   **Audience**: Daytime café, notebook workers.
//!
//! ## Design intent
//! Kinetic art: subtle movement that catches the eye without distracting.
//!
//! - **PixStrobes – "Glinting Gems"**: Only the 8 RGB pixels are used; the WW/CW
//!   blinder strip stays dark.  Each pixel independently breathes between a dim
//!   warm-white base (5 %) and a 30 % peak using an organic two-sine LFO seeded
//!   with the golden angle, so no two pixels rise and fall in sync.  The result
//!   looks like sunlight glinting off a mirror or scattered jewels.
//!
//! - **QuadPhase**: OFF.  Rotating beams look cheap in daylight.
//!
//! - **LEDPars (RootPars)**: Static amber/orange at 20 % – just enough warmth on
//!   the back wall to keep it from being a black void.
//!
//! - **Scanners**: A single very slow, white, pseudo-random beam per unit drifts
//!   across the room (≈ 60–70 s wander) using the organic two-sine LFO, kept high
//!   to stay off faces.  Open gobo, no rotation, no prism – just a quiet moving
//!   accent that reads as kinetic art rather than a club effect.

use crate::effect::{Constant, Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{
    organic_lfo, scanner_frame, SC_COLOR_WHITE, SC_FOCUS_MID, SC_GOBO_OPEN, SC_GOBOROT_NONE,
    SC_PRISM_OFF, SC_SHUTTER_OPEN,
};

pub const NAME: &str = "The Looking Glass";

// ─── Pixel colours ─────────────────────────────────────────────────────────

/// Warm-white tint in RGB (no W channel on PixStrobe pixels).
const WARM_R: f32 = 1.00;
const WARM_G: f32 = 0.75;
const WARM_B: f32 = 0.30;

/// Dim background intensity – pixel at rest.
const GEM_BASE: f32 = 0.05;
/// Peak intensity of a glinting pixel.
const GEM_PEAK: f32 = 0.40;

/// Primary LFO frequency: quasi-period ≈ 4 s (0.25 Hz).
/// Each pixel breathes up and back down in roughly 2–3 s on average.
const GEM_FREQ: f64 = 0.25;

// ─── LEDPar output ──────────────────────────────────────────────────────────

/// Master dimmer for Phase-1 RootPars.
const PAR_DIMMER: f32 = 0.20;

/// 8-channel RootPar: [Dimmer, Strobe, R, G, B, W, Amber, UV].
const PAR_AMBER: [f32; 8] = [PAR_DIMMER, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

// ─── PixStrobe "Glinting Gems" ──────────────────────────────────────────────

/// Independent per-pixel shimmer on the 8 RGB segments.
///
/// `pixel_seed` offsets the LFO spot index so the two PixStrobe units look
/// different even though they run the same algorithm.
struct GlintingGems {
    pixel_seed: usize,
}

impl Effect for GlintingGems {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 32];

        // Channels 0–7 (WW + CW blinder strip): held at zero per spec.

        for pixel in 0..8usize {
            let lfo = organic_lfo(ctx.time, self.pixel_seed + pixel, GEM_FREQ);
            // Map organic_lfo ∈ [-1, 1] → intensity ∈ [GEM_BASE, GEM_PEAK].
            let t = (lfo + 1.0) * 0.5;
            let intensity = GEM_BASE + t * (GEM_PEAK - GEM_BASE);

            let ch = 8 + pixel * 3;
            out[ch]     = WARM_R * intensity;
            out[ch + 1] = WARM_G * intensity;
            out[ch + 2] = WARM_B * intensity;
        }

        out
    }
}

// ─── Scanner "Slow Drift" ────────────────────────────────────────────────────

/// Movement LFO frequency (Hz).  ≈ 0.015 Hz ⇒ quasi-period of ~65 s, so the
/// beam wanders very slowly.  The two axes use slightly different rates so the
/// path traces a slowly-evolving Lissajous figure that never quite repeats.
const DRIFT_FREQ: f64 = 0.015;
/// Pan wander range around center (0.5 = straight ahead).
const DRIFT_PAN_CENTER: f32 = 0.50;
const DRIFT_PAN_AMP: f32 = 0.38;
/// Tilt stays in the upper half so the beam never hits seated faces.
const DRIFT_TILT_CENTER: f32 = 0.62;
const DRIFT_TILT_AMP: f32 = 0.16;
/// Beam brightness – gentle; this is an ambient daytime accent.
const DRIFT_DIMMER: f32 = 0.55;

/// Very slow, pseudo-random white beam wander.
///
/// `seed` offsets the LFO spot index so the two scanners drift independently
/// and never mirror each other.
struct ScannerDrift {
    seed: usize,
}

impl Effect for ScannerDrift {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        // Independent pseudo-random LFOs per axis (different seeds + rates).
        let pan = DRIFT_PAN_CENTER
            + DRIFT_PAN_AMP * organic_lfo(ctx.time, self.seed, DRIFT_FREQ);
        let tilt = DRIFT_TILT_CENTER
            + DRIFT_TILT_AMP * organic_lfo(ctx.time, self.seed + 37, DRIFT_FREQ * 0.7);

        scanner_frame(
            pan,
            tilt,
            SC_COLOR_WHITE,
            SC_SHUTTER_OPEN,
            DRIFT_DIMMER,
            SC_GOBO_OPEN,
            SC_GOBOROT_NONE,
            SC_PRISM_OFF,
            SC_FOCUS_MID,
        )
    }
}

// ─── Theme factory ──────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    let par      = || Constant::new(PAR_AMBER.to_vec());
    let quad_off = || Constant::new(vec![0.0; 4]);

    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            // PixStrobe 1 and 2 use different LFO seeds so they look independent.
            Binding::single(&fixtures::PIXSTROBE_1, GlintingGems { pixel_seed: 0 }),
            Binding::single(&fixtures::PIXSTROBE_2, GlintingGems { pixel_seed: 8 }),
            // QuadPhase: shutter closed = all zeros.
            Binding::single(&fixtures::QUADPHASE_1, quad_off()),
            Binding::single(&fixtures::QUADPHASE_2, quad_off()),
            // RootPars: static amber glow.
            Binding::single(&fixtures::ROOTPAR_1, par()),
            Binding::single(&fixtures::ROOTPAR_2, par()),
            Binding::single(&fixtures::ROOTPAR_3, par()),
            Binding::single(&fixtures::ROOTPAR_4, par()),
            // Scanners: very slow pseudo-random white drift (independent seeds).
            Binding::single(&fixtures::SCANNER_1, ScannerDrift { seed: 0 }),
            Binding::single(&fixtures::SCANNER_2, ScannerDrift { seed: 64 }),
        ],
    )
}
