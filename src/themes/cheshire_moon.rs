//! # Phase 3 – "Cheshire Moon"
//! **Hours**: 19:00 – close   **Audience**: Cocktails, dates, conversations.
//!
//! ## Design intent
//! This is where the RGBW capabilities earn their keep.  The walls are painted
//! with Alice-in-Wonderland murals; the goal is to make those colours "vibrate"
//! by surrounding them with complementary light that shifts slowly over time.
//! A teal/cyan light makes warm painted reds and oranges pop; a deep purple
//! makes painted greens and blues disappear into the wall – so parts of the
//! mural literally fade in and out as the room breathes.
//!
//! The bar area stays functional (bartenders need to see) with a dim warm
//! white.  All the drama happens in the lounge.
//!
//! ### The Rabbit Hole (lounge effect)
//! Each of the 10 lounge ceiling spots independently fades between two colours:
//!
//! - **Purple anchor**: `RGB(50, 0, 100)` + 4 % warm white – a deep surreal base.
//! - **Teal visitor**: `RGB(0, 115, 140)` – arrives briefly, then retreats.
//!
//! The transition is shaped by `tanh(3 · sin(ωt + φ))`, which gives:
//! - ≈ 10-second **hold** at each extreme (the spot looks "settled")
//! - ≈  4-second **fade** between them (a smooth wash of colour)
//! - Period ≈ 28 s  →  each spot completes one full purple→teal→purple cycle
//!   in about half a minute.
//!
//! The 10 spots carry golden-angle phase offsets so at any moment the room
//! has spots at every stage of the cycle simultaneously – some in deep purple,
//! some in mid-teal, some mid-transition.  The overall impression is that the
//! room is gently breathing with an alien, bioluminescent life.

use crate::effect::{Constant, Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, shaped_lfo, uniform_ceiling, Rgbw, GOLDEN_ANGLE};

pub const NAME: &str = "Cheshire Moon";

// ─── Colour constants ─────────────────────────────────────────────────────

/// Bar warm white: dim enough for atmosphere, bright enough for bartenders.
/// Strictly no RGB – consistent, reliable, functional.
const BAR_WHITE: Rgbw = Rgbw::new(0.10, 0.0, 0.0, 1.0);
const BAR_INTENSITY: f32 = 0.30;

/// Purple anchor: `RGB(50, 0, 100) / 255` + slight warm white for depth.
/// The warm white component (4 %) prevents the purple from looking cold and
/// keeps it in the "warm club" territory rather than "hospital UV lamp".
const PURPLE: Rgbw = Rgbw::new(0.333, 0.0, 0.666, 0.04);

/// Teal visitor: saturated cyan-teal.  Against the purple background this
/// creates a complementary contrast that makes the wall murals dance.
/// No white component – full saturation for maximum colour impact.
const TEAL: Rgbw = Rgbw::new(0.0, 0.45, 0.55, 0.0);

/// Shaped-LFO parameters.
///
/// Period: 4 s fade + 10 s hold + 4 s fade + 10 s hold = 28 s.
const RABBIT_HOLE_PERIOD_S: f64 = 28.0;

/// Shape factor k in `tanh(k · sin(...))`.
/// k = 3 → ≈ 4 s transition, ≈ 10 s hold per extreme.
const RABBIT_HOLE_K: f64 = 3.0;

// ─── Glint parameters ─────────────────────────────────────────────────────

/// Full-white target for the sparkle peak.
const GLINT_WHITE: Rgbw = Rgbw::new(1.0, 1.0, 1.0, 1.0);

/// Each spot glints once every 120 s.
const GLINT_PERIOD_S: f64 = 60.0*10.0;

/// Linear ramp duration (seconds): 0 → 100 %.
const GLINT_RISE_S: f64 = 0.5;

/// Seconds after the rise at which the exponential tail ends (≈ zero).
/// Total visible glint window: GLINT_RISE_S + GLINT_DECAY_S = 4 s.
const GLINT_DECAY_S: f64 = 5.0;

/// Exponent k in e^{−k·t}: k = 3 → e^{−9} ≈ 0.0001 at t = 3 s.
const GLINT_K: f64 = 2.0;

// ─── Glint envelope ───────────────────────────────────────────────────────

/// Sparkle amplitude ∈ [0, 1] for `spot` at time `t`.
///
/// Once every `GLINT_PERIOD_S` seconds, with golden-angle phase offsets so no
/// two spots fire simultaneously:
///
/// ```text
/// 1 ┤  /\
///   | /  \_____
///   |/         ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾ (≈ 0)
///   +---+---+---+---- 120 s ────→
///   0   1   4
///        rise decay     silent
/// ```
fn glint_envelope(t: f64, spot: usize) -> f32 {
    let offset = (spot as f64 * GOLDEN_ANGLE / std::f64::consts::TAU) * GLINT_PERIOD_S;
    let t_local = (t + offset).rem_euclid(GLINT_PERIOD_S);

    if t_local < GLINT_RISE_S {
        (t_local / GLINT_RISE_S) as f32
    } else if t_local < GLINT_RISE_S + GLINT_DECAY_S {
        let t_decay = t_local - GLINT_RISE_S;
        (-GLINT_K * t_decay).exp() as f32
    } else {
        0.0
    }
}

// ─── "The Rabbit Hole" effect ─────────────────────────────────────────────

/// Per-spot colour oscillation between purple and teal.
/// Outputs 40 channels (10 × RGBW).
///
/// Each spot's `teal_ness ∈ [0, 1]` is driven by a shaped LFO:
/// - 0.0 → pure purple  
/// - 1.0 → pure teal  
/// - in between → smooth crossfade
struct RabbitHole;

impl Effect for RabbitHole {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        build_ceiling(|spot| {
            // shaped_lfo → 0.0 = purple extreme, 1.0 = teal extreme.
            let teal_ness = shaped_lfo(ctx.time, spot, RABBIT_HOLE_PERIOD_S, RABBIT_HOLE_K);
            let base = PURPLE.lerp(TEAL, teal_ness);
            // Overlay the glint: lerp toward full white at the sparkle peak.
            let glint = glint_envelope(ctx.time, spot);
            base.lerp(GLINT_WHITE, glint)
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 5_000 },
        vec![
            // Bar ceiling: dim warm white.  Staff need to see what they are
            // pouring; the cocktails will still look great under this.
            Binding::single(
                &fixtures::BAR_DECKE,
                Constant::new(uniform_ceiling(BAR_WHITE.scale(BAR_INTENSITY))),
            ),
            // Lounge ceiling: "The Rabbit Hole".  This is the whole show.
            Binding::single(&fixtures::LOUNGE_DECKE, RabbitHole),
        ],
    )
}
