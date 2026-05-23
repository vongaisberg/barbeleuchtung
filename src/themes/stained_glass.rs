//! # "Stained Glass"
//! **Scheduling**: Never – manual selection only.
//!
//! ## Design intent
//! Jewel-tone light cast through a slowly-rotating cathedral window.  Each of
//! the 10 ceiling spots holds a saturated palette colour (ruby, emerald,
//! sapphire, amber, amethyst) and the whole palette migrates one slot
//! sideways across the ceiling per `ROTATION_PERIOD_S`.  After ≈ 2.5 minutes
//! every spot has cycled through every colour.
//!
//! ## What moves
//! Only the **palette index**.  No intensity changes, no shimmer, no glints –
//! the only visible motion is each spot smoothly crossfading to its
//! neighbour's colour and so on around the ring.  The motion is slow enough
//! that on first glance the room reads as a static stained-glass window.
//!
//! ## Bar / Lounge
//! Both fixtures run the identical effect on the same time base, so the two
//! ceilings show the same rotation in lock-step.  No separate bartender white.
//!
//! ## Spot distribution
//! With 10 spots and 5 palette entries we lay 10 evenly-spaced positions
//! across the 5-entry ring (spacing = 0.5 palette-slots) so every adjacent
//! pair of spots is half-way between two colours – maximally varied, no
//! "two spots identical" symmetry artefacts.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, Rgbw, NUM_SPOTS};

pub const NAME: &str = "Stained Glass";

// ─── Jewel palette ────────────────────────────────────────────────────────

static JEWELS: &[Rgbw] = &[
    Rgbw::new(1.00, 0.00, 0.05, 0.0), // ruby
    Rgbw::new(0.00, 1.00, 0.20, 0.0), // emerald
    Rgbw::new(0.00, 0.10, 1.00, 0.0), // sapphire
    Rgbw::new(1.00, 0.50, 0.00, 0.0), // amber
    Rgbw::new(0.70, 0.00, 1.00, 0.0), // amethyst
];

/// Seconds it takes for the palette to migrate by one slot.
/// A full loop for any given spot = `ROTATION_PERIOD_S × JEWELS.len()`
/// ≈ 2 min 30 s with the current values.
const ROTATION_PERIOD_S: f64 = 30.0;

// ─── Effect ───────────────────────────────────────────────────────────────

struct StainedGlass;

impl Effect for StainedGlass {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let n = JEWELS.len() as f64;
        let palette_pos = ctx.time / ROTATION_PERIOD_S;
        // Even spacing across the palette ring so 10 spots map to 10 distinct
        // positions in [0, n).  spot_step = n / NUM_SPOTS = 0.5 for n=5.
        let spot_step = n / NUM_SPOTS as f64;

        build_ceiling(|spot| {
            let pos = palette_pos + spot as f64 * spot_step;
            let pos_mod = pos.rem_euclid(n);
            let idx = pos_mod.floor() as usize;
            let next = (idx + 1) % JEWELS.len();
            let t = pos_mod.fract() as f32;
            JEWELS[idx].lerp(JEWELS[next], t)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 5_000 },
        vec![
            Binding::single(&fixtures::BAR_DECKE, StainedGlass),
            Binding::single(&fixtures::LOUNGE_DECKE, StainedGlass),
        ],
    )
}
