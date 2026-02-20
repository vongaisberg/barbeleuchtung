//! # "Closed"
//! **Hours**: 02:00 onward   **Active when**: bar is shut for the night.
//!
//! ## Design intent
//! All ceiling spots off.  No DMX output on the ceiling universes whatsoever.
//! Any other fixtures (Arri, fans) are controlled independently via their own
//! faders and are unaffected by this theme.

use crate::effect::Constant;
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::uniform_ceiling;
use super::Rgbw;

pub const NAME: &str = "Closed";

const OFF: Rgbw = Rgbw::new(0.0, 0.0, 0.0, 0.0);

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 8_000 }, // slow, gentle fade-to-black
        vec![
            Binding::single(
                &fixtures::BAR_DECKE,
                Constant::new(uniform_ceiling(OFF)),
            ),
            Binding::single(
                &fixtures::LOUNGE_DECKE,
                Constant::new(uniform_ceiling(OFF)),
            ),
        ],
    )
}
