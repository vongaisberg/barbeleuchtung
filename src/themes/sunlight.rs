//! # "Sunlight"
//! **Scheduling**: Never – manual selection only.
//!
//! ## Design intent
//! A fully static, cool daylight look for the ceiling.  Mimics natural sunlight
//! by mixing warm white (base) with blue to cool it down, green to correct the
//! pinkish tint from warm white + blue, and minimal red.
//!
//! ### Colour mixture
//! - Warm White: 100 % (base spectrum)
//! - Blue: 70 % – 100 % (cools down the warm LED)
//! - Green: 20 % – 40 % (corrects pinkish tint)
//! - Red: 0 % – 10 % (sunlight has very little red)

use crate::effect::Constant;
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::uniform_ceiling;
use super::Rgbw;

pub const NAME: &str = "Sunlight";

// Values at middle of specified ranges
const SUNLIGHT: Rgbw = Rgbw::new(0.7, 0.7, 0.7, 1.0);

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Crossfade { duration_ms: 2_000 },
        vec![
            Binding::single(
                &fixtures::BAR_DECKE,
                Constant::new(uniform_ceiling(SUNLIGHT)),
            ),
            Binding::single(
                &fixtures::LOUNGE_DECKE,
                Constant::new(uniform_ceiling(SUNLIGHT)),
            ),
        ],
    )
}
