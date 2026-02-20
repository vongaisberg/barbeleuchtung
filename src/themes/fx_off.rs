//! FX bank "Off" theme – no bindings, all FX fixtures output zero.
//! This is the default FX theme; it is always index 0 in the FX registry.

use crate::theme::{Theme, Transition};

pub const NAME: &str = "Off";

pub fn theme() -> Theme {
    Theme::new(NAME, Transition::Instant, vec![])
}
