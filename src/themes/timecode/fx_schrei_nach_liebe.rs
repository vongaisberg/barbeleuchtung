//! # FX Show – "Schrei nach Liebe"
//!
//! Full-length, beat-perfect timecoded show for
//! *Die Ärzte – Schrei nach Liebe* (`die ärzte schrei nach liebe.opus`).
//!
//! ## Timing (measured from the audio, not guessed)
//! - Duration: 251.29 s
//! - Tempo: **166.605 BPM** (beat = 0.36013 s, bar = 1.44053 s) — live-band
//!   recording, so the grid wobbles slightly; section cues therefore use
//!   *measured absolute times* and only the fast chases ride the beat grid.
//! - Grid anchor: 0.0348 s; the main riff slams in at **5.797 s** (bar 0).
//! - The three **"Arschloch!"** band-stops were located by detecting beats
//!   where the low band collapses while the vocal band stays hot:
//!   52.61 s, 110.96 s and the double stop-hit at 135.08 s / 136.53 s.
//!
//! ## Synchronisation
//! Keys off [`TickContext::show_time`]; start the song the moment the scene
//! activates (or let the Spotify sync do it — `SPOTIFY_SCHREI_NACH_LIEBE_MATCH`).
//!
//! ## Concept — *violence vs. love*
//! The song answers skinhead violence with tenderness, so the show lives in
//! that tension: verses are harsh red/white punk stomps, every chorus gets
//! flooded by hot magenta/pink (the "Liebe"), and each **"Arschloch!"** is a
//! band-stop blackout followed by a brutal white strobe blast.  The quiet
//! bridge ("Weil du Probleme hast…") turns the whole rig into a dim red
//! **heartbeat** (lub-dub at ~83 bpm) under UV — the "lieber Kerl" hiding
//! underneath.  The guitar solo goes full rainbow with prisms.
//!
//! ## Structure (measured absolute times)
//! | Time (s)      | Section      | Feel                                    |
//! |---------------|--------------|------------------------------------------|
//! | 0.0–5.8       | Intro        | feedback — red ember smolder, swelling   |
//! | 5.8–17.3      | Riff         | pogo slam, red/white stabs               |
//! | 17.3–40.4     | Verse 1      | red/amber boot-stomp chase               |
//! | 40.4–52.2     | Chorus 1     | magenta flood, white downbeat slams      |
//! | 52.2–54.8     | ARSCHLOCH #1 | blackout → white strobe blast            |
//! | 54.8–72.1     | Verse 2      | cooler blue/UV groove                    |
//! | 72.1–77.8     | Slam         | all-par red stomps, scanner X            |
//! | 77.8–99.4     | Verse 2b     | verse warming back toward magenta        |
//! | 99.4–110.6    | Chorus 2     | magenta flood, hotter                    |
//! | 110.6–112.4   | ARSCHLOCH #2 | blackout → white strobe blast            |
//! | 112.4–125.4   | Interlude    | amber riff rolls, green scanner 8s       |
//! | 125.4–135.1   | Build        | everything tightens + ramps              |
//! | 135.1–137.1   | Stop-hits    | two white punches in darkness            |
//! | 137.1–149.9   | Solo         | rainbow scanners + prism, pixel wave     |
//! | 149.9–165.7   | Bridge       | near-dark red heartbeat + UV             |
//! | 165.7–167.1   | Lift         | 1-bar white riser                        |
//! | 167.1–196.0   | Final Chorus | the explosion — magenta/white/strobes    |
//! | 196.0–198.8   | Stomp        | half-dark beat slams                     |
//! | 198.8–206.0   | Chant        | "Arschloch!" slams on every downbeat     |
//! | 206.0–210.4   | Comedown     | warm amber pull-back                     |
//! | 210.4–234.8   | Outro        | pogo afterglow, pink, slowly cooling     |
//! | 234.8–246.5   | Fade         | fade to black with the tape              |

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    scanner_frame, SC_COLOR_BLUE, SC_COLOR_DKBLUE, SC_COLOR_KGREEN, SC_COLOR_PINK,
    SC_COLOR_RAINBOW, SC_COLOR_RED, SC_COLOR_SALMON, SC_COLOR_WHITE, SC_FOCUS_MID, SC_GOBO_OPEN,
    SC_GOBOROT_NONE, SC_PRISM_OFF, SC_PRISM_ROT, SC_SHUTTER_CLOSED, SC_SHUTTER_OPEN,
    SC_SHUTTER_STROBE_MED,
};

pub const NAME: &str = "Schrei nach Liebe";

// ─── Beat grid (from audio analysis) ─────────────────────────────────────────

const BPM: f64 = 166.605;
const BEAT_S: f64 = 60.0 / BPM; // 0.36013 s
const BAR_S: f64 = BEAT_S * 4.0; // 1.44053 s
/// Downbeat of the main riff entry (bar 0 of the show's bar grid).
const RIFF_S: f64 = 5.797;

// ─── Section cues (measured absolute times, seconds) ────────────────────────

