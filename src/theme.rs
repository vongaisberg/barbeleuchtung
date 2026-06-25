#![allow(dead_code)]

use crate::effect::Effect;
use crate::fixture::Fixture;

// ---------------------------------------------------------------------------
// Theme data structures
// ---------------------------------------------------------------------------

/// How to transition into this theme.
#[derive(Clone, Debug)]
pub enum Transition {
    Instant,
    Crossfade { duration_ms: u64 },
}

/// Binds one fixture to the effect that drives it within a given theme.
pub struct FixtureBinding {
    pub fixture: &'static Fixture,
    pub effect: Box<dyn Effect>,
}


/// Binds a whole group of fixtures to a single effect. The effect is called
/// once per fixture, with `ctx.slot` filled in from the group layout so the
/// effect can vary per fixture (phase offset, position, etc.).
pub struct GroupBinding {
    pub fixtures: Vec<&'static Fixture>,
    pub effect: Box<dyn Effect>,
}

/// A binding inside a theme: either a single fixture or a whole group.
pub enum Binding {
    Single(FixtureBinding),
    Group(GroupBinding),
}

// Convenience constructors so call-sites stay concise.
impl Binding {
    pub fn single(fixture: &'static Fixture, effect: impl Effect + 'static) -> Self {
        Binding::Single(FixtureBinding {
            fixture,
            effect: Box::new(effect),
        })
    }

    pub fn group(
        fixtures: Vec<&'static Fixture>,
      
        effect: impl Effect + 'static,
    ) -> Self {
        Binding::Group(GroupBinding {
            fixtures,
            effect: Box::new(effect),
        })
    }

}

/// A named lighting scene.
pub struct Theme {
    pub name: &'static str,
    pub transition: Transition,
    /// All bindings that this theme controls (single fixtures or groups).
    /// Fixtures not mentioned here are left at zero.
    pub bindings: Vec<Binding>,
}

impl Theme {
    pub fn new(name: &'static str, transition: Transition, bindings: Vec<Binding>) -> Self {
        Self {
            name,
            transition,
            bindings,
        }
    }
}

// ---------------------------------------------------------------------------
// Theme registry – delegates to src/themes/
// ---------------------------------------------------------------------------

/// All active themes, in the order their IDs are assigned.
/// Theme IDs are positional: index 0, 1, 2, … match what the UI sends.
pub fn all_themes() -> Vec<Theme> {
    crate::themes::all_themes()
}

/// Display names for the web UI (same order as `all_themes()`).
pub fn theme_names() -> Vec<&'static str> {
    crate::themes::theme_names()
}

/// All FX bank themes, in the order their IDs are assigned.
pub fn all_fx_themes() -> Vec<Theme> {
    crate::themes::all_fx_themes()
}

/// Display names for the FX bank (same order as `all_fx_themes()`).
pub fn fx_theme_names() -> Vec<&'static str> {
    crate::themes::fx_theme_names()
}

/// All reactive bank themes, in the order their IDs are assigned.
pub fn all_reactive_themes() -> Vec<Theme> {
    crate::themes::all_reactive_themes()
}

/// Display names for the reactive bank (same order as `all_reactive_themes()`).
pub fn reactive_theme_names() -> Vec<&'static str> {
    crate::themes::reactive_theme_names()
}
