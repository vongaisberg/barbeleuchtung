//! # FX Show – "Would You"
//!
//! Full-length, beat-perfect timecoded show for
//! *Campbell x Alcemist – Would You (go to bed with me?)* (`Campbell.opus`).
//!
//! ## Timing (measured from the audio, not guessed)
//! - Duration: 120.69 s
//! - Tempo: **86.860 BPM** — rock-steady across the whole track
//!   (beat = 0.69077 s, bar = 2.76306 s)
//! - First downbeat: **0.4818 s**
//!
//! The grid is locked to these numbers, so every hit lands on the beat for the
//! entire song (the previous 86.54 BPM guess drifted ~½ beat by the end).
//!
//! ## Synchronisation
//! The show keys off [`TickContext::show_time`], which is reset to ~0 whenever
//! this scene is selected.  **Start the song the moment you activate the
//! scene** and it will stay locked for the full track.
//!
//! ## Structure (bars counted from the first downbeat)
//! | Bars  | Section    | Feel                                  |
//! |-------|------------|---------------------------------------|
//! | 0–3   | Intro      | dark purple breath, slow scanner drift|
//! | 4–10  | Verse 1    | deep-red beat chase, cool scanner steps|
//! | 11    | Lift       | 1-bar riser into the drop             |
//! | 12–19 | Chorus 1   | white downbeats, magenta wash, X beams|
//! | 20–25 | Verse 2    | cooler blue/purple groove             |
//! | 26–28 | Build      | rising intensity ramp                 |
//! | 29–30 | Breakdown  | dark tension, beams converge up       |
//! | 31–37 | Finale     | biggest — saturated red, white strobe |
//! | 38    | Come-down  | pull back                             |
//! | 39–42 | Outro      | fade to black                         |

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    scanner_frame, SC_COLOR_BLUE, SC_COLOR_DKBLUE, SC_COLOR_PINK, SC_COLOR_RED, SC_COLOR_SALMON,
    SC_COLOR_WHITE, SC_FOCUS_MID, SC_GOBO_OPEN, SC_GOBOROT_NONE, SC_PRISM_OFF, SC_SHUTTER_CLOSED,
    SC_SHUTTER_OPEN,
};

pub const NAME: &str = "Would You";

// ─── Beat grid (from audio analysis) ─────────────────────────────────────────

const BPM: f64 = 86.860;
const BEAT_S: f64 = 60.0 / BPM; // 0.690766 s
const BAR_S: f64 = BEAT_S * 4.0; // 2.763063 s
/// First downbeat, seconds into the song.
const BEAT0_S: f64 = 0.4818;
/// Audio end (for outro fade scaling).
const SONG_END_S: f64 = 120.6935;

/// Start of the outro section (bar 39), seconds into the song.
const OUTRO_START_S: f64 = BEAT0_S + 39.0 * BAR_S;

// ─── Time helpers ─────────────────────────────────────────────────────────────

/// Seconds since the first downbeat (negative during the pre-roll).
#[inline]
fn song_t(ctx: &TickContext) -> f64 {
    ctx.show_time - BEAT0_S
}

#[inline]
fn beat_idx(ctx: &TickContext) -> i64 {
    let t = song_t(ctx);
    if t < 0.0 { -1 } else { (t / BEAT_S).floor() as i64 }
}

#[inline]
fn bar_idx(ctx: &TickContext) -> i64 {
    let t = song_t(ctx);
    if t < 0.0 { -1 } else { (t / BAR_S).floor() as i64 }
}

/// Position within the current beat in seconds (0 at each beat onset).
#[inline]
fn t_in_beat(ctx: &TickContext) -> f64 {
    let t = song_t(ctx);
    if t < 0.0 { BEAT_S } else { t.rem_euclid(BEAT_S) }
}