const T_RIFF: f64 = RIFF_S;
const T_VERSE1: f64 = 17.32;
const T_CHORUS1: f64 = 40.37;
const T_SHOUT1: f64 = 52.20;
const T_VERSE2: f64 = 54.78;
const T_SLAM: f64 = 72.06;
const T_VERSE2B: f64 = 77.82;
const T_CHORUS2: f64 = 99.43;
const T_SHOUT2: f64 = 110.60;
const T_INTERLUDE: f64 = 112.40;
const T_BUILD: f64 = 125.36;
const T_STOPHITS: f64 = 135.08;
const T_SOLO: f64 = 137.05;
const T_BRIDGE: f64 = 149.85;
const T_LIFT: f64 = 165.70;
const T_FINAL: f64 = 167.14;
const T_STOMP: f64 = 195.95;
const T_CHANT: f64 = 198.83;
const T_COMEDOWN: f64 = 206.03;
const T_OUTRO: f64 = 210.35;
const T_FADE: f64 = 234.84;
const T_END: f64 = 246.50;

/// "Arschloch!" white-blast windows (band stopped, voice shouting).
const BLAST1: (f64, f64) = (52.55, 53.80);
const BLAST2: (f64, f64) = (110.95, 112.20);
/// The two stop-hit punches before the solo.
const PUNCH1: (f64, f64) = (135.08, 135.58);
const PUNCH2: (f64, f64) = (136.53, 137.03);

// ─── Time helpers ─────────────────────────────────────────────────────────────

#[inline]
fn beat_idx(ctx: &TickContext) -> i64 {
    let t = ctx.show_time - RIFF_S;
    if t < 0.0 { -1 } else { (t / BEAT_S).floor() as i64 }
}

#[inline]
fn bar_idx(ctx: &TickContext) -> i64 {
    let t = ctx.show_time - RIFF_S;
    if t < 0.0 { -1 } else { (t / BAR_S).floor() as i64 }
}

