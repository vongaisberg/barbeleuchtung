//! # FX Theme 5 – "The Jabberwocky"
//!
//! A predatory green-and-purple creature stalks through the Tulgey Wood.
//! All-organic, no rigid beat-grid: everything is driven by LFOs so the
//! atmosphere shifts without needing a DJ.
//!
//! ## Design intent
//! Menacing but beautiful. Works as a wild card late-night scene when people
//! want something unsettling rather than a straight dance loop.
//!
//! - **RootPars**: Slow green↔purple crossfade with a rare, soft brightness
//!   swell seeded from the golden angle (no hard strobe – just a gentle lift).
//!
//! - **PixStrobes**: Per-pixel green/purple twinkle using `organic_lfo`.
//!   Every ≈ 45 s a soft white "lightning" swell rises and decays on the CW
//!   blinder – present but no longer a sharp flash.
//!
//! - **QuadPhases**: Medium rotation. Color wheel slowly walking green→blue over
//!   90 s so the beams feel alive without being frantic.
//!
//! - **Scanners**: "Searchlight Eyes" – slow drift with sudden snaps to new
//!   positions, independent golden-angle phase offsets between the two units.
//!   Breakup gobo, prism rotating, green base color. The two converge on the
//!   same point every ≈ 30 s for a 3 s "lock" moment, then split again.

use std::f64::consts::TAU;

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    organic_lfo, scanner_frame, shaped_lfo, SC_COLOR_GREEN, SC_COLOR_KGREEN, SC_FOCUS_MID,
    SC_GOBO_5, SC_GOBOROT_FAST_POS, SC_PRISM_OFF, SC_SHUTTER_CLOSED, SC_SHUTTER_OPEN,
};

pub const NAME: &str = "The Jabberwocky";

// ─── RootPar crossfade + sparkle ─────────────────────────────────────────────

const PAR_DIMMER: f32 = 0.45;
const PAR_XFADE_PERIOD_S: f64 = 25.0; // slow green↔purple oscillation
/// [Dimmer, Strobe, R, G, B, W, Amber, UV]
const PAR_GREEN:  [f32; 8] = [PAR_DIMMER, 0.0, 0.10, 1.0, 0.10, 0.0, 0.0, 0.0];
const PAR_PURPLE: [f32; 8] = [PAR_DIMMER, 0.0, 0.70, 0.0, 1.00, 0.0, 0.0, 0.0];

struct JabberRootPar {
    phase_offset: f64,
}

impl Effect for JabberRootPar {
    fn channel_count(&self) -> usize { 8 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = ctx.time + self.phase_offset;
        let blend = (0.5 + 0.5 * (TAU * t / PAR_XFADE_PERIOD_S).sin()) as f32;

        let mut out = [0.0f32; 8];
        for i in 0..8 {
            out[i] = PAR_GREEN[i] * (1.0 - blend) + PAR_PURPLE[i] * blend;
        }

        // Soft swell: a rare, gentle brightness lift (no hard strobe channel).
        // Lower frequency + higher threshold make the swells infrequent, and we
        // only nudge the dimmer instead of slamming it + firing the strobe.
        let sparkle = shaped_lfo(ctx.time, (self.phase_offset * 10.0) as usize, 3.0, 5.0);
        if sparkle > 0.97 {
            // Ease the dimmer up toward a soft peak rather than a hit.
            let lift = (sparkle - 0.97) / 0.03; // 0→1 across the top of the spike
            out[0] = PAR_DIMMER + (0.70 - PAR_DIMMER) * lift;
        }

        out.to_vec()
    }
}

// ─── PixStrobe: green/purple twinkle + lightning flash ───────────────────────

/// Background intensity (pixels never fully dark).
const PIX_BG: f32 = 0.02;
/// Peak twinkle brightness per pixel.
const PIX_PEAK: f32 = 0.40;
const PIX_LFO_FREQ: f64 = 0.18; // slow organic breathing
 
