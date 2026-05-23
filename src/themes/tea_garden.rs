//! # "Tea Garden Pastels"
//! **Scheduling**: Never – manual selection only.
//!
//! ## Design intent
//! The Mad Hatter's lawn an hour before sundown: blush, sage, periwinkle and
//! their close pastel cousins, mixed with ~30 % warm-white floor so every
//! colour reads as a soft pastel rather than a saturated jewel.  This is the
//! brightest of the three colourful manual scenes (the W-LED contribution
//! lifts perceived brightness substantially – see
//! `canvases/decke-scene-brightness.canvas.tsx`).
//!
//! ## What moves
//! Only the **hue**.  Each spot oscillates between two adjacent pastel
//! colours via `shaped_lfo` with `k = 3` – the LFO holds at each extreme
//! for ~22 s and crossfades over ~8 s, so each spot looks "settled" most of
//! the time and only briefly slips into its second colour.  Intensity stays
//! constant.  Per-spot golden-angle phase offsets (inside `shaped_lfo`) keep
//! the 10 oscillations decorrelated.
//!
//! ## Bar / Lounge
//! Both fixtures run the identical effect.  No separate bartender white.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, shaped_lfo, Rgbw};

pub const NAME: &str = "Tea Garden Pastels";

// ─── Pastel pairs ─────────────────────────────────────────────────────────
// Each pair stays within one hue zone so the oscillation reads as the spot
// breathing colour, not switching identity.  W = 0.30 across all entries is
// what gives the pastels their "milky" softness.

struct PastelPair {
    a: Rgbw,
    b: Rgbw,
}

static PAIRS: &[PastelPair] = &[
    // blush ↔ peach (warm pink → warm peach)
    PastelPair {
        a: Rgbw::new(0.90, 0.60, 0.60, 0.30),
        b: Rgbw::new(1.00, 0.70, 0.50, 0.30),
    },
    // sage ↔ mint (cool green → cooler green-cyan)
    PastelPair {
        a: Rgbw::new(0.50, 0.90, 0.60, 0.30),
        b: Rgbw::new(0.40, 0.90, 0.70, 0.30),
    },
    // periwinkle ↔ lavender (blue-violet → red-violet)
    PastelPair {
        a: Rgbw::new(0.50, 0.50, 0.90, 0.30),
        b: Rgbw::new(0.70, 0.50, 0.90, 0.30),
    },
];

/// Full LFO period in seconds.  `shaped_lfo` with `k = 3` spends ~80 % of a
/// period near its extremes, so the perceived behaviour is ≈ 22 s settled,
/// ≈ 8 s transition.
const OSCILLATION_PERIOD_S: f64 = 60.0;

/// Shape factor – higher = more "hold at extremes, snap between them".
const OSCILLATION_K: f64 = 3.0;

// ─── Effect ───────────────────────────────────────────────────────────────

struct TeaGarden;

impl Effect for TeaGarden {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        build_ceiling(|spot| {
            let pair = &PAIRS[spot % PAIRS.len()];
            let t = shaped_lfo(ctx.time, spot, OSCILLATION_PERIOD_S, OSCILLATION_K);
            pair.a.lerp(pair.b, t)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 4_000 },
        vec![
            Binding::single(&fixtures::BAR_DECKE, TeaGarden),
            Binding::single(&fixtures::LOUNGE_DECKE, TeaGarden),
        ],
    )
}
