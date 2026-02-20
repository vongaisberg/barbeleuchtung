//! # FX Theme 2 – "The Rabbit Hole"
//! **Hours**: 19:00 – close   **Audience**: Evening bar crowd.
//!
//! ## Design intent
//! High energy but inviting rather than attacking.
//!
//! - **PixStrobes – "The Cheshire Grin"**: A pink/magenta-to-teal/cyan gradient
//!   scrolls slowly across the 8 RGB pixels (≈ 20 s per full cycle).  Every
//!   45 s a CW blinder ripple sweeps outward from the center of the combined
//!   panel: PixStrobe 1 sweeps right→left, PixStrobe 2 sweeps left→right, so
//!   both units radiate outward simultaneously from the middle of the stage.
//!   Intensity 25 % – these fixtures are very bright; keep them dim so people
//!   can look directly at them.
//!
//! - **QuadPhase – "Vortex"**: Slow/medium continuous rotation in deep blue.
//!   Smooth beams cutting through the room create atmosphere without lighting
//!   up faces.  Makes the empty space in front of the stage feel like a
//!   dancefloor waiting to happen.
//!
//! - **LEDPars (RootPars)**: Deep purple backdrop at 40 %.  Against this the
//!   PixStrobe pink/teal colours pop dramatically.
//!
//! ### Physical pixel layout (PixStrobe)
//! ```text
//! col:       0       1       2       3
//! top (row 0):  px1    px3    px5    px7
//! bot (row 1):  px2    px4    px6    px8
//! ```
//! Pixel index in DMX (0-based) = col × 2 + row.