/// Lightning: a soft white swell on the CW blinder, ≈ every 45 s.
/// Rarer, lower-peak, and slower-decaying than a sharp flash so it reads as a
/// gentle pulse of light rather than a strobe hit.
const LIGHTNING_PERIOD_S: f64 = 45.0; // far rarer than before
const LIGHTNING_RISE_S: f64  = 0.20;  // gentle ramp-up
const LIGHTNING_DECAY_S: f64 = 0.60;  // slow fade-out
const LIGHTNING_K: f64 = 7.0;         // softer decay curve
const LIGHTNING_PEAK: f32 = 0.45;     // never full white

fn lightning_cw(t: f64) -> f32 {
    let t_cycle = t.rem_euclid(LIGHTNING_PERIOD_S);
    let raw = if t_cycle < LIGHTNING_RISE_S {
        (t_cycle / LIGHTNING_RISE_S) as f32
    } else if t_cycle < LIGHTNING_RISE_S + LIGHTNING_DECAY_S {
        let decay_t = t_cycle - LIGHTNING_RISE_S;
        (-LIGHTNING_K * decay_t).exp() as f32
    } else {
        0.0
    };
    raw * LIGHTNING_PEAK
}

struct JabberPixStrobe {
    pixel_seed: usize,
}

impl Effect for JabberPixStrobe {
    fn channel_count(&self) -> usize { 32 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 32];

        // CW blinder: lightning flash (channels 4–7).
        let cw = lightning_cw(ctx.time);
        for i in 4..8 { out[i] = cw; }
 
        // RGB pixels: organic green/purple twinkle.
        for pixel in 0..8usize {
            let lfo = organic_lfo(ctx.time, self.pixel_seed + pixel, PIX_LFO_FREQ);
            let t = (lfo + 1.0) * 0.5;
            let intensity = PIX_BG + t * (PIX_PEAK - PIX_BG);

            // Alternate between green and purple per pixel using a slow secondary LFO.
            let color_t = organic_lfo(ctx.time * 0.5, self.pixel_seed + pixel + 100, 0.07);
            let color_t = (color_t + 1.0) * 0.5; // [0,1]

            let r = (0.10 * (1.0 - color_t) + 0.70 * color_t) * intensity;
            let g = (1.00 * (1.0 - color_t) + 0.00 * color_t) * intensity;
            let b = (0.10 * (1.0 - color_t) + 1.00 * color_t) * intensity;

            let ch = 8 + pixel * 3;
            out[ch]     = r;
            out[ch + 1] = g;
            out[ch + 2] = b;
        }

        out
    }
}

// ─── QuadPhase: fast rotation, slow color walk ───────────────────────────────

const QP_SHUTTER_OPEN: f32  = 1.0;
const QP_ROTATION_FAST: f32 = 0.45; // medium spin – calmer than before
const QP_COLOR_PERIOD_S: f64 = 90.0; // full color-wheel sweep in 90 s

struct JabberQuadPhase;

impl Effect for JabberQuadPhase {
    fn channel_count(&self) -> usize { 4 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        // Walk the color wheel from green to blue and back (≈ first quarter).
        let phase = (ctx.time / QP_COLOR_PERIOD_S).rem_euclid(1.0) as f32;
        let color = SC_COLOR_GREEN + (SC_COLOR_KGREEN - SC_COLOR_GREEN) * phase;
        vec![color, QP_ROTATION_FAST, 0.0, QP_SHUTTER_OPEN]
    }
}

// ─── Scanner: "Searchlight Eyes" ─────────────────────────────────────────────
//
// Slow drift on a low-frequency LFO (organic, independent per unit).
// Periodically a "snap" envelope kicks in: the LFO phase is frozen and the
// scanner jumps to a snap position, then the drift resumes.
// Every LOCK_PERIOD_S both scanners converge on stage-center for LOCK_DURATION_S.

/// Drift LFO frequency (≈ one wandering cycle per 20 s).
const SCAN_DRIFT_FREQ: f64 = 0.05;
/// Pan range for the drift [center ± amp].
const SCAN_PAN_CENTER: f32  = 0.50;
const SCAN_PAN_AMP: f32     = 0.35;
/// Tilt range (always in upper half to avoid eye-level).
const SCAN_TILT_CENTER: f32 = 0.60;
const SCAN_TILT_AMP: f32    = 0.20;

