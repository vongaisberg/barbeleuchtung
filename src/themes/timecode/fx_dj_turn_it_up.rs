//! # FX Show – "DJ Turn It Up" (Dimension, 174 BPM drum & bass)
//!
//! High-energy roller, but **the quiet parts are protected**. Only the four
//! drop-halves go full-tilt; everything else (intro, verses, the breakdown,
//! the in-drop breaks, the pre-drop held breath and the outro) stays calm so
//! the room isn't pinned at 174 BPM for three minutes straight.
//!
//!   * **Energy reserved for the drops.** Drops get the white axe + hot chase
//!     + beam motion; every other section is a low colour wash with at most a
//!     soft per-beat pulse. The long drops ease back on the 4th bar of each
//!     phrase (`phrase_relax`) so they breathe.
//!   * **The breakdown (bars 64–79, ~22 s) is a genuine rest** – low blue/teal
//!     wash, slow swell, no strobe at all. Same for the 4-bar in-drop breaks
//!     and the 2-bar pre-drop held breath.
//!   * **PixStrobe blinders capped** at ~0.45 (drops) / ~0.20 (accents); pixel
//!     beds scaled ~0.30 (calm) / ~0.55 (drop). Short gated bursts only, never
//!     a continuous strobe.
//!   * **Ceiling scanners stay lit.** Shutter held OPEN the whole show; accents
//!     are position skips + cross-beam "X", colour changes at section
//!     boundaries only, dimmer ramps slowly. No per-beat shutter stabs.
//!
//! ## Beat grid (verified against the audio)
//! - **BPM 174.0** (matches published), **first downbeat t = 0.040 s**,
//!   beat 0.34483 s, bar 1.37931 s. Global onset-grid fit; downbeat chosen by
//!   the sharpest bar-aligned low-energy jump (0.413 vs next 0.336).
//! - Drops land on bar downbeats at 44.18 s (bar 32) and 132.45 s (bar 96) –
//!   exactly 64 bars apart.
//!
//! ## Arrangement (bar → time)
//! | Section    | Bars     | Time (s)       |
//! |------------|----------|----------------|
//! | Intro      | 0–7      | 0.04 – 11.07   |
//! | Verse 1    | 8–15     | 11.07 – 22.11  |
//! | Verse 2    | 16–27    | 22.11 – 38.66  |
//! | Build 1    | 28–31    | 38.66 – 44.18  |
//! | Drop 1a    | 32–43    | 44.18 – 60.73  |
//! | Break 1    | 44–47    | 60.73 – 66.25  |
//! | Drop 1b    | 48–59    | 66.25 – 82.80  |
//! | Break 2    | 60–63    | 82.80 – 88.32  |
//! | Breakdown  | 64–79    | 88.32 – 110.38 |
//! | Build 2    | 80–93    | 110.38 – 129.70|
//! | Pre-drop   | 94–95    | 129.70 – 132.45|
//! | Drop 2a    | 96–107   | 132.45 – 149.01|
//! | Break 3    | 108–111  | 149.01 – 154.52|
//! | Drop 2b    | 112–123  | 154.52 – 171.07|
//! | Outro      | 124–end  | 171.07 – 177.92|
//!
//! ## Palette – cool teal/blue at rest, hot red→magenta on the drops.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    scanner_frame, SC_COLOR_BLUE, SC_COLOR_DKBLUE, SC_COLOR_PINK, SC_COLOR_RED, SC_FOCUS_MID,
    SC_GOBO_OPEN, SC_GOBOROT_NONE, SC_PRISM_OFF, SC_SHUTTER_OPEN,
};

pub const NAME: &str = "DJ Turn It Up";

// ─── Tempo / beat grid (verified) ────────────────────────────────────────────

const BPM: f64 = 174.0;
const BEAT_S: f64 = 60.0 / BPM; // 0.34483… s per beat
const BAR_S: f64 = BEAT_S * 4.0; // 1.37931… s per bar
const PHASE_S: f64 = 0.040;
const SONG_END_S: f64 = 177.92;

// ─── Section boundaries (start bar of each section) ──────────────────────────

const BAR_INTRO: i64 = 0;
const BAR_VERSE1: i64 = 8;
const BAR_VERSE2: i64 = 16;
const BAR_BUILD1: i64 = 28;
const BAR_DROP1A: i64 = 32;
const BAR_BREAK1: i64 = 44;
const BAR_DROP1B: i64 = 48;
const BAR_BREAK2: i64 = 60;
const BAR_BREAKDOWN: i64 = 64;
const BAR_BUILD2: i64 = 80;
const BAR_PREDROP: i64 = 94;
const BAR_DROP2A: i64 = 96;
const BAR_BREAK3: i64 = 108;
const BAR_DROP2B: i64 = 112;
const BAR_OUTRO: i64 = 124;
const BAR_END: i64 = 128;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    PreRoll,
    Intro,
    Verse1,
    Verse2,
    Build1,
    Drop1a,
    Break1,
    Drop1b,
    Break2,
    Breakdown,
    Build2,
    PreDrop,
    Drop2a,
    Break3,
    Drop2b,
    Outro,
}

