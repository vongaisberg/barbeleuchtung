use chrono::NaiveTime;

use crate::state::AppState;
use crate::theme::{all_fx_themes, all_themes};

/// A single time-of-day schedule entry: at `time` activate `theme_id`.
#[derive(Clone, Debug)]
pub struct ScheduleEntry {
    pub time: NaiveTime,
    pub theme_id: Option<usize>,
    pub fx_theme_id: Option<usize>,
    pub fader_values: Option<Vec<f32>>,
    /// If `Some`, set `AppState::fog_enabled` to this value when the entry fires.
    /// Fog is not a fader fixture so it needs its own scheduling channel.
    pub fog_enabled: Option<bool>,
}

/// Checks whether any schedule entry has been crossed since the last tick and,
/// if so, requests the corresponding theme change on `AppState`.
///
/// `prev_time` is the wall-clock time from the previous tick; `current_time`
/// is the time for the current tick. Crossing midnight is handled correctly.
pub struct Scheduler {
    entries: Vec<ScheduleEntry>,
    pub enabled: bool,
}

impl Scheduler {
    pub fn new(entries: Vec<ScheduleEntry>) -> Self {
        Self {
            entries,
            enabled: true,
        }
    }

    /// Create a scheduler from a slice of `(HH, MM, SS, theme_id)` tuples.
    /// Legacy method for backward compatibility.
    pub fn from_tuples(entries: &[(u32, u32, u32, usize)]) -> Self {
        let entries = entries
            .iter()
            .map(|&(h, m, s, id)| ScheduleEntry {
                time: NaiveTime::from_hms_opt(h, m, s)
                    .expect("invalid time in scheduler"),
                theme_id: Some(id),
                fx_theme_id: None,
                fader_values: None,
                fog_enabled: None,
            })
            .collect();
        Self::new(entries)
    }

    /// Call once per tick to check for crossed schedule entries.
    pub fn tick(&self, prev: NaiveTime, current: NaiveTime, state: &mut AppState) {
        if !self.enabled || self.entries.is_empty() {
            return;
        }

        for entry in &self.entries {
            let crossed = if current >= prev {
                // Normal case: no midnight crossing.
                entry.time > prev && entry.time <= current
            } else {
                // Midnight crossing: prev is late, current is early.
                entry.time > prev || entry.time <= current
            };

            if crossed {
                // Handle regular theme change
                if let Some(theme_id) = entry.theme_id {
                    let themes = all_themes();
                    let duration_ms = themes.get(theme_id).and_then(|t| {
                        match t.transition {
                            crate::theme::Transition::Crossfade { duration_ms } => Some(duration_ms),
                            _ => None,
                        }
                    });
                    state.request_theme(theme_id, duration_ms);
                    log::info!(
                        "Scheduler: switched to theme {} at {:?}",
                        theme_id,
                        current
                    );
                }

                // Handle FX theme change
                if let Some(fx_theme_id) = entry.fx_theme_id {
                    let fx_themes = all_fx_themes();
                    let duration_ms = fx_themes.get(fx_theme_id).and_then(|t| {
                        match t.transition {
                            crate::theme::Transition::Crossfade { duration_ms } => Some(duration_ms),
                            _ => None,
                        }
                    });
                    state.request_fx_theme(fx_theme_id, duration_ms);
                    log::info!(
                        "Scheduler: switched to FX theme {} at {:?}",
                        fx_theme_id,
                        current
                    );
                }

                // Handle fader values
                if let Some(ref fader_vals) = entry.fader_values {
                    if fader_vals.len() == state.fader_values.len() {
                        state.fader_values = fader_vals.clone();
                        log::info!(
                            "Scheduler: updated fader values at {:?}",
                            current
                        );
                    } else {
                        log::warn!(
                            "Scheduler: fader values length mismatch (expected {}, got {})",
                            state.fader_values.len(),
                            fader_vals.len()
                        );
                    }
                }

                // Handle fog enable/disable
                if let Some(fog_enabled) = entry.fog_enabled {
                    state.fog_enabled = fog_enabled;
                    log::info!(
                        "Scheduler: fog_enabled = {} at {:?}",
                        fog_enabled,
                        current
                    );
                }
            }
        }
    }
}

/// Complete schedule for the bar. All schedule entries defined in one place.
///
/// Theme IDs match the order in `themes::all_themes()`:
///   0 = Mad Hatter's Workspace, 1 = Golden Afternoon,
///   2 = Cheshire Moon,          3 = Closed,
///   4 = Police (manual),        5 = Sunlight (never scheduled)
///
/// FX theme IDs match the order in `themes::all_fx_themes()`:
///   0 = fx_off, 1 = fx_looking_glass, 2 = fx_cheshire_grin, 3 = fx_white_rabbit
pub fn complete_schedule() -> Scheduler {
    use crate::fixtures;

    // Fans on: all three fan faders set to 1.0, all other faders left at 0.
    // Indices match the order in `fixtures::FADER_FIXTURES`:
    //   0 ARRI_1, 1 ARRI_2, 2 FAN_ZULUFT, 3 FAN_BAR, 4 FAN_GANG,
    //   5 TRAFFIC_RED, 6 TRAFFIC_GREEN
    let fans_on = {
        let mut vals = vec![0.0; fixtures::FADER_FIXTURES.len()];
        vals[2] = 1.0; // FAN_ZULUFT
        vals[3] = 1.0; // FAN_BAR
        vals[4] = 1.0; // FAN_GANG
        vals
    };

    // All faders off (fans + traffic-light bulbs + Arri dimmers).
    let all_faders_off = vec![0.0; fixtures::FADER_FIXTURES.len()];

    let mut entries = vec![
        // 07:00 – Open, Mad Hatter daytime, FX off, all faders off
        //          (fans + traffic-light both red & green), fog disabled.
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            theme_id: Some(0), // Mad Hatter's Workspace
            fx_theme_id: Some(0), // fx_off
            fader_values: Some(all_faders_off.clone()),
            fog_enabled: Some(false),
        },
        // 17:00 – Golden Afternoon transition, The Looking Glass starts
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            theme_id: Some(1), // Golden Afternoon
            fx_theme_id: Some(1), // fx_looking_glass
            fader_values: None,
            fog_enabled: None,
        },
        // 21:00 – Cheshire Grin starts, fans turn on
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
            theme_id: Some(2), // Keep current theme
            fx_theme_id: Some(2), // fx_cheshire_grin
            fader_values: Some(fans_on.clone()),
            fog_enabled: None,
        },
        // 23:00 – White Rabbit starts
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            theme_id: None, // Keep current theme
            fx_theme_id: Some(3), // fx_white_rabbit
            fader_values: None, // Keep fans on
            fog_enabled: None,
        },

    ];
    
    // Sort entries by time for correct processing order
    entries.sort_by_key(|e| e.time);
    
    Scheduler::new(entries)
}
