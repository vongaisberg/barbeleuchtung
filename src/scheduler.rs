use chrono::{Datelike, NaiveDate, NaiveTime, Weekday};

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
    /// If `Some`, the entry only fires on dates for which this returns `true`.
    pub active_on: Option<fn(NaiveDate) -> bool>,
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
                active_on: None,
            })
            .collect();
        Self::new(entries)
    }

    /// Return the FX theme id the schedule dictates at wall-clock `now`.
    ///
    /// Unlike [`tick`](Self::tick) (which is edge-triggered on boundary
    /// crossings), this resolves the *currently in-effect* scheduled FX theme:
    /// the most recent entry – cyclically over the day, so before the first
    /// entry it wraps to the last one – that carries an `fx_theme_id`.
    pub fn scheduled_fx_theme_id(&self, now: NaiveTime) -> Option<usize> {
        let mut fx_entries: Vec<&ScheduleEntry> = self
            .entries
            .iter()
            .filter(|e| e.fx_theme_id.is_some())
            .collect();
        if fx_entries.is_empty() {
            return None;
        }
        fx_entries.sort_by_key(|e| e.time);

        // Latest entry whose time is at or before `now`.
        let chosen = fx_entries.iter().filter(|e| e.time <= now).last().copied();
        // If none have fired yet today, the active one is the previous day's
        // last entry (wrap around midnight).
        let chosen = chosen.unwrap_or_else(|| fx_entries.last().copied().unwrap());
        chosen.fx_theme_id
    }

    /// Call once per tick to check for crossed schedule entries.
    pub fn tick(&self, prev: NaiveTime, current: NaiveTime, state: &mut AppState) {
        if !self.enabled || self.entries.is_empty() {
            return;
        }

        let today = chrono::Local::now().date_naive();

        for entry in &self.entries {
            if entry.active_on.is_some_and(|active| !active(today)) {
                continue;
            }

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
            active_on: None,
        },
        // 17:00 – Golden Afternoon transition, The Looking Glass starts
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            theme_id: Some(1), // Golden Afternoon
            fx_theme_id: Some(1), // fx_looking_glass
            fader_values: None,
            fog_enabled: None,
            active_on: None,
        },
        // 21:00 – Cheshire Grin starts
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
            theme_id: Some(7), // stained glass
            fx_theme_id: Some(2), // fx_cheshire_grin
            fader_values: None,
            fog_enabled: None,
            active_on: None,
        },
        // 22:00 – Keep current theme, fans on (daily in summer, Fridays only
        //          October–April). The 07:00 entry turns them off every day.
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            theme_id: None, // keep current theme
            fx_theme_id: None, // fx_cheshire_grin
            fader_values: Some(fans_on.clone()),
            fog_enabled: None,
            active_on: Some(fans_scheduled_on),
        },
        // 23:00 – White Rabbit starts
        ScheduleEntry {
            time: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            theme_id: None, // Keep current theme
            fx_theme_id: Some(3), // fx_white_rabbit
            fader_values: None, // Keep fans on
            fog_enabled: None,
            active_on: None,
        },

    ];
    
    // Sort entries by time for correct processing order
    entries.sort_by_key(|e| e.time);
    
    Scheduler::new(entries)
}

/// Fans run every night May–September, but only on Fridays October–April.
fn fans_scheduled_on(date: NaiveDate) -> bool {
    let winter = !(5..=9).contains(&date.month());
    !winter || date.weekday() == Weekday::Fri
}

/// FX theme id the standard bar schedule dictates *right now* (local time).
/// Convenience wrapper around [`Scheduler::scheduled_fx_theme_id`] used by
/// callers outside the engine thread (e.g. the Spotify sync) that don't hold a
/// `Scheduler` instance.
pub fn current_scheduled_fx_theme_id() -> Option<usize> {
    complete_schedule().scheduled_fx_theme_id(chrono::Local::now().time())
}