/// Position within the current beat in seconds (0 at each beat onset).
#[inline]
fn t_in_beat(ctx: &TickContext) -> f64 {
    let t = ctx.show_time - RIFF_S;
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

/// Linear ramp 0→1 across the absolute time range [a, b).
fn time_ramp(ctx: &TickContext, a: f64, b: f64) -> f32 {
    if b <= a {
        return 1.0;
    }
    clamp01(((ctx.show_time - a) / (b - a)) as f32)
}

/// Square-wave strobe gate at `hz`.
#[inline]
fn strobing(t: f64, hz: f64) -> bool {
    (t * hz).fract() < 0.5
}

#[inline]
fn in_window(t: f64, w: (f64, f64)) -> bool {
    t >= w.0 && t < w.1
}

/// True while one of the "Arschloch!" / stop-hit white blasts is live.
fn in_blast(ctx: &TickContext) -> bool {
    let t = ctx.show_time;
    in_window(t, BLAST1) || in_window(t, BLAST2) || in_window(t, PUNCH1) || in_window(t, PUNCH2)
}

/// Global fade envelope: full through the body, cools through the outro and
/// dies with the tape.
fn fade_env(ctx: &TickContext) -> f32 {
    let t = ctx.show_time;
    if t < T_OUTRO {
        1.0
    } else if t < T_FADE {
        // Outro: 0.60 → 0.30
        0.60 - 0.30 * time_ramp(ctx, T_OUTRO, T_FADE)
    } else {
        0.30 * (1.0 - time_ramp(ctx, T_FADE, T_END))
    }
}

/// Heartbeat (lub-dub) intensity, period = 2 beats (~83 bpm at song tempo).
/// Returns 0..1: strong thump at phase 0, weaker echo at +0.26 s.
fn heartbeat(ctx: &TickContext) -> f32 {
    let period = 2.0 * BEAT_S;
    let ph = (ctx.show_time - T_BRIDGE).rem_euclid(period);
    let lub = (-ph / 0.10).exp();
    let dub = if ph >= 0.26 { 0.55 * (-(ph - 0.26) / 0.10).exp() } else { 0.0 };
    clamp01((lub + dub) as f32)
}

/// Cheap 6-segment rainbow, hue in [0, 1) → (r, g, b).
fn rainbow(h: f32) -> (f32, f32, f32) {
    let h = h.rem_euclid(1.0) * 6.0;
    let f = h.fract();
    match h as usize {
        0 => (1.0, f, 0.0),
        1 => (1.0 - f, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, 1.0 - f, 1.0),
        4 => (f, 0.0, 1.0),
        _ => (1.0, 0.0, 1.0 - f),
    }
}

// ─── Song sections ────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Eq)]
enum Section {
    Intro,
    Riff,
    Verse1,
    Chorus1,
    Shout1,
    Verse2,
    Slam,
    Verse2b,
    Chorus2,
    Shout2,
    Interlude,
    Build,
    StopHits,
    Solo,
    Bridge,
    Lift,
    FinalChorus,
    Stomp,
    Chant,
    Comedown,
    Outro,
    Fade,
}

fn section_at(ctx: &TickContext) -> Section {
    let t = ctx.show_time;
    if t < T_RIFF {
        Section::Intro
    } else if t < T_VERSE1 {
        Section::Riff
    } else if t < T_CHORUS1 {
        Section::Verse1
    } else if t < T_SHOUT1 {
        Section::Chorus1
    } else if t < T_VERSE2 {
        Section::Shout1
    } else if t < T_SLAM {
        Section::Verse2
    } else if t < T_VERSE2B {
        Section::Slam
    } else if t < T_CHORUS2 {
        Section::Verse2b
    } else if t < T_SHOUT2 {
        Section::Chorus2
    } else if t < T_INTERLUDE {
        Section::Shout2
    } else if t < T_BUILD {
        Section::Interlude
    } else if t < T_STOPHITS {
        Section::Build
    } else if t < T_SOLO {
        Section::StopHits
    } else if t < T_BRIDGE {
        Section::Solo
    } else if t < T_LIFT {
        Section::Bridge
    } else if t < T_FINAL {
        Section::Lift
    } else if t < T_STOMP {
        Section::FinalChorus
    } else if t < T_CHANT {
        Section::Stomp
    } else if t < T_COMEDOWN {
        Section::Chant
    } else if t < T_OUTRO {
        Section::Comedown
    } else if t < T_FADE {
        Section::Outro
    } else {
        Section::Fade
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RootPars – punk pogo chase; magenta floods in choruses; heartbeat bridge
// ─────────────────────────────────────────────────────────────────────────────

/// [Dimmer, Strobe, R, G, B, W, Amber, UV]
const PAR_OFF: [f32; 8] = [0.0; 8];
const PAR_WHITE_DUR_S: f64 = 0.08;
const PAR_WINK_DUR_S: f64 = 0.07;

struct SnlRootPar {
    step: u64,
}

impl SnlRootPar {
    fn active(&self, ctx: &TickContext) -> bool {
        let b = beat_idx(ctx);
        let s = if b < 0 { 0 } else { b as u64 % 4 };
        s == self.step
    }
}

impl Effect for SnlRootPar {
    fn channel_count(&self) -> usize {
        8
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = ctx.show_time;
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let active = self.active(ctx);
        let env = fade_env(ctx);

        // "Arschloch!" blasts override everything: brutal white strobe.
        if in_blast(ctx) {
            return if strobing(t, 14.0) {
                vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]
            } else {
                PAR_OFF.to_vec()
            };
        }
        // The rest of the shout/stop-hit sections is the band-stop: blackout.
        if matches!(section, Section::Shout1 | Section::Shout2 | Section::StopHits) {
            return PAR_OFF.to_vec();
        }

        // White downbeat slams in the big sections (all pars together).
        if matches!(section, Section::Chorus1 | Section::Chorus2 | Section::FinalChorus)
            && bib == 0
            && tb < PAR_WHITE_DUR_S
        {
            return vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        }

        let mut out = match section {
            Section::Intro => {
                // Feedback smolder: red ember swelling toward the riff.
                let r = time_ramp(ctx, 0.0, T_RIFF);
                let flicker = ((t * 9.0).sin() * (t * 2.3).sin()) as f32 * 0.04;
                let lvl = 0.04 + 0.20 * r * r + flicker;
                vec![clamp01(lvl), 0.0, 1.0, 0.0, 0.0, 0.0, 0.15, 0.0]
            }
            Section::Riff => {
                if active {
                    // Hard red pogo.
                    vec![0.85, 0.0, 1.0, 0.0, 0.05, 0.0, 0.0, 0.0]
                } else if (bib == 1 || bib == 3) && tb < PAR_WINK_DUR_S {
                    // Snare wink.
                    vec![0.40, 0.0, 0.0, 0.0, 0.0, 0.35, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Verse1 => {
                if active {
                    // Boot-stomp red/amber.
                    vec![0.55, 0.0, 1.0, 0.0, 0.0, 0.0, 0.55, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Chorus1 | Section::Chorus2 => {
                if active {
                    // The "Liebe": hot magenta flood.
                    vec![0.95, 0.0, 1.0, 0.0, 0.60, 0.0, 0.0, 0.20]
                } else if bib == 2 && tb < PAR_WINK_DUR_S {
                    // Backbeat pink wink on the resting pars.
                    vec![0.50, 0.0, 1.0, 0.0, 0.45, 0.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Verse2 => {
                if active {
                    // Introspective: blue-violet with UV.
                    vec![0.50, 0.0, 0.15, 0.0, 1.0, 0.0, 0.0, 0.50]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Slam | Section::Stomp => {
                // Every beat: ALL pars slam red and decay fast.
                let d = clamp01(1.0 - (tb / (BEAT_S * 0.8)) as f32);
                vec![0.15 + 0.85 * d, 0.0, 1.0, 0.0, 0.02, 0.0, 0.0, 0.0]
            }
            Section::Verse2b => {
                // Verse stomp slowly warming back toward the chorus magenta.
                let r = time_ramp(ctx, T_VERSE2B, T_CHORUS2);
                if active {
                    vec![0.55 + 0.20 * r, 0.0, 1.0, 0.0, 0.45 * r, 0.0, 0.45 * (1.0 - r), 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Interlude => {
                // Instrumental riff: rolling amber pairs.
                if (beat_idx(ctx).max(0) as u64 + self.step).is_multiple_of(2) {
                    vec![0.50, 0.0, 1.0, 0.10, 0.0, 0.0, 0.80, 0.0]
                } else {
                    vec![0.08, 0.0, 0.60, 0.0, 0.0, 0.0, 0.30, 0.0]
                }
            }
            Section::Build => {
                // Chase tightens and brightens into the stop-hits.
                let r = time_ramp(ctx, T_BUILD, T_STOPHITS);
                if active {
                    vec![0.50 + 0.50 * r, 0.0, 1.0, 0.0, 0.15 * r, 0.0, 0.30 * (1.0 - r), 0.25 * r]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Solo => {
                // Rainbow chase under the guitar solo.
                let (r, g, b) = rainbow((t * 0.10) as f32 + self.step as f32 * 0.25);
                let d = clamp01(1.0 - (tb / BEAT_S) as f32 * 0.6);
                if active {
                    vec![0.85 * d, 0.0, r, g, b, 0.0, 0.0, 0.0]
                } else {
                    vec![0.10, 0.0, r * 0.4, g * 0.4, b * 0.4, 0.0, 0.0, 0.0]
                }
            }
            Section::Bridge => {
                // Near-dark heartbeat: deep red lub-dub under UV.
                let hb = heartbeat(ctx);
                vec![0.04 + 0.55 * hb, 0.0, 1.0, 0.0, 0.10, 0.0, 0.0, 0.35]
            }
            Section::Lift => {
                // 1-bar riser: red → white, flicker accelerating.
                let r = time_ramp(ctx, T_LIFT, T_FINAL);
                let fl = if strobing(t, 8.0 + 16.0 * r as f64) { 1.0 } else { 0.55 };
                vec![(0.30 + 0.70 * r) * fl, 0.0, 1.0, 0.0, 0.20 * r, 0.85 * r, 0.0, 0.0]
            }
            Section::FinalChorus => {
                if active {
                    // Biggest look: saturated magenta, full power.
                    vec![1.0, 0.0, 1.0, 0.0, 0.55, 0.0, 0.0, 0.25]
                } else if bib == 2 && tb < PAR_WINK_DUR_S {
                    vec![0.65, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                } else {
                    PAR_OFF.to_vec()
                }
            }
            Section::Chant => {
                // "Arschloch!" on every downbeat: red+white slam, decaying.
                if bib == 0 {
                    let d = clamp01(1.0 - (tb / BEAT_S) as f32);
                    vec![0.30 + 0.70 * d, 0.0, 1.0, 0.0, 0.05, 0.70 * d, 0.0, 0.0]
                } else {
                    vec![0.15, 0.0, 1.0, 0.0, 0.05, 0.0, 0.0, 0.0]
                }
            }
            Section::Comedown => {
                // Warm amber pull-back.
                if active {
                    vec![0.40, 0.0, 1.0, 0.15, 0.0, 0.0, 0.70, 0.0]
                } else {
                    vec![0.12, 0.0, 0.70, 0.10, 0.0, 0.0, 0.40, 0.0]
                }
            }
            Section::Outro => {
                // Pogo afterglow: soft pink half-time chase.
                let half_active = (beat_idx(ctx).max(0) as u64 / 2) % 4 == self.step;
                if half_active {
                    vec![0.45, 0.0, 1.0, 0.0, 0.40, 0.0, 0.0, 0.10]
                } else {
                    vec![0.08, 0.0, 0.60, 0.0, 0.25, 0.0, 0.0, 0.0]
                }
            }
            Section::Fade => {
                vec![0.40, 0.0, 1.0, 0.0, 0.35, 0.0, 0.0, 0.10]
            }
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        // White snare winks on 2 & 4 in the stomping verses.
        if matches!(section, Section::Verse1 | Section::Verse2 | Section::Verse2b)
            && (bib == 1 || bib == 3)
            && tb < PAR_WINK_DUR_S
        {
            out = vec![0.25, 0.0, 0.0, 0.0, 0.0, 0.22, 0.0, 0.0];
        }

        out[0] = clamp01(out[0] * env);
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PixStrobes – pogo pixel flip, blinder hits, warm-white bridge breath
// ─────────────────────────────────────────────────────────────────────────────

const PIX_FLASH_DUR_S: f64 = 0.08;
const PIX_STROBE_DUR_S: f64 = 0.14;

struct SnlPixStrobe;

impl Effect for SnlPixStrobe {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = ctx.show_time;
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let beat_global = beat_idx(ctx).max(0) as usize;
        let env = fade_env(ctx);

        let mut out = vec![0.0f32; 32];

        // "Arschloch!" blast: everything white, hard strobe.
        if in_blast(ctx) {
            let on = strobing(t, 16.0);
            let v = if on { 1.0 } else { 0.0 };
            for ch in out.iter_mut().take(8) {
                *ch = v; // WW + CW full
            }
            for px in 0..8 {
                let ch = 8 + px * 3;
                out[ch] = v;
                out[ch + 1] = v;
                out[ch + 2] = v;
            }
            return out;
        }
        if matches!(section, Section::Shout1 | Section::Shout2 | Section::StopHits) {
            return out; // band-stop darkness
        }

        // Section palette for the RGB pixels.
        let (pr, pg, pb) = match section {
            Section::Intro => (0.30, 0.01, 0.0),
            Section::Riff => (1.0, 0.06, 0.06),
            Section::Verse1 => (1.0, 0.22, 0.0),
            Section::Chorus1 | Section::Chorus2 => (1.0, 0.04, 0.45),
            Section::Verse2 => (0.15, 0.05, 0.95),
            Section::Slam | Section::Stomp => (1.0, 0.0, 0.0),
            Section::Verse2b => (1.0, 0.15, 0.12),
            Section::Interlude => (1.0, 0.40, 0.0),
            Section::Build => (1.0, 0.08, 0.06),
            Section::Solo => (0.0, 0.0, 0.0), // rainbow handled below
            Section::Bridge => (0.65, 0.0, 0.10),
            Section::Lift => (1.0, 0.50, 0.50),
            Section::FinalChorus => (1.0, 0.02, 0.40),
            Section::Chant => (1.0, 0.04, 0.04),
            Section::Comedown => (1.0, 0.35, 0.05),
            Section::Outro | Section::Fade => (0.90, 0.10, 0.30),
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        let boost = match section {
            Section::Intro => 0.12 + 0.25 * time_ramp(ctx, 0.0, T_RIFF),
            Section::Riff => 0.85,
            Section::Verse1 | Section::Verse2 | Section::Verse2b => 0.60,
            Section::Chorus1 | Section::Chorus2 => 0.95,
            Section::Slam | Section::Stomp => 1.0,
            Section::Interlude => 0.55,
            Section::Build => 0.55 + 0.45 * time_ramp(ctx, T_BUILD, T_STOPHITS),
            Section::Solo => 0.80,
            Section::Bridge => 0.0, // heartbeat handled below
            Section::Lift => 0.60 + 0.40 * time_ramp(ctx, T_LIFT, T_FINAL),
            Section::FinalChorus | Section::Chant => 1.0,
            Section::Comedown => 0.45,
            Section::Outro | Section::Fade => 0.40,
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        } * env;

        // CW blinder strip (channels 4–7), beat-locked per section.
        let mut blinder = 0.0f32;
        match section {
            Section::Riff => {
                if bib == 0 && tb < PIX_FLASH_DUR_S {
                    blinder = 0.55;
                }
            }
            Section::Verse1 | Section::Verse2 | Section::Verse2b => {
                if (bib == 1 || bib == 3) && tb < PIX_FLASH_DUR_S {
                    blinder = 0.25;
                }
            }
            Section::Chorus1 | Section::Chorus2 => {
                if (bib == 0 || bib == 2) && tb < PIX_FLASH_DUR_S {
                    blinder = 1.0;
                }
                if bib == 0 && tb < PIX_STROBE_DUR_S {
                    blinder = if strobing(t, 20.0) { 1.0 } else { 0.0 };
                }
            }
            Section::Slam | Section::Stomp => {
                if tb < PIX_FLASH_DUR_S {
                    blinder = 0.80;
                }
            }
            Section::Build => {
                if bib == 0 && tb < PIX_FLASH_DUR_S {
                    blinder = 0.30 + 0.70 * time_ramp(ctx, T_BUILD, T_STOPHITS);
                }
            }
            Section::Lift => {
                blinder = if strobing(t, 12.0) { 0.50 + 0.50 * time_ramp(ctx, T_LIFT, T_FINAL) } else { 0.0 };
            }
            Section::FinalChorus => {
                if (bib == 0 || bib == 2) && tb < PIX_FLASH_DUR_S {
                    blinder = 1.0;
                }
                if bib == 0 && tb < PIX_STROBE_DUR_S {
                    blinder = if strobing(t, 24.0) { 1.0 } else { 0.0 };
                }
            }
            Section::Chant => {
                if bib == 0 && tb < 0.20 {
                    blinder = 1.0;
                }
            }
            _ => {}
        }
        let blinder = blinder * env;
        for ch in out.iter_mut().take(8).skip(4) {
            *ch = blinder;
        }

        // WW strip (channels 0–3): warmth where the song softens.
        let ww = match section {
            Section::Interlude => 0.12 + 0.10 * ((t * 2.0).sin() as f32 * 0.5 + 0.5),
            Section::Bridge => 0.04 + 0.10 * heartbeat(ctx),
            Section::Comedown => 0.22,
            Section::Outro => 0.10,
            _ => 0.0,
        } * env;
        for ch in out.iter_mut().take(4) {
            *ch = ww;
        }

        // RGB pixels.
        for px in 0..8usize {
            let ch = 8 + px * 3;
            let (r, g, b, lvl) = match section {
                Section::Solo => {
                    // Rainbow wave rolling along the strip.
                    let (r, g, b) = rainbow((t * 0.25) as f32 + px as f32 / 8.0);
                    (r, g, b, boost)
                }
                Section::Bridge => {
                    // Heartbeat in deep red, whole strip together.
                    let hb = heartbeat(ctx);
                    (pr, pg, pb, (0.03 + 0.45 * hb) * env)
                }
                _ => {
                    // Punk pogo: odd/even pixels flip-flop every beat.
                    let strong = (beat_global + px).is_multiple_of(2);
                    let pulse = if strong {
                        1.0 - 0.5 * (tb / BEAT_S) as f32
                    } else {
                        0.30
                    };
                    (pr, pg, pb, pulse * boost)
                }
            };
            out[ch] = r * lvl;
            out[ch + 1] = g * lvl;
            out[ch + 2] = b * lvl;
        }

        out
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// QuadPhases – section colour + rotation, beat-locked shutter
// ─────────────────────────────────────────────────────────────────────────────

// QuadPhase colour-wheel guesses (0..1); tune to the fixture's chart.
const QP_MAGENTA: f32 = 0.83;
const QP_RED: f32 = 0.95;
const QP_BLUE: f32 = 0.58;
const QP_AMBER: f32 = 0.08;
const QP_WHITE: f32 = 0.0;

struct SnlQuadPhase;

impl Effect for SnlQuadPhase {
    fn channel_count(&self) -> usize {
        4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = ctx.show_time;
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);

        // "Arschloch!" blast: white, spinning hard, manual strobe.
        if in_blast(ctx) {
            let open = strobing(t, 14.0);
            return vec![QP_WHITE, 0.90, 0.0, if open { 1.0 } else { 0.0 }];
        }
        if matches!(section, Section::Shout1 | Section::Shout2 | Section::StopHits) {
            return vec![QP_WHITE, 0.0, 0.0, 0.0];
        }

        let rotation = match section {
            Section::Intro => 0.06,
            Section::Riff => 0.50,
            Section::Verse1 | Section::Verse2 | Section::Verse2b => 0.35,
            Section::Chorus1 | Section::Chorus2 => 0.72,
            Section::Slam | Section::Stomp => 0.55,
            Section::Interlude => 0.25,
            Section::Build => 0.40 + 0.50 * time_ramp(ctx, T_BUILD, T_STOPHITS),
            Section::Solo => 0.80,
            Section::Bridge => 0.0,
            Section::Lift => 0.90,
            Section::FinalChorus | Section::Chant => 0.85,
            Section::Comedown => 0.20,
            Section::Outro | Section::Fade => 0.10,
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        let color = match section {
            Section::Intro => QP_RED,
            Section::Riff => QP_RED,
            Section::Verse1 => QP_RED,
            Section::Chorus1 | Section::Chorus2 => if bib == 0 { QP_WHITE } else { QP_MAGENTA },
            Section::Verse2 => QP_BLUE,
            Section::Slam | Section::Stomp => QP_RED,
            Section::Verse2b => QP_RED,
            Section::Interlude => QP_AMBER,
            Section::Build => QP_RED,
            // Colour wheel scroll under the solo.
            Section::Solo => (t * 0.05).fract() as f32,
            Section::Bridge => QP_RED,
            Section::Lift => QP_WHITE,
            Section::FinalChorus => if bib == 0 || bib == 2 { QP_WHITE } else { QP_MAGENTA },
            Section::Chant => if bib == 0 { QP_WHITE } else { QP_RED },
            Section::Comedown => QP_AMBER,
            Section::Outro | Section::Fade => QP_MAGENTA,
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        let open = match section {
            Section::Intro | Section::Bridge | Section::Fade => false,
            Section::Riff => tb < 0.12,
            Section::Verse1 | Section::Verse2 | Section::Verse2b => {
                (bib == 1 || bib == 3) && tb < 0.11
            }
            Section::Chorus1 | Section::Chorus2 => tb < 0.20,
            Section::Slam | Section::Stomp => tb < 0.15,
            Section::Interlude => tb < (BEAT_S * 0.5),
            Section::Build => tb < 0.10 + 0.12 * time_ramp(ctx, T_BUILD, T_STOPHITS) as f64,
            Section::Solo => true,
            Section::Lift => strobing(t, 10.0),
            Section::FinalChorus => tb < 0.26,
            Section::Chant => bib == 0,
            Section::Comedown => bib == 0 && tb < 0.10,
            Section::Outro => bib == 0 && tb < 0.08,
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        vec![color, rotation, 0.0, if open { 1.0 } else { 0.0 }]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scanners – pogo stabs, X-slams, rainbow solo, heartbeat bridge
// ─────────────────────────────────────────────────────────────────────────────

const SCAN_PAN_CTR_L: f32 = 0.62;
const SCAN_PAN_CTR_R: f32 = 0.38;
const SCAN_PAN_SPLAY_L: f32 = 0.22;
const SCAN_PAN_SPLAY_R: f32 = 0.78;
const SCAN_PAN_MID: f32 = 0.50;

const SCAN_TILT_STAGE: f32 = 0.50;
const SCAN_TILT_UP: f32 = 0.80;
const SCAN_TILT_CROWD: f32 = 0.42;

const SCAN_STAB_S: f64 = 0.10;

struct SnlScanner {
    reverse: bool,
}

impl SnlScanner {
    #[inline]
    fn side(&self, l: f32, r: f32) -> f32 {
        if self.reverse { r } else { l }
    }

    /// Per-beat step pattern shared by the stomping sections.
    fn step_pose(&self, bib: usize) -> (f32, f32) {
        match bib {
            0 => (SCAN_PAN_MID, SCAN_TILT_STAGE),
            1 => (self.side(SCAN_PAN_SPLAY_L, SCAN_PAN_SPLAY_R), SCAN_TILT_STAGE),
            2 => (SCAN_PAN_MID, SCAN_TILT_UP),
            _ => (self.side(SCAN_PAN_CTR_L, SCAN_PAN_CTR_R), SCAN_TILT_STAGE),
        }
    }
}

impl Effect for SnlScanner {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = ctx.show_time;
        let section = section_at(ctx);
        let tb = t_in_beat(ctx);
        let bib = beat_in_bar(ctx);
        let bar = bar_idx(ctx);
        let env = fade_env(ctx);

        // "Arschloch!" blast: both beams dead ahead into the room, strobing white.
        if in_blast(ctx) {
            return scanner_frame(
                SCAN_PAN_MID,
                SCAN_TILT_CROWD,
                SC_COLOR_WHITE,
                SC_SHUTTER_STROBE_MED,
                1.0,
                SC_GOBO_OPEN,
                SC_GOBOROT_NONE,
                SC_PRISM_OFF,
                SC_FOCUS_MID,
            );
        }
        if matches!(section, Section::Shout1 | Section::Shout2 | Section::StopHits) {
            return scanner_frame(
                SCAN_PAN_MID,
                SCAN_TILT_CROWD,
                SC_COLOR_WHITE,
                SC_SHUTTER_CLOSED,
                0.0,
                SC_GOBO_OPEN,
                SC_GOBOROT_NONE,
                SC_PRISM_OFF,
                SC_FOCUS_MID,
            );
        }

        let mut prism = SC_PRISM_OFF;

        let (pan, tilt, color) = match section {
            Section::Intro => {
                // Parked low, waiting in the dark (shutter stays closed).
                (SCAN_PAN_MID, SCAN_TILT_STAGE, SC_COLOR_RED)
            }
            Section::Riff => {
                // Wide splay, hammering white stabs.
                (self.side(SCAN_PAN_SPLAY_L, SCAN_PAN_SPLAY_R), SCAN_TILT_STAGE, SC_COLOR_WHITE)
            }
            Section::Verse1 => {
                let (p, tl) = self.step_pose(bib);
                (p, tl, SC_COLOR_RED)
            }
            Section::Chorus1 | Section::Chorus2 => {
                let (p, tl) = self.step_pose(bib);
                (p, tl, if bib == 0 { SC_COLOR_WHITE } else { SC_COLOR_PINK })
            }
            Section::Verse2 => {
                let (p, tl) = self.step_pose(bib);
                (p, tl, SC_COLOR_BLUE)
            }
            Section::Slam | Section::Stomp => {
                // X-cross: beams swap sides every beat.
                let flip = (beat_idx(ctx).max(0) % 2 == 0) ^ self.reverse;
                let pan = if flip { SCAN_PAN_SPLAY_L } else { SCAN_PAN_SPLAY_R };
                (pan, SCAN_TILT_STAGE, SC_COLOR_RED)
            }
            Section::Verse2b => {
                let (p, tl) = self.step_pose(bib);
                (p, tl, SC_COLOR_SALMON)
            }
            Section::Interlude => {
                // Slow green figure-8 over the room.
                let ph = if self.reverse { std::f64::consts::PI } else { 0.0 };
                let pan = 0.5 + 0.25 * ((t * 0.8 + ph).sin() as f32);
                let tilt = 0.58 + 0.16 * ((t * 1.6 + ph).sin() as f32);
                (pan, tilt, SC_COLOR_KGREEN)
            }
            Section::Build => {
                // Tighten toward center as it ramps.
                let r = time_ramp(ctx, T_BUILD, T_STOPHITS);
                let (p, tl) = self.step_pose(bib);
                (p + (SCAN_PAN_MID - p) * r, tl, SC_COLOR_RED)
            }
            Section::Solo => {
                // Rainbow lissajous party, prism on.
                prism = SC_PRISM_ROT;
                let ph = if self.reverse { std::f64::consts::PI } else { 0.0 };
                let pan = 0.5 + 0.28 * ((t * 3.4 + ph).sin() as f32);
                let tilt = 0.55 + 0.22 * ((t * 2.1 + ph * 0.5).sin() as f32);
                (pan, tilt, SC_COLOR_RAINBOW)
            }
            Section::Bridge => {
                // Slow drift skyward in deep blue, pulsing with the heartbeat.
                let d = (t * 0.25).sin() as f32;
                (SCAN_PAN_MID + self.side(-0.06, 0.06) * d, SCAN_TILT_UP, SC_COLOR_DKBLUE)
            }
            Section::Lift => {
                // Rise to the ceiling with the riser.
                let r = time_ramp(ctx, T_LIFT, T_FINAL);
                (SCAN_PAN_MID, SCAN_TILT_STAGE + (SCAN_TILT_UP - SCAN_TILT_STAGE) * r, SC_COLOR_WHITE)
            }
            Section::FinalChorus => {
                // Step pattern; every 4th bar the downbeat sweeps into an X.
                if bar % 4 == 3 && bib == 0 {
                    let frac = (tb / BEAT_S) as f32;
                    let from = self.side(SCAN_PAN_SPLAY_L, SCAN_PAN_SPLAY_R);
                    (from + (SCAN_PAN_MID - from) * frac, SCAN_TILT_STAGE, SC_COLOR_WHITE)
                } else {
                    let (p, tl) = self.step_pose(bib);
                    (p, tl, if bib == 0 { SC_COLOR_WHITE } else { SC_COLOR_PINK })
                }
            }
            Section::Chant => {
                // Punching straight into the crowd on every shout.
                (SCAN_PAN_MID, SCAN_TILT_CROWD, SC_COLOR_WHITE)
            }
            Section::Comedown => {
                (self.side(SCAN_PAN_CTR_L, SCAN_PAN_CTR_R), SCAN_TILT_STAGE, SC_COLOR_SALMON)
            }
            Section::Outro | Section::Fade => {
                // Lazy pink circles overhead, cooling down.
                let ph = if self.reverse { std::f64::consts::PI } else { 0.0 };
                let pan = 0.5 + 0.10 * ((t * 0.4 + ph).sin() as f32);
                let tilt = 0.72 + 0.06 * ((t * 0.4 + ph).cos() as f32);
                (pan, tilt, SC_COLOR_PINK)
            }
            Section::Shout1 | Section::Shout2 | Section::StopHits => unreachable!(),
        };

        // Shutter: hammering stabs in the stomping sections, open in the flowing ones.
        let stab_sections = matches!(
            section,
            Section::Riff
                | Section::Verse1
                | Section::Verse2
                | Section::Verse2b
                | Section::Chorus1
                | Section::Chorus2
                | Section::Build
                | Section::FinalChorus
        );
        let shutter = if matches!(section, Section::Intro) {
            SC_SHUTTER_CLOSED
        } else if matches!(section, Section::Slam | Section::Stomp) {
            // Hold the X visible for most of the beat.
            if tb < BEAT_S * 0.7 { SC_SHUTTER_OPEN } else { SC_SHUTTER_CLOSED }
        } else if matches!(section, Section::Chant) {
            if bib == 0 { SC_SHUTTER_STROBE_MED } else { SC_SHUTTER_CLOSED }
        } else if matches!(section, Section::Lift) {
            SC_SHUTTER_STROBE_MED
        } else if stab_sections {
            // X-sweep bars in the final chorus hold the beam open.
            let x_sweep = matches!(section, Section::FinalChorus) && bar % 4 == 3 && bib == 0;
            if x_sweep || tb < SCAN_STAB_S {
                SC_SHUTTER_OPEN
            } else {
                SC_SHUTTER_CLOSED
            }
        } else {
            SC_SHUTTER_OPEN
        };

        let base_dim = match section {
            Section::Bridge => 0.10 + 0.30 * heartbeat(ctx),
            Section::Interlude => 0.70,
            Section::Comedown => 0.55,
            Section::Outro | Section::Fade => 0.45,
            _ => 1.0,
        };
        let dimmer = clamp01(base_dim * env);

        scanner_frame(
            pan,
            tilt,
            color,
            shutter,
            dimmer,
            SC_GOBO_OPEN,
            SC_GOBOROT_NONE,
            prism,
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
            Binding::single(&fixtures::PIXSTROBE_1, SnlPixStrobe),
            Binding::single(&fixtures::PIXSTROBE_2, SnlPixStrobe),
            Binding::single(&fixtures::QUADPHASE_1, SnlQuadPhase),
            Binding::single(&fixtures::QUADPHASE_2, SnlQuadPhase),
            Binding::single(&fixtures::ROOTPAR_1, SnlRootPar { step: 0 }),
            Binding::single(&fixtures::ROOTPAR_2, SnlRootPar { step: 1 }),
            Binding::single(&fixtures::ROOTPAR_3, SnlRootPar { step: 2 }),
            Binding::single(&fixtures::ROOTPAR_4, SnlRootPar { step: 3 }),
            Binding::single(&fixtures::SCANNER_1, SnlScanner { reverse: false }),
            Binding::single(&fixtures::SCANNER_2, SnlScanner { reverse: true }),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tick every effect across the full song (and a little past the end) at
    /// 100 Hz and check that nothing panics, every output has the right
    /// channel count, and all values are finite and in [0, 1].
    #[test]
    fn full_show_sweep_is_sane() {
        let effects: Vec<(&str, Box<dyn Effect>)> = vec![
            ("pix", Box::new(SnlPixStrobe)),
            ("qp", Box::new(SnlQuadPhase)),
            ("par0", Box::new(SnlRootPar { step: 0 })),
            ("par3", Box::new(SnlRootPar { step: 3 })),
            ("scanL", Box::new(SnlScanner { reverse: false })),
            ("scanR", Box::new(SnlScanner { reverse: true })),
        ];
        let mut step = 0u64;
        let mut t: f64 = -0.5; // pre-roll before scene/song start
        while t < 260.0 {
            let ctx = TickContext {
                tick: step,
                time: t.max(0.0),
                show_time: t,
                wall_clock: chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
                dt: 0.01,
                slot: 0,
            };
            for (name, fx) in &effects {
                let out = fx.tick(&ctx);
                assert_eq!(out.len(), fx.channel_count(), "{name} at t={t}");
                for (i, v) in out.iter().enumerate() {
                    assert!(
                        v.is_finite() && (0.0..=1.0).contains(v),
                        "{name} ch{i} = {v} at t={t}"
                    );
                }
            }
            step += 1;
            t += 0.01;
        }
    }

    /// The "Arschloch!" blasts must light up and the band-stops must be dark.
    #[test]
    fn shout_moments_hit() {
        let at = |t: f64| TickContext {
            tick: 0,
            time: t,
            show_time: t,
            wall_clock: chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            dt: 0.01,
            slot: 0,
        };
        let par = SnlRootPar { step: 0 };
        // Mid-blast, on a strobe-on phase: white at full.
        for (a, b) in [BLAST1, BLAST2, PUNCH1, PUNCH2] {
            let mid = (a + b) / 2.0;
            let lit = (0..50).any(|k| {
                let out = par.tick(&at(mid - 0.125 + k as f64 * 0.005));
                out[0] > 0.9 && out[5] > 0.9
            });
            assert!(lit, "blast window {a}-{b} never lit");
        }
        // Band-stop just before blast 1: pars dark.
        let out = par.tick(&at(BLAST1.0 - 0.1));
        assert!(out[0] == 0.0, "band-stop before shout 1 not dark: {out:?}");
    }
}