/// How often (s) the scanner snaps to a new position.
const SNAP_PERIOD_S: f64  = 7.3; // irrational to avoid grid feel
/// Duration of shutter close during a snap (position changes while blacked out).
const SNAP_BLACKOUT_S: f64 = 0.12;

/// Lock: both scanners converge to center every LOCK_PERIOD_S.
const LOCK_PERIOD_S: f64   = 31.0; // prime-ish, no relation to BPM
const LOCK_DURATION_S: f64 = 3.5;
const LOCK_PAN: f32  = 0.50;
const LOCK_TILT: f32 = 0.55; // slightly below ceiling

const SCAN_DIMMER: f32 = 0.70;

struct JabberScanner {
    /// Phase seed index for organic_lfo (different per unit).
    seed: usize,
}

impl Effect for JabberScanner {
    fn channel_count(&self) -> usize { 11 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        // Lock window?
        let t_lock = ctx.time.rem_euclid(LOCK_PERIOD_S);
        let in_lock = t_lock < LOCK_DURATION_S;

        let (pan, tilt, shutter) = if in_lock {
            // Both units converge to the same center point.
            // Fade in/out gently using a sine envelope so the "lock" is smooth.
            let lock_env = (TAU * t_lock / (LOCK_DURATION_S * 2.0)).sin().abs() as f32;
            let pan  = LOCK_PAN  * lock_env + SCAN_PAN_CENTER  * (1.0 - lock_env);
            let tilt = LOCK_TILT * lock_env + SCAN_TILT_CENTER * (1.0 - lock_env);
            (pan, tilt, SC_SHUTTER_OPEN)
        } else {
            // Snap blackout: close shutter at the start of each snap period.
            let t_in_snap = ctx.time.rem_euclid(SNAP_PERIOD_S);
            let shutter = if t_in_snap < SNAP_BLACKOUT_S {
                SC_SHUTTER_CLOSED
            } else {
                SC_SHUTTER_OPEN
            };

            // Drift: organic LFO for pan and tilt, independent axes.
            let pan_lfo  = organic_lfo(ctx.time, self.seed,      SCAN_DRIFT_FREQ);
            let tilt_lfo = organic_lfo(ctx.time, self.seed + 50, SCAN_DRIFT_FREQ * 0.7);

            let pan  = SCAN_PAN_CENTER  + SCAN_PAN_AMP  * pan_lfo;
            let tilt = SCAN_TILT_CENTER + SCAN_TILT_AMP * tilt_lfo;
            (pan, tilt, shutter)
        };

        scanner_frame(
            pan,
            tilt,
            SC_COLOR_GREEN,
            shutter,
            SCAN_DIMMER,
            SC_GOBO_5,         // foliage/star breakup gobo
            SC_GOBOROT_FAST_POS, // rotating gobo for organic pattern movement
            SC_PRISM_OFF,      
            SC_FOCUS_MID,
        )
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    use crate::themes::GOLDEN_ANGLE;

    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            Binding::single(&fixtures::PIXSTROBE_1, JabberPixStrobe { pixel_seed: 0 }),
            Binding::single(&fixtures::PIXSTROBE_2, JabberPixStrobe { pixel_seed: 8 }),
            Binding::single(&fixtures::QUADPHASE_1, JabberQuadPhase),
            Binding::single(&fixtures::QUADPHASE_2, JabberQuadPhase),
            // RootPars have golden-angle phase offsets so they drift independently.
            Binding::single(&fixtures::ROOTPAR_1, JabberRootPar { phase_offset: 0.0 }),
            Binding::single(&fixtures::ROOTPAR_2, JabberRootPar { phase_offset: GOLDEN_ANGLE }),
            Binding::single(&fixtures::ROOTPAR_3, JabberRootPar { phase_offset: GOLDEN_ANGLE * 2.0 }),
            Binding::single(&fixtures::ROOTPAR_4, JabberRootPar { phase_offset: GOLDEN_ANGLE * 3.0 }),
            // Scanners: independent seeds so they never drift in unison.
            Binding::single(&fixtures::SCANNER_1, JabberScanner { seed: 0 }),
            Binding::single(&fixtures::SCANNER_2, JabberScanner { seed: 200 }),
        ],
    )
}