fn section_for_bar(bar: i64) -> Section {
    match bar {
        b if b < BAR_INTRO => Section::PreRoll,
        b if b < BAR_VERSE1 => Section::Intro,
        b if b < BAR_VERSE2 => Section::Verse1,
        b if b < BAR_BUILD1 => Section::Verse2,
        b if b < BAR_DROP1A => Section::Build1,
        b if b < BAR_BREAK1 => Section::Drop1a,
        b if b < BAR_DROP1B => Section::Break1,
        b if b < BAR_BREAK2 => Section::Drop1b,
        b if b < BAR_BREAKDOWN => Section::Break2,
        b if b < BAR_BUILD2 => Section::Breakdown,
        b if b < BAR_PREDROP => Section::Build2,
        b if b < BAR_DROP2A => Section::PreDrop,
        b if b < BAR_BREAK3 => Section::Drop2a,
        b if b < BAR_DROP2B => Section::Break3,
        b if b < BAR_OUTRO => Section::Drop2b,
        _ => Section::Outro,
    }
}

/// True for the four full-energy drop sub-sections.
fn is_drop(s: Section) -> bool {
    matches!(s, Section::Drop1a | Section::Drop1b | Section::Drop2a | Section::Drop2b)
}

// ─── Musical position helper ─────────────────────────────────────────────────

struct Musical {
    t: f64,
    beat: i64,
    bar: i64,
    beat_in_bar: usize,
    t_in_beat: f64,
}

fn musical(time: f64) -> Musical {
    let t = time - PHASE_S;
    if t < 0.0 {
        return Musical { t, beat: -1, bar: -1, beat_in_bar: 0, t_in_beat: 0.0 };
    }
    let beat = (t / BEAT_S).floor() as i64;
    Musical {
        t,
        beat,
        bar: beat.div_euclid(4),
        beat_in_bar: beat.rem_euclid(4) as usize,
        t_in_beat: t.rem_euclid(BEAT_S),
    }
}

fn section_progress(t: f64, start_bar: i64, n_bars: i64) -> f32 {
    let s = start_bar as f64 * BAR_S;
    (((t - s) / (n_bars as f64 * BAR_S)).clamp(0.0, 1.0)) as f32
}

fn beat_env(t_in_beat: f64, decay_s: f64) -> f32 {
    (-(t_in_beat / decay_s)).exp() as f32
}

fn breathe(t: f64, period_s: f64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::TAU * t / period_s).cos()) as f32
}

/// 1.0 for the first three bars of each 4-bar phrase, dropping to `lo` on the
/// fourth – gives the long drops a short breather each phrase instead of
/// staying pinned at full energy the whole time.
fn phrase_relax(m: &Musical, drop_start: i64, lo: f32) -> f32 {
    if (m.bar - drop_start).rem_euclid(4) == 3 {
        lo
    } else {
        1.0
    }
}

// ─── Colour palette (linear RGB, 0..1) ───────────────────────────────────────

const C_BLUE: [f32; 3] = [0.0, 0.12, 1.0];
const C_TEAL: [f32; 3] = [0.0, 0.85, 0.85];
const C_PURPLE: [f32; 3] = [0.45, 0.0, 1.0];
const C_RED: [f32; 3] = [1.0, 0.05, 0.0];
const C_MAGENTA: [f32; 3] = [1.0, 0.0, 0.45];
const C_WHITE3: [f32; 3] = [1.0, 1.0, 1.0];

fn lerp3(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
}

