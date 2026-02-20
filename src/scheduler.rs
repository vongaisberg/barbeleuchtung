use chrono::NaiveTime;

use crate::state::AppState;
use crate::theme::all_themes;

/// A single time-of-day schedule entry: at `time` activate `theme_id`.
#[derive(Clone, Debug)]
pub struct ScheduleEntry {
    pub time: NaiveTime,
    pub theme_id: usize,
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
    pub fn from_tuples(entries: &[(u32, u32, u32, usize)]) -> Self {
        let entries = entries
            .iter()
            .map(|&(h, m, s, id)| ScheduleEntry {
                time: NaiveTime::from_hms_opt(h, m, s)
                    .expect("invalid time in scheduler"),
                theme_id: id,
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
                let themes = all_themes();
                let duration_ms = themes.get(entry.theme_id).and_then(|t| {
                    match t.transition {
                        crate::theme::Transition::Crossfade { duration_ms } => Some(duration_ms),
                        _ => None,
                    }
                });
                state.request_theme(entry.theme_id, duration_ms);
                log::info!(
                    "Scheduler: switched to theme {} at {:?}",
                    entry.theme_id,
                    current
                );
            }
        }
    }
}

/// Default schedule for the bar. Edit to match opening hours.
/// Theme IDs match the order in `themes::all_themes()`:
///   0 = Mad Hatter's Workspace, 1 = Golden Afternoon,
///   2 = Cheshire Moon,          3 = Closed
pub fn default_schedule() -> Scheduler {
    Scheduler::from_tuples(&[
        (7, 0, 0, 0), // 08:00 – open, Mad Hatter daytime
        (17, 0, 0, 1), // 17:00 – Golden Afternoon transition
        (10, 0, 0, 2), // 19:00 – Cheshire Moon evening
       // ( 2, 0, 0, 3), // 02:00 – Closed, gentle fade to black
    ])
}
