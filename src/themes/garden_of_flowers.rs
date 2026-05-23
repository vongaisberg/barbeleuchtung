//! # "Garden of Live Flowers"
//! **Scheduling**: Never – manual selection only.
//!
//! ## Design intent
//! Reference: the chapter in *Through the Looking-Glass* where Alice meets a
//! flowerbed of talking blooms.  The ceiling is the bouquet seen from above:
//! every spot is a different flower, and the room reads as bright and
//! colourful precisely *because* the 10 spots show 5 distinct hues at once,
//! not because anything is moving quickly.
//!
//! ## What moves
//! Only the **hue**.  Each spot is assigned a base flower colour and a "drift
//! partner" – an adjacent flower in the same hue zone – and gently lerps
//! between them over ~40 s using `organic_lfo`.  A rose spot drifts a small
//! way toward coral and back; it never crosses into a wholly different colour.
//! Intensity stays constant.  Per-spot golden-angle phase offsets (inside
//! `organic_lfo`) keep the 10 wobbles out of sync, so the room never holds
//! a static snapshot.
//!
//! ## Bar / Lounge
//! Both fixtures run the identical effect.  Bartenders share the same flower
//! light as the lounge; no separate functional white.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, organic_lfo, Rgbw};

pub const NAME: &str = "Garden of Live Flowers";

// ─── Flower palette ───────────────────────────────────────────────────────
// Saturated-but-warm flower colours.  A small (W = 0.10) warm-white floor
// keeps reds and pinks from going cold under the cool-cast white LED of the
// fixture; it is *not* an intensity dial.

const ROSE:     Rgbw = Rgbw::new(1.00, 0.20, 0.40, 0.10);
const CORAL:    Rgbw = Rgbw::new(1.00, 0.40, 0.20, 0.10);
const DAFFODIL: Rgbw = Rgbw::new(1.00, 0.80, 0.00, 0.10);
const LILAC:    Rgbw = Rgbw::new(0.60, 0.20, 0.80, 0.10);
const SAGE:     Rgbw = Rgbw::new(0.30, 0.70, 0.40, 0.10);

/// A flower and the colour it sways toward.  Pairs are picked to stay within
/// the same hue zone so each spot looks like it's swaying in place rather
/// than morphing into a different bloom.
struct Flower {
    base: Rgbw,
    drift: Rgbw,
}

static FLOWERS: &[Flower] = &[
    Flower { base: ROSE,     drift: CORAL    }, // pink → orange-pink
    Flower { base: CORAL,    drift: ROSE     }, // orange → back toward pink
    Flower { base: DAFFODIL, drift: CORAL    }, // yellow warms toward amber
    Flower { base: LILAC,    drift: ROSE     }, // purple shifts warmer
    Flower { base: SAGE,     drift: DAFFODIL }, // green shifts slightly yellow
];

/// Primary LFO frequency in `organic_lfo`.  PHI-scaled secondary makes the
/// combined motion incommensurate – every spot's wobble is unique and never
/// exactly repeats.  Period ≈ 40 s.
const WOBBLE_FREQ: f64 = 0.025;

/// Maximum lerp fraction toward the drift partner – kept small so each spot
/// stays recognisably its own flower.
const WOBBLE_AMOUNT: f32 = 0.20;

// ─── Effect ───────────────────────────────────────────────────────────────

struct GardenFlowers;

impl Effect for GardenFlowers {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        build_ceiling(|spot| {
            let f = &FLOWERS[spot % FLOWERS.len()];
            // organic_lfo ∈ [-1, 1]; map to [0, WOBBLE_AMOUNT].
            let lfo01 = (organic_lfo(ctx.time, spot, WOBBLE_FREQ) + 1.0) * 0.5;
            let wobble = lfo01 * WOBBLE_AMOUNT;
            f.base.lerp(f.drift, wobble)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 4_000 },
        vec![
            Binding::single(&fixtures::BAR_DECKE, GardenFlowers),
            Binding::single(&fixtures::LOUNGE_DECKE, GardenFlowers),
        ],
    )
}