fn scale3(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Hot colour of a drop sub-section (Drop 1 = red, Drop 2 = magenta).
fn drop_rgb(start_bar: i64) -> [f32; 3] {
    if start_bar < BAR_BREAKDOWN { C_RED } else { C_MAGENTA }
}
fn drop_wheel(start_bar: i64) -> f32 {
    if start_bar < BAR_BREAKDOWN { SC_COLOR_RED } else { SC_COLOR_PINK }
}

// ════════════════════════════════════════════════════════════════════════════
//  RootPar – RGBW wash + energy meter   [Dimmer, Strobe, R, G, B, W, Amber, UV]
// ════════════════════════════════════════════════════════════════════════════

const PAR_OFF: [f32; 8] = [0.0; 8];

fn par(dimmer: f32, rgb: [f32; 3], white: f32) -> Vec<f32> {
    vec![dimmer, 0.0, rgb[0], rgb[1], rgb[2], white, 0.0, 0.0]
}

struct ShowPar {
    index: usize,
}

impl Effect for ShowPar {
    fn channel_count(&self) -> usize {
        8
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let m = musical(ctx.show_time);
        if ctx.show_time >= SONG_END_S {
            return PAR_OFF.to_vec();
        }
        match section_for_bar(m.bar) {
            Section::PreRoll => PAR_OFF.to_vec(),

            // Calm intro: very low blue wash, one par swelling slowly at a time.
            Section::Intro => {
                let active = (m.bar.rem_euclid(4)) as usize == self.index;
                let base = 0.06 + 0.06 * breathe(m.t, 2.0 * BAR_S);
                let pulse = 0.05 * beat_env(m.t_in_beat, 0.16);
                let dim = if active { base + 0.07 } else { base } + pulse;
                par(dim.min(1.0), C_BLUE, 0.0)
            }

            // Verses: gentle colour wash, soft per-beat pulse; verse 2 lifts a
            // touch toward the first drop but stays calm.
            Section::Verse1 => verse_par(&m, self.index, C_TEAL, 0.14, 0.20, 0.0),
            Section::Verse2 => {
                let lift = section_progress(m.t, BAR_VERSE2, BAR_BUILD1 - BAR_VERSE2);
                verse_par(&m, self.index, C_PURPLE, 0.16, 0.22, 0.10 * lift)
            }

            // Risers climb from calm toward the drop, with a held-breath blackout.
            Section::Build1 => build_par(&m, BAR_BUILD1, BAR_DROP1A - BAR_BUILD1, C_PURPLE),
            Section::Build2 => build_par(&m, BAR_BUILD2, BAR_PREDROP - BAR_BUILD2, C_TEAL),

            // Drops: the only full-energy section – white axe + hot chase.
            Section::Drop1a => drop_par(&m, self.index, BAR_DROP1A),
            Section::Drop1b => drop_par(&m, self.index, BAR_DROP1B),
            Section::Drop2a => drop_par(&m, self.index, BAR_DROP2A),
            Section::Drop2b => drop_par(&m, self.index, BAR_DROP2B),

            // In-drop breaks: pull right down, swell a riser back up at the end.
            Section::Break1 => break_par(&m, BAR_BREAK1, drop_rgb(BAR_DROP1B)),
            Section::Break2 => break_par(&m, BAR_BREAK2, C_BLUE),
            Section::Break3 => break_par(&m, BAR_BREAK3, drop_rgb(BAR_DROP2B)),

            // Breakdown: the long quiet rest – low blue wash, very slow swell.
            Section::Breakdown => {
                let active = (m.bar.rem_euclid(4)) as usize == self.index;
                let base = 0.06 + 0.07 * breathe(m.t, 4.0 * BAR_S);
                let dim = if active { base + 0.05 } else { base };
                par(dim.min(1.0), lerp3(C_BLUE, C_TEAL, 0.4), 0.0)
            }

            // Pre-drop: 2-bar held breath – a single slow swell toward white.
            Section::PreDrop => {
                let prog = section_progress(m.t, BAR_PREDROP, BAR_DROP2A - BAR_PREDROP);
                let dim = 0.05 + 0.18 * prog;
                par(dim.min(1.0), lerp3(C_TEAL, C_WHITE3, prog * 0.5), 0.0)
            }

            Section::Outro => {
                let fade = 1.0 - section_progress(m.t, BAR_OUTRO, BAR_END - BAR_OUTRO);
                let base = (0.07 + 0.10 * breathe(m.t, 3.0 * BAR_S)) * fade;
                par(base.min(1.0), C_BLUE, 0.0)
            }
        }
    }
}

/// Calm verse wash: a low floor plus a soft per-beat pulse (no hard chase).
fn verse_par(m: &Musical, index: usize, col: [f32; 3], floor_lit: f32, env_amt: f32, lift: f32) -> Vec<f32> {
    let lit = (m.beat.rem_euclid(4)) as usize == index;
    let env = beat_env(m.t_in_beat, 0.13);
    let floor = if lit { floor_lit } else { 0.07 };
    let dim = (floor + lift + env_amt * env).min(1.0);
    par(dim, col, 0.0)
}

/// Drop look: white flash on the downbeat, hot 4/4 chase, with a small
/// per-phrase relax so the long drops aren't relentless.
fn drop_par(m: &Musical, index: usize, drop_start: i64) -> Vec<f32> {
    if m.beat_in_bar == 0 && m.t_in_beat < 0.05 {
        return par(1.0, [0.0, 0.0, 0.0], 1.0);
    }
    let relax = phrase_relax(m, drop_start, 0.6);
    let col = drop_rgb(drop_start);
    let lit = (m.beat.rem_euclid(4)) as usize == index;
    if lit {
        let env = (0.45 + 0.45 * beat_env(m.t_in_beat, 0.10)) * relax;
        par(env.min(1.0), col, 0.0)
    } else {
        par(0.10 * relax, col, 0.0)
    }
}

/// In-drop break: calm hold for the first ~70 %, then a riser swelling toward
/// white in the final bar to set up the next drop half. No strobe.
fn break_par(m: &Musical, start_bar: i64, calm_col: [f32; 3]) -> Vec<f32> {
    let prog = section_progress(m.t, start_bar, 4);
    if prog < 0.70 {
        let dim = 0.08 + 0.07 * breathe(m.t, 2.0 * BAR_S);
        par(dim.min(1.0), calm_col, 0.0)
    } else {
        let r = (prog - 0.70) / 0.30; // 0→1 over the last bar
        let col = lerp3(calm_col, C_WHITE3, r);
        par((0.15 + 0.55 * r).min(1.0), col, (r * r).min(1.0))
    }
}

/// Riser: fade up from the section colour toward white, blackout the final ~12 %.
fn build_par(m: &Musical, start_bar: i64, n_bars: i64, from: [f32; 3]) -> Vec<f32> {
    let prog = section_progress(m.t, start_bar, n_bars);
    if prog > 0.88 {
        return PAR_OFF.to_vec();
    }
    let col = lerp3(from, C_WHITE3, prog);
    let dim = 0.12 + 0.70 * prog;
    let white = prog * prog;
    par(dim.min(1.0), col, white.min(1.0))
}

// ════════════════════════════════════════════════════════════════════════════
//  PixStrobe – CW blinder (ch 4–7) + 8 RGB pixels (ch 8–31).  Toned DOWN.
// ════════════════════════════════════════════════════════════════════════════

/// Hard ceilings so the strip never blinds the room.
const PIX_BLINDER_DROP: f32 = 0.45;
const PIX_BLINDER_ACCENT: f32 = 0.20;
const PIX_BED_CALM: f32 = 0.30;
const PIX_BED_DROP: f32 = 0.55;

fn pix(blinder: f32, pixels: [[f32; 3]; 8]) -> Vec<f32> {
    let mut out = vec![0.0f32; 32];
    for i in 4..8 {
        out[i] = blinder;
    }
    for (p, rgb) in pixels.iter().enumerate() {
        let ch = 8 + p * 3;
        out[ch] = rgb[0];
        out[ch + 1] = rgb[1];
        out[ch + 2] = rgb[2];
    }
    out
}

fn solid8(rgb: [f32; 3]) -> [[f32; 3]; 8] {
    [rgb; 8]
}

struct ShowPixStrobe {
    id: usize,
}

impl ShowPixStrobe {
    /// Soft moving comet across the 8 pixels (dim, for the calm sections).
    fn comet(&self, m: &Musical, base: [f32; 3], speed_beats: f64) -> [[f32; 3]; 8] {
        let head = (m.t / (BEAT_S * speed_beats)).rem_euclid(8.0);
        let mut px = [[0.0f32; 3]; 8];
        for (i, slot) in px.iter_mut().enumerate() {
            let pos = if self.id == 0 { i as f64 } else { 7.0 - i as f64 };
            let d = (pos - head).abs().min(8.0 - (pos - head).abs());
            let k = (1.0 - d / 2.5).max(0.0) as f32;
            *slot = scale3(base, 0.10 + 0.55 * k);
        }
        px
    }
}

impl Effect for ShowPixStrobe {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let m = musical(ctx.show_time);
        if ctx.show_time >= SONG_END_S {
            return pix(0.0, [[0.0; 3]; 8]);
        }
        match section_for_bar(m.bar) {
            Section::PreRoll => pix(0.0, [[0.0; 3]; 8]),

            Section::Intro => {
                let k = 0.10 + 0.12 * breathe(m.t, 2.0 * BAR_S);
                pix(0.0, self.comet(&m, scale3(C_BLUE, k + 0.25), 2.0))
            }

            Section::Verse1 => verse_pix(&m, C_TEAL),
            Section::Verse2 => verse_pix(&m, C_PURPLE),

            Section::Build1 => build_pix(&m, BAR_BUILD1, BAR_DROP1A - BAR_BUILD1, C_PURPLE),
            Section::Build2 => build_pix(&m, BAR_BUILD2, BAR_PREDROP - BAR_BUILD2, C_TEAL),

            Section::Drop1a => drop_pix(&m, BAR_DROP1A),
            Section::Drop1b => drop_pix(&m, BAR_DROP1B),
            Section::Drop2a => drop_pix(&m, BAR_DROP2A),
            Section::Drop2b => drop_pix(&m, BAR_DROP2B),

            Section::Break1 => break_pix(&m, BAR_BREAK1, drop_rgb(BAR_DROP1B)),
            Section::Break2 => break_pix(&m, BAR_BREAK2, C_BLUE),
            Section::Break3 => break_pix(&m, BAR_BREAK3, drop_rgb(BAR_DROP2B)),

            // Breakdown: the quiet rest – very dim slow comet, no blinder.
            Section::Breakdown => {
                let k = 0.06 + 0.08 * breathe(m.t, 4.0 * BAR_S);
                pix(0.0, self.comet(&m, scale3(lerp3(C_BLUE, C_TEAL, 0.4), k + 0.15), 4.0))
            }

            // Pre-drop: a single slow brightening glow, no strobe.
            Section::PreDrop => {
                let prog = section_progress(m.t, BAR_PREDROP, BAR_DROP2A - BAR_PREDROP);
                pix(0.0, solid8(scale3(lerp3(C_TEAL, C_WHITE3, prog * 0.5), 0.10 + 0.30 * prog)))
            }

            Section::Outro => {
                let fade = 1.0 - section_progress(m.t, BAR_OUTRO, BAR_END - BAR_OUTRO);
                let k = (0.08 + 0.10 * breathe(m.t, 3.0 * BAR_S)) * fade;
                pix(0.0, self.comet(&m, scale3(C_BLUE, k + 0.15), 4.0))
            }
        }
    }
}

