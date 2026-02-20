//! # Phase 1 – "The Mad Hatter's Workspace"
//! **Hours**: 11:00 – 17:00   **Audience**: Notebook workers, coffee drinkers.
//!
//! ## Design intent
//! The bar is full of people who need to see their laptop screens clearly and
//! make their coffee look appetising.  That means **maximum CRI, minimum colour
//! cast**.  We do not touch the RGB channels of the ceiling spots; the white
//! channel does the heavy lifting, and a small dose of red shifts the correlated
//! colour temperature from "office cool" toward a comfortable 2700–3000 K.
//!
//! ### Sunlight Through Trees
//! A perfectly static room feels dead.  Adding a barely-perceptible, organic
//! intensity shimmer to the lounge ceiling creates the same subconscious
//! comfort as sitting near a window – without distracting anyone from their
//! work.  Each of the 10 spots floats independently between 70 % and 85 %
//! driven by two superimposed sine waves at incommensurate frequencies
//! (≈ 17 s and ≈ 27 s).  Because the frequencies are irrational multiples of
//! each other the pattern never repeats, making the motion feel genuinely alive.

use crate::effect::{Constant, Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, organic_lfo, uniform_ceiling, Rgbw};

pub const NAME: &str = "Mad Hatter's Workspace";

// ─── Colour constants ─────────────────────────────────────────────────────

/// Base white for the ceiling.
const BASE_WHITE: Rgbw = Rgbw::new(0.0, 0.0, 0.0, 1.0);

/// Highlite white for the ceiling.
const WHITE: Rgbw = Rgbw::new(0.4, 0.4, 0.5, 0.0);

/// Bar ceiling intensity.  80 % keeps the counter bright without being harsh.
const BAR_INTENSITY: f32 = 0.80;

/// Lounge intensity range for the sunlight flicker [min, max].
const LOUNGE_LO: f32 = 0.25;
const LOUNGE_HI: f32 = 1.0;

/// Primary LFO frequency: ≈ 17-second period.
/// Slow enough to be subliminal, fast enough to feel alive.
const FREQ_A: f64 = 0.059*5.0;

// ─── "Sunlight Through Trees" effect ─────────────────────────────────────

/// Per-spot intensity drift that mimics dappled sunlight filtering through
/// leaves.  Outputs 40 channels (10 × RGBW), one per ceiling spot.
///
/// The organic LFO uses two incommensurate sines seeded with the golden angle,
/// giving every spot a uniquely-phased, never-repeating breathe cycle.
struct SunlightThroughTrees;

impl Effect for SunlightThroughTrees {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mid = (LOUNGE_LO + LOUNGE_HI) * 0.5;
        let half = (LOUNGE_HI - LOUNGE_LO) * 0.5;

        build_ceiling(|spot| {
            // organic_lfo ∈ [-1, 1]; mapped to [LOUNGE_LO, LOUNGE_HI].
            let intensity = mid + organic_lfo(ctx.time, spot, FREQ_A) * half;
            WHITE.scale(intensity).add(BASE_WHITE)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 3_000 },
        vec![
            // Bar ceiling: static warm white – no theatrics, just good light.
            Binding::single(
                &fixtures::BAR_DECKE,
                Constant::new(uniform_ceiling(WHITE.scale(BAR_INTENSITY))),
            ),
            // Lounge ceiling: "Sunlight Through Trees".
            Binding::single(&fixtures::LOUNGE_DECKE, SunlightThroughTrees),
        ],
    )
}