use crate::effect::{Constant, Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

pub const NAME: &str = "The Cheshire Grin";

// ─── Gradient colours ──────────────────────────────────────────────────────

const MAGENTA: (f32, f32, f32) = (1.0, 0.00, 0.80);
const TEAL:    (f32, f32, f32) = (0.0, 0.80, 1.00);

/// Overall pixel intensity (25 %).  These panels are very bright; keep low.
const GRADIENT_INTENSITY: f32 = 0.25;

/// Gradient scroll cycle length in seconds.
/// One full MAGENTA→TEAL→MAGENTA sine wave completes in this time.
const SCROLL_PERIOD_S: f64 = 10.0;

// ─── Ripple ("enticing trick") parameters ──────────────────────────────────

/// How often the sweep fires (seconds).  Adjust to taste (30–60 s range).
const RIPPLE_PERIOD_S: f64 = 10.0;

/// Extra delay (seconds) added per column so the pulse travels left → right.
/// 0.08 s / column → full panel sweep takes 3 × 0.08 = 0.24 s.
const RIPPLE_PROP_S: f64 = 0.08;

/// Linear ramp-up duration in seconds.
const RIPPLE_RISE_S: f64 = 0.15;
/// Exponential-decay duration in seconds (total visible window = rise + this).
const RIPPLE_DECAY_S: f64 = 0.35;
/// Decay exponent: e^{−k·t} ≈ 0.03 at t = 0.35 s with k = 10.
const RIPPLE_K: f64 = 10.0;
/// Peak brightness of the CW blinder strip during the ripple sweep.
/// Keep low – the center strip is very bright even at a few percent.
const RIPPLE_CW_PEAK: f32 = 1.00;

// ─── QuadPhase DMX constants ────────────────────────────────────────────────
// ⚠  These are approximate values – verify against your fixture's DMX chart.

/// Shutter fully open (beam on).
/// Common convention: 255 = open.  If your fixture uses 0 = open, set to 0.0.
const QP_SHUTTER_OPEN: f32 = 1.0;

/// Color wheel: deep blue.
/// Typical moonflower color wheels vary by model.  DMX ≈ 170 is a starting
/// point; adjust until you see the blue beam.
const QP_COLOR_DEEP_BLUE: f32 = 0.667; // ≈ DMX 170

/// Rotation: slow-medium continuous spin.
/// DMX ≈ 130–150 for most fixtures (direction/speed depends on model).
const QP_ROTATION_SLOW: f32 = 0.549; // ≈ DMX 140

// ─── LEDPar output ──────────────────────────────────────────────────────────

const PAR_DIMMER: f32 = 0.40;

/// [Dimmer, Strobe, R, G, B, W, Amber, UV] – deep purple.
const PAR_PURPLE: [f32; 8] = [PAR_DIMMER, 0.0, 0.70, 0.0, 1.0, 0.0, 0.0, 0.0];

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Physical column index (0 = leftmost) from 0-based pixel index.
/// Layout: pixel_idx 0,1 → col 0; 2,3 → col 1; 4,5 → col 2; 6,7 → col 3.
#[inline]
fn pixel_col(pixel_idx: usize) -> usize {
    pixel_idx / 2
}

#[inline]
fn lerp_rgb(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

/// CW intensity for the ripple sweep on physical column `col` (0 = first to fire)
/// at time `t`.  Pass columns in the desired fire order; the caller is
/// responsible for reversing the order when the ripple travels right→left.
fn ripple_cw(t: f64, col: usize) -> f32 {
    let t_cycle = t.rem_euclid(RIPPLE_PERIOD_S);
    let t_col   = t_cycle - col as f64 * RIPPLE_PROP_S;

    if t_col < 0.0 || t_col > RIPPLE_RISE_S + RIPPLE_DECAY_S {
        return 0.0;
    }
    if t_col < RIPPLE_RISE_S {
        (t_col / RIPPLE_RISE_S) as f32 * RIPPLE_CW_PEAK
    } else {
        (-RIPPLE_K * (t_col - RIPPLE_RISE_S)).exp() as f32 * RIPPLE_CW_PEAK
    }
}

// ─── PixStrobe "The Cheshire Grin" ─────────────────────────────────────────

/// Scrolling pink/magenta → teal/cyan gradient with a sweeping CW ripple.
///
/// `reverse_ripple`: when `true` the ripple travels right→left (col 3 fires
/// first), so PixStrobe 1 (inner/stage side) and PixStrobe 2 together produce
/// an outward-radiating sweep from the center of the combined panel.
struct CheshireGrin {
    reverse_ripple: bool,
}

impl Effect for CheshireGrin {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 32];

        // ── CW blinder strip (channels 4–7): sweeping ripple ─────────────
        // CW 1–4 correspond to physical columns 0–3 (left→right).
        // `fire_col` maps the physical column to its fire order: for a
        // right→left sweep (reverse) col 3 fires first, so fire_col = 3−col.
        for col in 0..4usize {
            let fire_col = if self.reverse_ripple { 3 - col } else { col };
            out[4 + col] = ripple_cw(ctx.time, fire_col);
        }
        // WW channels 0–3: held at zero.

        // ── RGB pixels (channels 8–31): scrolling colour gradient ─────────
        // A sinusoidal wave replaces the previous fract()-based approach.
        // sin() is naturally periodic → the colour transition through
        // MAGENTA → mid → TEAL → mid → MAGENTA is always smooth with no
        // abrupt jumps, no matter how far time has advanced.
        // Subtracting time/period makes the wave travel rightward.
        for pixel in 0..8usize {
            let col = pixel_col(pixel);

            let grad_phase = std::f64::consts::TAU
                * (col as f64 / 3.0 - ctx.time / SCROLL_PERIOD_S);
            let grad_t = (0.5 + 0.5 * grad_phase.sin()) as f32;
            let (r, g, b) = lerp_rgb(MAGENTA, TEAL, grad_t);

            let ch = 8 + pixel * 3;
            out[ch]     = r * GRADIENT_INTENSITY;
            out[ch + 1] = g * GRADIENT_INTENSITY;
            out[ch + 2] = b * GRADIENT_INTENSITY;
        }

        out
    }
}

// ─── Theme factory ──────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    let par  = || Constant::new(PAR_PURPLE.to_vec());
    let quad = || Constant::new(vec![
        QP_COLOR_DEEP_BLUE,
        QP_ROTATION_SLOW,
        0.0,            // strobe: off
        QP_SHUTTER_OPEN,
    ]);

    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            // PixStrobe 1 ripples right→left (center outward on its side).
            // PixStrobe 2 ripples left→right (center outward on its side).
            // Together they radiate outward from the center of the combined panel.
            Binding::single(&fixtures::PIXSTROBE_1, CheshireGrin { reverse_ripple: true }),
            Binding::single(&fixtures::PIXSTROBE_2, CheshireGrin { reverse_ripple: false }),
            Binding::single(&fixtures::QUADPHASE_1, quad()),
            Binding::single(&fixtures::QUADPHASE_2, quad()),
            Binding::single(&fixtures::ROOTPAR_1, par()),
            Binding::single(&fixtures::ROOTPAR_2, par()),
            Binding::single(&fixtures::ROOTPAR_3, par()),
            Binding::single(&fixtures::ROOTPAR_4, par()),
        ],
    )
}