/// Calm verse bed: low-brightness colour glow, faint blinder tap on the backbeat.
fn verse_pix(m: &Musical, base: [f32; 3]) -> Vec<f32> {
    let env = beat_env(m.t_in_beat, 0.12);
    let bed = scale3(base, PIX_BED_CALM * (0.5 + 0.5 * env));
    let backbeat = m.beat_in_bar == 1 || m.beat_in_bar == 3;
    let blind = if backbeat && m.t_in_beat < 0.05 { PIX_BLINDER_ACCENT } else { 0.0 };
    pix(blind, solid8(bed))
}

/// Drop: hot bed, a single dimmer-controlled blinder pop on the downbeat (not
/// every beat) so it accents without strobing the room white.
fn drop_pix(m: &Musical, drop_start: i64) -> Vec<f32> {
    let mut blinder = 0.0;
    if m.beat_in_bar == 0 && m.t_in_beat < 0.10 {
        // short ~20 Hz burst, capped well below full output
        let phase = (m.t * 20.0).fract();
        blinder = if phase < 0.5 { PIX_BLINDER_DROP } else { 0.0 };
    } else if m.beat_in_bar == 2 && m.t_in_beat < 0.05 {
        blinder = PIX_BLINDER_ACCENT;
    }
    let relax = phrase_relax(m, drop_start, 0.7);
    let env = 0.45 + 0.55 * beat_env(m.t_in_beat, 0.09);
    pix(blinder, solid8(scale3(drop_rgb(drop_start), PIX_BED_DROP * env * relax)))
}

