//! # Phase 2 – "Golden Afternoon"
//! **Hours**: 17:00 – 19:00   **Audience**: Work-to-after-work transition.
//!
//! ## Design intent
//! The shift from daytime to evening is psychological as much as physical.
//! Dropping intensity to 50–60 % signals to the brain that the pace is
//! slowing down.  Simultaneously, the colour temperature drifts from the
//! crisp 2700 K of the work phase toward a rich amber-gold.  This warms the
//! glassware and beer taps, making them sparkle without making the room feel
//! theatrical – it still looks like "natural late-afternoon light".
//!
//! ### The Tea Party
//! A very slow, room-wide sine tide shifts the entire ceiling from warm white
//! toward liquid gold over a two-minute period.  Rather than a harsh cut or a
//! single synchronised wave, each spot carries a small golden-angle phase
//! offset so the shift ripples gently from one end of the room to the other –
//! like light reflected from moving water.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, organic_lfo, Rgbw, GOLDEN_ANGLE};

pub const NAME: &str = "Golden Afternoon";

// ─── Colour constants ─────────────────────────────────────────────────────

/// Warm white anchor at this phase's reduced intensity.
/// Still no green/blue – the colour temperature prescription does not change,
/// only the intensity and the amber lift.
const WARM_WHITE: Rgbw = Rgbw::new(0.15, 0.0, 0.0, 1.0);

/// Gold/amber colour point: strong red, moderate green for yellow-gold,
/// no blue, reduced white so the W LED does not wash out the warm tones.
const GOLD_AMBER: Rgbw = Rgbw::new(1.0, 0.35, 0.0, 0.20);

/// Overall intensity for this phase: 55 % (mid-range of 50–60 %).
const INTENSITY: f32 = 0.8;

/// Tea-Party tide period in seconds.  Two minutes is slow enough to be
/// felt rather than noticed – perfect for an unconscious mood transition.
const TIDE_PERIOD_S: f64 = 12.0;

/// The colour mix oscillates between these two fractions of GOLD_AMBER.
/// At `GOLD_LO` we are almost entirely warm white; at `GOLD_HI` the room
/// has gone full liquid-gold.  We never reach pure amber – that would be
/// too much for the transition hour.
const GOLD_LO: f32 = 0.10; // 10 % amber → warm white dominates
const GOLD_HI: f32 = 0.60; // 60 % amber → rich golden light

/// Tiny LFO that adds a second layer of organic life to the lounge,
/// so the lounge and the bar do not feel completely identical.
const SUNLIGHT_FREQ: f64 = 0.059;

// ─── "The Tea Party" effect ───────────────────────────────────────────────

/// Slow golden tide that drifts every ceiling spot between warm white and
/// amber-gold.  The per-spot phase ripple makes the shift travel across
/// the room rather than blink in all at once.
///
/// Outputs 40 channels (10 × RGBW).
struct TeaParty {
    /// Extra per-spot LFO for organic life (lounge) or 0.0 (bar, static tide only).
    sunlight_mix: f32,
}

impl Effect for TeaParty {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mix = self.sunlight_mix;

        build_ceiling(|spot| {
            // Tide: a very slow sine that applies equally to all spots but
            // arrives slightly later for each successive spot (ripple effect).
            let spot_phase = spot as f64 * GOLDEN_ANGLE * 0.25; // gentle ripple
            let omega = std::f64::consts::TAU / TIDE_PERIOD_S;
            let tide = (0.5 + 0.5 * (omega * ctx.time + spot_phase).sin()) as f32;

            // Gold fraction oscillates between GOLD_LO and GOLD_HI.
            let gold_frac = GOLD_LO + tide * (GOLD_HI - GOLD_LO);

            // Optional organic shimmer layered on top (lounge only).
            let shimmer = organic_lfo(ctx.time, spot, SUNLIGHT_FREQ) * 0.04 * mix;

            // Mix colours and apply intensity + shimmer.
            let base_intensity = INTENSITY + shimmer;
            WARM_WHITE.lerp(GOLD_AMBER, gold_frac).scale(base_intensity)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 4_000 },
        vec![
            // Bar ceiling: pure slow golden tide, no shimmer.
            // The glassware and beer taps benefit from consistent, predictable light.
            Binding::single(&fixtures::BAR_DECKE, TeaParty { sunlight_mix: 0.0 }),
            // Lounge ceiling: golden tide + a whisper of organic shimmer.
            Binding::single(&fixtures::LOUNGE_DECKE, TeaParty { sunlight_mix: 1.0 }),
        ],
    )
}