#[inline]
fn beat_in_bar(ctx: &TickContext) -> usize {
    let b = beat_idx(ctx);
    if b < 0 { 0 } else { (b as usize) & 3 }
}

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Linear ramp 0→1 across a bar range [start_bar, end_bar).
fn bar_ramp(ctx: &TickContext, start_bar: f64, end_bar: f64) -> f32 {
    let t = song_t(ctx);
    let start = start_bar * BAR_S;
    let end = end_bar * BAR_S;
    if end <= start {
        return 1.0;
    }
    clamp01(((t - start) / (end - start)) as f32)
}

/// Outro fade multiplier: 1.0 until the outro, then ramps to 0 by song end.
fn outro_fade(ctx: &TickContext) -> f32 {
    if ctx.show_time < OUTRO_START_S {
        1.0
    } else {
        clamp01(((SONG_END_S - ctx.show_time) / (SONG_END_S - OUTRO_START_S)) as f32)
    }
}

// ─── Song sections ────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Eq)]
enum Section {
    Intro,
    Verse1,
    Lift,
    Chorus1,
    Verse2,
    Build,
    Breakdown,
    Finale,
    Comedown,
    Outro,
}

fn section_at(ctx: &TickContext) -> Section {
    match bar_idx(ctx) {
        b if b < 4 => Section::Intro,     // incl. pre-roll (b < 0)
        b if b < 11 => Section::Verse1,   // 4..10
        11 => Section::Lift,              // 11
        b if b < 20 => Section::Chorus1,  // 12..19
        b if b < 26 => Section::Verse2,   // 20..25
        b if b < 29 => Section::Build,    // 26..28
        b if b < 31 => Section::Breakdown,// 29..30
        b if b < 38 => Section::Finale,   // 31..37
        38 => Section::Comedown,          // 38
        _ => Section::Outro,              // 39..
    }
}

/// True on the downbeat window of a phrase-ending bar, used for scanner X-beams.
fn is_cross_bar(bar: i64) -> bool {
    // Last bar of chorus 1 (19) and finale (37): sweep the beams into an X.
    bar == 19 || bar == 37
}

// ─────────────────────────────────────────────────────────────────────────────
// RootPars – 4-step beat chase with section-aware colour + downbeat hits
// ─────────────────────────────────────────────────────────────────────────────

/// [Dimmer, Strobe, R, G, B, W, Amber, UV]
const PAR_WHITE: [f32; 8] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
const PAR_OFF: [f32; 8] = [0.0; 8];

const PAR_WHITE_DUR_S: f64 = 0.06;
const PAR_FLASH_DUR_S: f64 = 0.10;

struct WouldYouRootPar {
    step: u64,
}

impl WouldYouRootPar {
    fn active(&self, ctx: &TickContext) -> bool {
        let b = beat_idx(ctx);
        let s = if b < 0 { 0 } else { b as u64 % 4 };
        s == self.step
    }
}