/// In-drop break: dim calm bed, no blinder until the final bar, where a short
/// accelerating riser-strobe (capped at the accent level) sets up the drop.
fn break_pix(m: &Musical, start_bar: i64, calm_col: [f32; 3]) -> Vec<f32> {
    let prog = section_progress(m.t, start_bar, 4);
    if prog < 0.70 {
        let bed = scale3(calm_col, PIX_BED_CALM * (0.4 + 0.3 * breathe(m.t, 2.0 * BAR_S)));
        return pix(0.0, solid8(bed));
    }
    let r = (prog - 0.70) / 0.30;
    let rate = 8.0 + 14.0 * r as f64;
    let phase = (m.t * rate).fract();
    let blinder = if phase < 0.5 { (PIX_BLINDER_ACCENT + 0.2 * r).min(PIX_BLINDER_DROP) } else { 0.0 };
    let bed = scale3(lerp3(calm_col, C_WHITE3, r), (0.20 + 0.35 * r).min(PIX_BED_DROP));
    pix(blinder, solid8(bed))
}

/// Build: section-colour bed brightening, blinder strobe accelerating from
/// ~6→~22 Hz but capped at the accent level, then a held-breath blackout.
fn build_pix(m: &Musical, start_bar: i64, n_bars: i64, base: [f32; 3]) -> Vec<f32> {
    let prog = section_progress(m.t, start_bar, n_bars);
    if prog > 0.88 {
        return pix(0.0, [[0.0; 3]; 8]);
    }
    let rate = 6.0 + 16.0 * prog as f64;
    let phase = (m.t * rate).fract();
    let blinder = if phase < 0.5 { (PIX_BLINDER_ACCENT + 0.2 * prog).min(PIX_BLINDER_DROP) } else { 0.0 };
    let bed = scale3(lerp3(base, C_WHITE3, prog), (0.15 + 0.4 * prog).min(PIX_BED_DROP));
    pix(blinder, solid8(bed))
}

