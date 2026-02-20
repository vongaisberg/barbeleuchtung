//! # "Police" theme
//!
//! Emergency-light strobing across all DECKE ceiling spots.
//! The pattern is: **4 red blinks** on even spots (0, 2, 4, 6, 8), then
//! **4 blue blinks** on odd spots (1, 3, 5, 7, 9), then repeat.
//! While one colour group blinks the other stays dark.
//!
//! ```text
//!       even spots:  ██░██░██░██░░░░░░░░░░░░░░  (4× red)
//!        odd spots:  ░░░░░░░░░░░░░██░██░██░██░  (4× blue)
//!                    ←── 4/FREQ ──→←── 4/FREQ ──→
//! ```
//!
//! Both `LOUNGE_DECKE` and `BAR_DECKE` run the same effect.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{build_ceiling, Rgbw};

pub const NAME: &str = "Police";

// ─── Colour constants ─────────────────────────────────────────────────────

const RED: Rgbw = Rgbw::new(1.0, 0.0, 0.0, 0.0);
const BLUE: Rgbw = Rgbw::new(0.0, 0.0, 1.0, 0.0);
const BLACK: Rgbw = Rgbw::new(0.0, 0.0, 0.0, 0.0);

// ─── Strobe parameters ────────────────────────────────────────────────────

/// Individual blink rate within each colour group (on+off cycles per second).
const BLINK_FREQ: f64 = 10.0;

/// Number of blinks per colour group before switching to the other colour.
const BLINKS_PER_COLOR: f64 = 8.0;

/// Duration of each colour group's window (4 blinks ÷ 3 Hz ≈ 1.333 s).
const HALF_PERIOD_S: f64 = BLINKS_PER_COLOR / BLINK_FREQ;

/// Full red-then-blue cycle duration (2 × HALF_PERIOD_S ≈ 2.667 s).
const CYCLE_PERIOD_S: f64 = 2.0 * HALF_PERIOD_S;

// ─── Police strobe effect ─────────────────────────────────────────────────

/// 4 red blinks on even spots, then 4 blue blinks on odd spots, repeat.
struct PoliceStrobe;

impl Effect for PoliceStrobe {
    fn channel_count(&self) -> usize {
        super::NUM_SPOTS * 4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t_cycle = ctx.time.rem_euclid(CYCLE_PERIOD_S);
        let red_phase = t_cycle < HALF_PERIOD_S;

        // Time within the current colour group's half-period.
        let t_half = if red_phase { t_cycle } else { t_cycle - HALF_PERIOD_S };

        // 50 % duty-cycle blink within the active half.
        let blink_on = (t_half * BLINK_FREQ).fract() < 0.5;

        build_ceiling(|spot| {
            let is_even = spot % 2 == 0;
            match (is_even, red_phase, blink_on) {
                (true, true, true) => RED,   // even spot, red window, blink on
                (false, false, true) => BLUE, // odd spot, blue window, blink on
                _ => BLACK,
            }
        })
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            Binding::single(&fixtures::LOUNGE_DECKE, PoliceStrobe),
            Binding::single(&fixtures::BAR_DECKE, PoliceStrobe),
        ],
    )
}