impl Effect for WouldYouRootPar {
    fn channel_count(&self) -> usize {
        8
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let active = self.active(ctx);

        // White downbeat hits in the big sections (all pars together).
        if matches!(section, Section::Chorus1 | Section::Finale)
            && bib == 0
            && tb < PAR_WHITE_DUR_S
        {
            return PAR_WHITE.to_vec();
        }

        let mut out = match section {
            Section::Intro => {
                // Slow purple breath, barely lit.
                let breath = (ctx.show_time * 0.20).sin() * 0.5 + 0.5;
                let lvl = 0.05 + 0.13 * breath as f32;
                vec![lvl, 0.0, 0.45, 0.0, 1.0, 0.0, 0.0, 0.0]
            }
            Section::Verse1 => {
                if active {
                    // Deep red with a touch of magenta.
                    vec![0.50, 0.0, 1.0, 0.0, 0.18, 0.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Lift => {
                // 1-bar riser: brightness ramps over the bar to a near-white.
                let r = bar_ramp(ctx, 11.0, 12.0);
                vec![0.20 + 0.70 * r, 0.0, 1.0, 0.30 * r, 0.40 * r, 0.30 * r, 0.0, 0.0]
            }
            Section::Chorus1 => {
                if active {
                    vec![0.85, 0.0, 1.0, 0.10, 0.30, 0.0, 0.0, 0.0]
                } else if bib == 2 && self.step % 2 == 0 && tb < PAR_FLASH_DUR_S {
                    // Backbeat magenta wink on alternating pars.
                    vec![0.55, 0.0, 1.0, 0.0, 0.55, 0.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Verse2 => {
                if active {
                    // Cooler: indigo / blue.
                    vec![0.45, 0.0, 0.25, 0.05, 1.0, 0.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Build => {
                // Chase stays, but intensity ramps across bars 26→29.
                let r = 0.45 + 0.55 * bar_ramp(ctx, 26.0, 29.0);
                if active {
                    vec![r, 0.0, 1.0, 0.10, 0.45, 0.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Breakdown => {
                // Dark tension: one slow swell per bar on the downbeat par.
                let swell = (1.0 - (tb / BEAT_S)) as f32; // decays across the beat
                if bib == 0 && self.step == 0 {
                    vec![0.10 + 0.35 * swell, 0.0, 0.50, 0.0, 0.80, 0.0, 0.0, 0.0]
                } else {
                    vec![0.04, 0.0, 0.20, 0.0, 0.35, 0.0, 0.0, 0.0]
                }
            }
            Section::Finale => {
                if active {
                    // Saturated red, full power.
                    vec![1.0, 0.0, 1.0, 0.10, 0.05, 0.0, 0.0, 0.0]
                } else if bib == 2 && tb < PAR_FLASH_DUR_S {
                    // White backbeat wink across all.
                    vec![0.60, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Comedown => {
                // Pull back to a warm medium glow.
                if active {
                    vec![0.40, 0.0, 1.0, 0.20, 0.10, 0.0, 0.0, 0.0]
                } else {
                    vec![0.10, 0.0, 0.60, 0.05, 0.10, 0.0, 0.0, 0.0]
                }
            }
            Section::Outro => {
                let f = outro_fade(ctx);
                vec![0.18 * f, 0.0, 0.50 * f, 0.0, 0.60 * f, 0.0, 0.0, 0.0]
            }
        };

        // Weak white backbeat wink in the groove sections (not the big hits).
        if matches!(section, Section::Verse1 | Section::Verse2)
            && (beat_in_bar(ctx) == 1 || beat_in_bar(ctx) == 3)
            && tb < 0.07
        {
            out = vec![0.22, 0.0, 0.0, 0.0, 0.0, 0.18, 0.0, 0.0];
        }

        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PixStrobes – section palette pixel wave + beat-locked blinder hits
// ─────────────────────────────────────────────────────────────────────────────

const PIX_FLASH_DUR_S: f64 = 0.08;
const PIX_STROBE_DUR_S: f64 = 0.14;

struct WouldYouPixStrobe;

impl Effect for WouldYouPixStrobe {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let beat_global = beat_idx(ctx).max(0) as usize;
        let fade = outro_fade(ctx);

        let mut out = vec![0.0f32; 32];

        // Section palette for the RGB pixels (R, G, B).
        let (pr, pg, pb) = match section {
            Section::Intro => (0.30, 0.0, 0.55),
            Section::Verse1 => (0.55, 0.02, 0.12),
            Section::Lift => (0.80, 0.20, 0.30),
            Section::Chorus1 => (0.95, 0.05, 0.35),
            Section::Verse2 => (0.05, 0.10, 0.55),
            Section::Build => (0.85, 0.05, 0.20),
            Section::Breakdown => (0.10, 0.0, 0.40),
            Section::Finale => (1.0, 0.05, 0.10),
            Section::Comedown => (0.55, 0.05, 0.25),
            Section::Outro => (0.25, 0.0, 0.35),
        };

        // Per-section brightness for the pixel field.
        let boost = match section {
            Section::Intro => 0.30,
            Section::Verse1 => 0.55,
            Section::Lift => 0.55 + 0.40 * bar_ramp(ctx, 11.0, 12.0),
            Section::Chorus1 => 0.95,
            Section::Verse2 => 0.55,
            Section::Build => 0.55 + 0.40 * bar_ramp(ctx, 26.0, 29.0),
            Section::Breakdown => 0.30,
            Section::Finale => 1.0,
            Section::Comedown => 0.55,
            Section::Outro => 0.30,
        } * fade;

        // Blinder (CW strip, channels 4–7), beat-locked per section.
        let mut blinder = 0.0f32;
        match section {
            Section::Verse1 | Section::Verse2 => {
                if (bib == 1 || bib == 3) && tb < PIX_FLASH_DUR_S {
                    blinder = 0.25;
                }
            }
            Section::Chorus1 | Section::Finale => {
                if (bib == 0 || bib == 2) && tb < PIX_FLASH_DUR_S {
                    blinder = 1.0;
                }
                if bib == 0 && tb < PIX_STROBE_DUR_S {
                    // Hard strobe window on the downbeat.
                    let hz = if matches!(section, Section::Finale) { 24.0 } else { 20.0 };
                    let phase = (ctx.show_time * hz).fract();
                    blinder = if phase < 0.5 { 1.0 } else { 0.0 };
                }
            }
            Section::Build => {
                // Blinder pulses grow over the build.
                if bib == 0 && tb < PIX_FLASH_DUR_S {
                    blinder = 0.30 + 0.60 * bar_ramp(ctx, 26.0, 29.0);
                }
            }
            Section::Breakdown => {
                if bib == 3 && tb < PIX_FLASH_DUR_S {
                    blinder = 0.20;
                }
            }
            _ => {}
        }
        let blinder = blinder * fade;
        for i in 4..8 {
            out[i] = blinder;
        }

        // Pixel strip: a 4-value wave that shifts one step per beat.
        for pixel in 0..8usize {
            let ch = 8 + pixel * 3;
            let wave = (beat_global + pixel) & 3;
            let pulse = match wave {
                0 => 1.0,
                1 => 0.70,
                2 => 0.45,
                _ => 0.82,
            };
            out[ch] = pr * pulse * boost;
            out[ch + 1] = pg * pulse * boost;
            out[ch + 2] = pb * pulse * boost;
        }

        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QuadPhases – section rotation + colour, beat-locked shutter
// ─────────────────────────────────────────────────────────────────────────────

// QuadPhase colour-wheel guesses (0..1); tune to the fixture's chart.
const QP_MAGENTA: f32 = 0.83;
const QP_RED: f32 = 0.95;
const QP_BLUE: f32 = 0.58;
const QP_AMBER: f32 = 0.08;
const QP_WHITE: f32 = 0.0;

struct WouldYouQuadPhase;

impl Effect for WouldYouQuadPhase {
    fn channel_count(&self) -> usize {
        4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);

        let rotation = match section {
            Section::Intro => 0.10,
            Section::Verse1 => 0.38,
            Section::Lift => 0.55,
            Section::Chorus1 => 0.74,
            Section::Verse2 => 0.34,
            Section::Build => 0.45 + 0.45 * bar_ramp(ctx, 26.0, 29.0),
            Section::Breakdown => 0.14,
            Section::Finale => 0.85,
            Section::Comedown => 0.30,
            Section::Outro => 0.05,
        };

        let color = match section {
            Section::Intro => QP_MAGENTA,
            Section::Verse1 => QP_RED,
            Section::Lift => QP_WHITE,
            Section::Chorus1 => if bib == 0 { QP_WHITE } else { QP_MAGENTA },
            Section::Verse2 => QP_BLUE,
            Section::Build => QP_RED,
            Section::Breakdown => QP_BLUE,
            Section::Finale => if bib == 0 || bib == 2 { QP_WHITE } else { QP_RED },
            Section::Comedown => QP_AMBER,
            Section::Outro => QP_WHITE,
        };

        let open_window = match section {
            Section::Intro => 0.06,
            Section::Verse1 => 0.11,
            Section::Lift => 0.16,
            Section::Chorus1 => 0.18,
            Section::Verse2 => 0.11,
            Section::Build => 0.10 + 0.10 * bar_ramp(ctx, 26.0, 29.0) as f64,
            Section::Breakdown => 0.08,
            Section::Finale => 0.22,
            Section::Comedown => 0.08,
            Section::Outro => 0.0,
        };

        let open = match section {
            Section::Intro | Section::Breakdown => bib == 0 && tb < open_window,
            Section::Verse1 | Section::Verse2 => (bib == 1 || bib == 3) && tb < open_window,
            Section::Chorus1 | Section::Finale | Section::Build | Section::Lift => tb < open_window,
            Section::Comedown => bib == 0 && tb < open_window,
            Section::Outro => false,
        };

        vec![color, rotation, 0.0, if open { 1.0 } else { 0.0 }]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scanners – phrase-driven sweeps, beat stabs, X-beams on phrase ends
// ─────────────────────────────────────────────────────────────────────────────

const SCAN_PAN_CTR_L: f32 = 0.62;
const SCAN_PAN_CTR_R: f32 = 0.38;
const SCAN_PAN_SPLAY_L: f32 = 0.22;
const SCAN_PAN_SPLAY_R: f32 = 0.78;
const SCAN_PAN_MID: f32 = 0.50;

const SCAN_TILT_STAGE: f32 = 0.50;
const SCAN_TILT_UP: f32 = 0.80;

const SCAN_STAB_S: f64 = 0.10;
const SCAN_CROSS_DUR_S: f64 = 0.24;

struct WouldYouScanner {
    reverse: bool,
}

impl WouldYouScanner {
    #[inline]
    fn side(&self, l: f32, r: f32) -> f32 {
        if self.reverse { r } else { l }
    }

    /// Per-beat step pattern shared by the energetic sections.
    fn step_pose(&self, bib: usize) -> (f32, f32) {
        match bib {
            0 => (SCAN_PAN_MID, SCAN_TILT_STAGE),
            1 => (self.side(SCAN_PAN_SPLAY_L, SCAN_PAN_SPLAY_R), SCAN_TILT_STAGE),
            2 => (SCAN_PAN_MID, SCAN_TILT_UP),
            _ => (self.side(SCAN_PAN_CTR_L, SCAN_PAN_CTR_R), SCAN_TILT_STAGE),
        }
    }
}

impl Effect for WouldYouScanner {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let bar = bar_idx(ctx);
        let fade = outro_fade(ctx);

        // Phrase-ending X-beam sweep on the downbeat of bars 19 / 37.
        let in_cross = is_cross_bar(bar) && bib == 0 && tb < SCAN_CROSS_DUR_S;

        let (pan, tilt, color) = if in_cross {
            let frac = (tb / SCAN_CROSS_DUR_S) as f32;
            let from = self.side(SCAN_PAN_SPLAY_L, SCAN_PAN_SPLAY_R);
            (from + (SCAN_PAN_MID - from) * frac, SCAN_TILT_STAGE, SC_COLOR_WHITE)
        } else {
            match section {
                Section::Intro => {
                    // Very slow drift around center, deep magenta.
                    let d = (ctx.show_time * 0.30).sin() as f32;
                    let pan = SCAN_PAN_MID + self.side(-0.10, 0.10) * (0.5 + 0.5 * d);
                    (pan, SCAN_TILT_STAGE, SC_COLOR_PINK)
                }
                Section::Verse1 => {
                    let (p, t) = self.step_pose(bib);
                    (p, t, SC_COLOR_RED)
                }
                Section::Lift => {
                    // Beams rise toward the ceiling over the riser bar.
                    let r = bar_ramp(ctx, 11.0, 12.0);
                    (SCAN_PAN_MID, SCAN_TILT_STAGE + (SCAN_TILT_UP - SCAN_TILT_STAGE) * r, SC_COLOR_WHITE)
                }
                Section::Chorus1 => {
                    let (p, t) = self.step_pose(bib);
                    (p, t, if bib == 0 { SC_COLOR_WHITE } else { SC_COLOR_PINK })
                }
                Section::Verse2 => {
                    let (p, t) = self.step_pose(bib);
                    (p, t, SC_COLOR_BLUE)
                }
                Section::Build => {
                    // Tighten toward center as the build progresses.
                    let r = bar_ramp(ctx, 26.0, 29.0);
                    let (p, t) = self.step_pose(bib);
                    let pan = p + (SCAN_PAN_MID - p) * r;
                    (pan, t, SC_COLOR_SALMON)
                }
                Section::Breakdown => {
                    // Both converge straight up, holding tension.
                    (SCAN_PAN_MID, SCAN_TILT_UP, SC_COLOR_DKBLUE)
                }
                Section::Finale => {
                    let (p, t) = self.step_pose(bib);
                    (p, t, if bib == 0 { SC_COLOR_WHITE } else { SC_COLOR_RED })
                }
                Section::Comedown => {
                    let pan = self.side(SCAN_PAN_CTR_L, SCAN_PAN_CTR_R);
                    (pan, SCAN_TILT_STAGE, SC_COLOR_SALMON)
                }
                Section::Outro => {
                    let pan = self.side(0.47, 0.53);
                    (pan, SCAN_TILT_UP, SC_COLOR_WHITE)
                }
            }
        };

        // Shutter: beat stabs in punchy sections; held open in smooth ones.
        let stab_sections = matches!(
            section,
            Section::Verse1 | Section::Chorus1 | Section::Finale | Section::Verse2
        );
        let shutter = if matches!(section, Section::Outro) {
            SC_SHUTTER_CLOSED
        } else if in_cross {
            SC_SHUTTER_OPEN
        } else if stab_sections {
            if tb < SCAN_STAB_S { SC_SHUTTER_OPEN } else { SC_SHUTTER_CLOSED }
        } else {
            SC_SHUTTER_OPEN
        };

        // Dimmer: section energy, scaled by the outro fade.
        let base_dim = match section {
            Section::Intro => 0.55,
            Section::Breakdown => 0.45 + 0.45 * bar_ramp(ctx, 29.0, 31.0),
            Section::Comedown => 0.55,
            Section::Outro => 0.40,
            _ => 1.0,
        };
        let dimmer = clamp01(base_dim * fade);

        scanner_frame(
            pan,
            tilt,
            color,
            shutter,
            dimmer,
            SC_GOBO_OPEN,
            SC_GOBOROT_NONE,
            SC_PRISM_OFF,
            SC_FOCUS_MID,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme factory
// ─────────────────────────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            Binding::single(&fixtures::PIXSTROBE_1, WouldYouPixStrobe),
            Binding::single(&fixtures::PIXSTROBE_2, WouldYouPixStrobe),
            Binding::single(&fixtures::QUADPHASE_1, WouldYouQuadPhase),
            Binding::single(&fixtures::QUADPHASE_2, WouldYouQuadPhase),
            Binding::single(&fixtures::ROOTPAR_1, WouldYouRootPar { step: 0 }),
            Binding::single(&fixtures::ROOTPAR_2, WouldYouRootPar { step: 1 }),
            Binding::single(&fixtures::ROOTPAR_3, WouldYouRootPar { step: 2 }),
            Binding::single(&fixtures::ROOTPAR_4, WouldYouRootPar { step: 3 }),
            Binding::single(&fixtures::SCANNER_1, WouldYouScanner { reverse: false }),
            Binding::single(&fixtures::SCANNER_2, WouldYouScanner { reverse: true }),
        ],
    )
}