// ════════════════════════════════════════════════════════════════════════════
//  QuadPhase – derby/beam: [colour, rotation, strobe(unused), shutter]
// ════════════════════════════════════════════════════════════════════════════

const QP_COLOR_BLUE: f32 = 0.18;
const QP_COLOR_PURPLE: f32 = 0.55;
const QP_COLOR_RED: f32 = 0.42;
const QP_COLOR_TEAL: f32 = 0.30;
const QP_CLOSED: f32 = 0.0;

fn quad(color: f32, rotation: f32, shutter: f32) -> Vec<f32> {
    vec![color, rotation, 0.0, shutter]
}

struct ShowQuadPhase {
    id: usize,
}

impl ShowQuadPhase {
    fn rot(&self, speed: f32) -> f32 {
        let s = if self.id == 0 { speed } else { -speed };
        0.5 + 0.5 * s.clamp(-1.0, 1.0)
    }
}

impl Effect for ShowQuadPhase {
    fn channel_count(&self) -> usize {
        4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let m = musical(ctx.show_time);
        if ctx.show_time >= SONG_END_S {
            return quad(QP_COLOR_BLUE, self.rot(0.0), QP_CLOSED);
        }
        match section_for_bar(m.bar) {
            Section::PreRoll => quad(QP_COLOR_BLUE, self.rot(0.0), QP_CLOSED),

            // Calm: slow drift, gentle shutter breathe (no per-beat pulsing).
            Section::Intro => quad(QP_COLOR_BLUE, self.rot(0.08), 0.2 * breathe(m.t, 4.0 * BAR_S)),
            Section::Verse1 => quad(QP_COLOR_TEAL, self.rot(0.2), 0.30 * breathe(m.t, 2.0 * BAR_S)),
            Section::Verse2 => quad(QP_COLOR_PURPLE, self.rot(0.25), 0.35 * breathe(m.t, 2.0 * BAR_S)),

            Section::Build1 => build_quad(self, &m, BAR_BUILD1, BAR_DROP1A - BAR_BUILD1, QP_COLOR_PURPLE),
            Section::Build2 => build_quad(self, &m, BAR_BUILD2, BAR_PREDROP - BAR_BUILD2, QP_COLOR_TEAL),

            // Drops: fast counter-rotation, shutter stabs on the backbeat.
            Section::Drop1a | Section::Drop1b | Section::Drop2a | Section::Drop2b => {
                let backbeat = m.beat_in_bar == 1 || m.beat_in_bar == 3;
                let s = if backbeat && m.t_in_beat < 0.16 { 1.0 } else { 0.0 };
                quad(QP_COLOR_RED, self.rot(0.85), s)
            }

            // Breaks / breakdown / pre-drop: calm, slow drift, soft shutter breathe.
            Section::Break1 | Section::Break2 | Section::Break3 => {
                quad(QP_COLOR_TEAL, self.rot(0.15), 0.25 * breathe(m.t, 2.0 * BAR_S))
            }
            Section::Breakdown => quad(QP_COLOR_BLUE, self.rot(0.1), 0.20 * breathe(m.t, 4.0 * BAR_S)),
            Section::PreDrop => {
                let prog = section_progress(m.t, BAR_PREDROP, BAR_DROP2A - BAR_PREDROP);
                quad(QP_COLOR_TEAL, self.rot(0.1 + 0.4 * prog), 0.15 * prog)
            }

            Section::Outro => {
                let fade = 1.0 - section_progress(m.t, BAR_OUTRO, BAR_END - BAR_OUTRO);
                quad(QP_COLOR_BLUE, self.rot(0.08), 0.2 * breathe(m.t, 3.0 * BAR_S) * fade)
            }
        }
    }
}

fn build_quad(q: &ShowQuadPhase, m: &Musical, start_bar: i64, n_bars: i64, color: f32) -> Vec<f32> {
    let prog = section_progress(m.t, start_bar, n_bars);
    let s = if prog > 0.6 { (m.t * (8.0 + 14.0 * prog as f64)).fract() as f32 } else { 0.0 };
    let shutter = if prog > 0.88 { 0.0 } else { s };
    quad(color, q.rot(0.2 + 0.5 * prog), shutter)
}

