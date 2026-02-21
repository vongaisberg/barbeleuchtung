use std::time::Instant;

use crate::fixtures;

/// Tracks an in-progress crossfade between two theme IDs.
#[derive(Clone, Debug)]
pub struct CrossfadeState {
    /// Theme we are fading away from.
    pub from_theme_id: usize,
    /// Theme we are fading towards.
    pub to_theme_id: usize,
    /// Crossfade total duration in milliseconds.
    pub duration_ms: u64,
    /// Wall-clock instant when the crossfade started.
    pub started_at: Instant,
}

impl CrossfadeState {
    /// Returns the blend factor `0.0..=1.0` (0 = fully `from`, 1 = fully `to`).
    pub fn factor(&self) -> f32 {
        let elapsed_ms = self.started_at.elapsed().as_millis() as f64;
        (elapsed_ms / self.duration_ms as f64).clamp(0.0, 1.0) as f32
    }

    pub fn is_complete(&self) -> bool {
        self.factor() >= 1.0
    }
}

/// Shared application state, protected externally by `Arc<Mutex<AppState>>`.
pub struct AppState {
    /// Index into the theme registry of the currently active theme.
    pub active_theme_id: usize,
    /// If a crossfade is in progress this holds its descriptor.
    pub crossfade: Option<CrossfadeState>,
    /// Index into the FX theme registry of the currently active FX theme.
    pub active_fx_theme_id: usize,
    /// If an FX crossfade is in progress this holds its descriptor.
    pub fx_crossfade: Option<CrossfadeState>,
    /// One value per fader, in `0.0..=1.0`.
    /// Length must equal `fixtures::FADER_FIXTURES.len()`.
    pub fader_values: Vec<f32>,
    /// If true, the engine sends all-zero DMX output regardless of themes.
    pub blackout: bool,
    /// Fog machine automation enabled.
    pub fog_enabled: bool,
    /// Minutes between fog bursts (x).  Range: 1–30.
    pub fog_interval_min: f32,
    /// Seconds the pump runs per burst (y).  Range: 1–30.
    pub fog_duration_s: f32,
    /// Pump output level (z), in `0.0..=1.0`.  Range: 0.1–1.0.
    pub fog_level: f32,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_theme_id: 0,
            crossfade: None,
            active_fx_theme_id: 0,
            fx_crossfade: None,
            fader_values: vec![0.0; fixtures::FADER_FIXTURES.len()],
            blackout: false,
            fog_enabled: false,
            fog_interval_min: 5.0,
            fog_duration_s: 5.0,
            fog_level: 0.5,
        }
    }

    /// Request a theme change. If the incoming theme uses a crossfade
    /// transition this initiates crossfade tracking; otherwise it switches
    /// instantly.
    pub fn request_theme(
        &mut self,
        new_id: usize,
        duration_ms: Option<u64>,
    ) {
        if new_id == self.active_theme_id {
            return;
        }

        match duration_ms {
            Some(dur) if dur > 0 => {
                let from = self
                    .crossfade
                    .as_ref()
                    .map(|cf| cf.to_theme_id)
                    .unwrap_or(self.active_theme_id);
                self.crossfade = Some(CrossfadeState {
                    from_theme_id: from,
                    to_theme_id: new_id,
                    duration_ms: dur,
                    started_at: Instant::now(),
                });
            }
            _ => {
                self.active_theme_id = new_id;
                self.crossfade = None;
            }
        }
    }

    /// Request an FX theme change, with the same crossfade logic as
    /// `request_theme` but operating on the FX bank fields.
    pub fn request_fx_theme(
        &mut self,
        new_id: usize,
        duration_ms: Option<u64>,
    ) {
        if new_id == self.active_fx_theme_id {
            return;
        }

        match duration_ms {
            Some(dur) if dur > 0 => {
                let from = self
                    .fx_crossfade
                    .as_ref()
                    .map(|cf| cf.to_theme_id)
                    .unwrap_or(self.active_fx_theme_id);
                self.fx_crossfade = Some(CrossfadeState {
                    from_theme_id: from,
                    to_theme_id: new_id,
                    duration_ms: dur,
                    started_at: Instant::now(),
                });
            }
            _ => {
                self.active_fx_theme_id = new_id;
                self.fx_crossfade = None;
            }
        }
    }

    /// Called by the engine each tick to advance / complete crossfades.
    pub fn tick_crossfade(&mut self) {
        if let Some(cf) = &self.crossfade {
            if cf.is_complete() {
                self.active_theme_id = cf.to_theme_id;
                self.crossfade = None;
            }
        }
        if let Some(cf) = &self.fx_crossfade {
            if cf.is_complete() {
                self.active_fx_theme_id = cf.to_theme_id;
                self.fx_crossfade = None;
            }
        }
    }
}