// ════════════════════════════════════════════════════════════════════════════
//  Scanner – ceiling-mounted moving mirror.
//  Beam stays OPEN the whole show; choreography is position + (slow) colour /
//  dimmer changes only.  No shutter on/off, no per-beat dimmer stabs.
// ════════════════════════════════════════════════════════════════════════════

const SCAN_PAN_CENTER_L: f32 = 0.65;
const SCAN_PAN_CENTER_R: f32 = 0.35;
const SCAN_PAN_SPLAY_L: f32 = 0.20;
const SCAN_PAN_SPLAY_R: f32 = 0.80;
const SCAN_PAN_CENTER_BOTH: f32 = 0.50;

const SCAN_TILT_STAGE: f32 = 0.50;
const SCAN_TILT_UP: f32 = 0.80;

/// Cross-beam sweep duration (s) from splay to centre.
const SCAN_CROSS_DURATION_S: f64 = 0.25;

/// Steady beam – always open. Choreograph with pan/tilt/colour/dimmer instead.
fn frame(pan: f32, tilt: f32, color: f32, dimmer: f32) -> Vec<f32> {
    scanner_frame(
        pan,
        tilt,
        color,
        SC_SHUTTER_OPEN,
        dimmer,
        SC_GOBO_OPEN,
        SC_GOBOROT_NONE,
        SC_PRISM_OFF,
        SC_FOCUS_MID,
    )
}

struct ShowScanner {
    reverse: bool,
}

impl ShowScanner {
    fn center_pan(&self) -> f32 {
        if self.reverse { SCAN_PAN_CENTER_R } else { SCAN_PAN_CENTER_L }
    }
    fn splay_pan(&self) -> f32 {
        if self.reverse { SCAN_PAN_SPLAY_R } else { SCAN_PAN_SPLAY_L }
    }

    /// Slow continuous sweep for calm sections (position motion only).
    fn slow_sweep(&self, m: &Musical) -> (f32, f32) {
        let osc = breathe(m.t, 4.0 * BAR_S);
        let pan = if self.reverse {
            SCAN_PAN_SPLAY_R + (SCAN_PAN_CENTER_R - SCAN_PAN_SPLAY_R) * osc
        } else {
            SCAN_PAN_SPLAY_L + (SCAN_PAN_CENTER_L - SCAN_PAN_SPLAY_L) * osc
        };
        let tilt = SCAN_TILT_STAGE + (SCAN_TILT_UP - SCAN_TILT_STAGE) * breathe(m.t, 3.0 * BAR_S);
        (pan, tilt)
    }

    /// Beat-stepping preset skip – beam stays on, only the mirror moves.
    fn beat_step(&self, m: &Musical, color: f32, dimmer: f32) -> Vec<f32> {
        let (pan, tilt) = match m.beat_in_bar {
            0 => (self.center_pan(), SCAN_TILT_STAGE),
            1 => (self.splay_pan(), SCAN_TILT_STAGE),
            2 => (SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP),
            _ => (self.center_pan(), SCAN_TILT_STAGE),
        };
        frame(pan, tilt, color, dimmer)
    }

    /// Build: park up-centre, raise the dimmer slowly toward the drop. Beam stays
    /// open (no flicker, no blackout) so the ceiling mounts look settled.
    fn build_park(&self, m: &Musical, start_bar: i64, n_bars: i64, color: f32) -> Vec<f32> {
        let prog = section_progress(m.t, start_bar, n_bars);
        frame(SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP, color, (0.3 + 0.6 * prog).min(0.9))
    }

    /// Drop: hard position skips + cross-beam X on the last bar of each phrase.
    /// Colour stays the drop colour, beam stays open – energy from motion.
    fn drop_scan(&self, m: &Musical, drop_start: i64) -> Vec<f32> {
        let color = drop_wheel(drop_start);
        let bar_in_phrase = (m.bar - drop_start).rem_euclid(4);
        let t_phrase_bar0 = (drop_start + (m.bar - drop_start).div_euclid(4) * 4) as f64 * BAR_S;
        let cross_bar_start = t_phrase_bar0 + 3.0 * BAR_S;
        let t_cross = m.t - cross_bar_start;
        let in_cross = bar_in_phrase == 3 && t_cross >= 0.0 && t_cross < SCAN_CROSS_DURATION_S;

        if in_cross {
            let frac = (t_cross / SCAN_CROSS_DURATION_S) as f32;
            let from = self.splay_pan();
            let pan = from + (SCAN_PAN_CENTER_BOTH - from) * frac;
            frame(pan, SCAN_TILT_STAGE, color, 0.9)
        } else {
            self.beat_step(m, color, 0.9)
        }
    }
}

impl Effect for ShowScanner {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let m = musical(ctx.show_time);
        if ctx.show_time >= SONG_END_S {
            // End of show: park centre-up, beam dark via dimmer (slow), shutter
            // left open so we never snap the mechanical shutter.
            return frame(SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP, SC_COLOR_BLUE, 0.0);
        }
        let sec = section_for_bar(m.bar);
        if is_drop(sec) {
            let start = match sec {
                Section::Drop1a => BAR_DROP1A,
                Section::Drop1b => BAR_DROP1B,
                Section::Drop2a => BAR_DROP2A,
                _ => BAR_DROP2B,
            };
            return self.drop_scan(&m, start);
        }
        match sec {
            // Pre-roll: parked, dimmer down (beam open but dark).
            Section::PreRoll => frame(SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP, SC_COLOR_BLUE, 0.0),

            Section::Intro => {
                let (pan, tilt) = self.slow_sweep(&m);
                let dim = 0.18 + 0.15 * breathe(m.t, 2.0 * BAR_S);
                frame(pan, tilt, SC_COLOR_BLUE, dim)
            }

            // Calm verses: slow sweeps, steady mid dimmer (no beat stepping).
            Section::Verse1 => {
                let (pan, tilt) = self.slow_sweep(&m);
                frame(pan, tilt, SC_COLOR_DKBLUE, 0.40)
            }
            Section::Verse2 => {
                let (pan, tilt) = self.slow_sweep(&m);
                frame(pan, tilt, SC_COLOR_PINK, 0.45)
            }

            Section::Build1 => self.build_park(&m, BAR_BUILD1, BAR_DROP1A - BAR_BUILD1, SC_COLOR_PINK),
            Section::Build2 => self.build_park(&m, BAR_BUILD2, BAR_PREDROP - BAR_BUILD2, SC_COLOR_DKBLUE),

            // In-drop breaks: slow sweep, lower dimmer, lifting toward the drop.
            Section::Break1 | Section::Break2 | Section::Break3 => {
                let start = match sec {
                    Section::Break1 => BAR_BREAK1,
                    Section::Break2 => BAR_BREAK2,
                    _ => BAR_BREAK3,
                };
                let (pan, tilt) = self.slow_sweep(&m);
                let prog = section_progress(m.t, start, 4);
                frame(pan, tilt, SC_COLOR_DKBLUE, 0.25 + 0.45 * prog)
            }

            // Breakdown: the quiet rest – very slow sweep, low steady dimmer.
            Section::Breakdown => {
                let (pan, tilt) = self.slow_sweep(&m);
                frame(pan, tilt, SC_COLOR_BLUE, 0.30)
            }

            // Pre-drop: ease up-centre and lift the dimmer into the drop.
            Section::PreDrop => {
                let prog = section_progress(m.t, BAR_PREDROP, BAR_DROP2A - BAR_PREDROP);
                frame(SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP, SC_COLOR_PINK, 0.35 + 0.5 * prog)
            }

            Section::Outro => {
                let (pan, tilt) = self.slow_sweep(&m);
                let fade = 1.0 - section_progress(m.t, BAR_OUTRO, BAR_END - BAR_OUTRO);
                frame(pan, tilt, SC_COLOR_BLUE, (0.15 + 0.15 * breathe(m.t, 3.0 * BAR_S)) * fade)
            }

            // Drops handled above; unreachable here.
            _ => frame(SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP, SC_COLOR_BLUE, 0.0),
        }
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            Binding::single(&fixtures::PIXSTROBE_1, ShowPixStrobe { id: 0 }),
            Binding::single(&fixtures::PIXSTROBE_2, ShowPixStrobe { id: 1 }),
            Binding::single(&fixtures::QUADPHASE_1, ShowQuadPhase { id: 0 }),
            Binding::single(&fixtures::QUADPHASE_2, ShowQuadPhase { id: 1 }),
            Binding::single(&fixtures::ROOTPAR_1, ShowPar { index: 0 }),
            Binding::single(&fixtures::ROOTPAR_2, ShowPar { index: 1 }),
            Binding::single(&fixtures::ROOTPAR_3, ShowPar { index: 2 }),
            Binding::single(&fixtures::ROOTPAR_4, ShowPar { index: 3 }),
            Binding::single(&fixtures::SCANNER_1, ShowScanner { reverse: false }),
            Binding::single(&fixtures::SCANNER_2, ShowScanner { reverse: true }),
        ],
    )
}
