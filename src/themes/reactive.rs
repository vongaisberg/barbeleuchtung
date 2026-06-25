//! Audio-reactive bank — a clean, bold, beat-locked club show.
//!
//! Two ways to drive the rig, both in this module:
//!
//!   * **Auto (the generative engine)** — the [`Director`] reads the reliable
//!     live signals (beat grid, `intensity`, `four_on_floor`) plus a per-phrase
//!     variation seed and produces [`ShowParams`]; the `Gen*` effects render
//!     from it via `ctx.show`. Variety every phrase, bold but clean, strobes
//!     reserved for genuine peaks. This is what the engine renders when Auto is
//!     on. (See the "Generative engine" section near the bottom.)
//!   * **Manual looks** — Dark / Groove / Club / Strobe, fixed scenes the
//!     operator can pin instead of Auto. Each emphasises a different gesture so
//!     they don't look interchangeable.
//!
//! Shared principles: hold colour over a phrase (no per-beat hue flicker); keep
//! the strobe beat-locked (`locked = beat_confidence > CONF_GATE`) so it never
//! free-runs; reserve white punch / blinder / scanner breakouts for peaks;
//! scanners stay lit (shutter open) and move smoothly. Effects are stateless —
//! all timing comes from `ctx`.
//!
//! Operator controls (`ctx.controls`): `sensitivity` scales perceived energy,
//! `strobe` scales (0 disables) every white strobe gesture, and the engine
//! applies `master` brightness to the whole bank.

use crate::audio::AudioFeatures;
use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};

use super::{
    scanner_frame, SC_COLOR_BLUE, SC_COLOR_GREEN, SC_COLOR_PINK, SC_COLOR_RED, SC_COLOR_WHITE,
    SC_COLOR_YELLOW, SC_FOCUS_MID, SC_GOBO_1, SC_GOBO_2, SC_GOBO_3, SC_GOBO_4, SC_GOBO_OPEN,
    SC_GOBOROT_NONE, SC_PRISM_OFF, SC_PRISM_ROT, SC_SHUTTER_OPEN,
};

// ===========================================================================
// Palettes
// ===========================================================================

#[derive(Clone, Copy, Debug)]
pub struct PColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub w: f32,
    pub a: f32,
    pub uv: f32,
}

impl PColor {
    const fn full(r: f32, g: f32, b: f32, w: f32, a: f32, uv: f32) -> Self {
        Self { r, g, b, w, a, uv }
    }
    fn lerp(self, o: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let l = |x: f32, y: f32| x + (y - x) * t;
        Self { r: l(self.r, o.r), g: l(self.g, o.g), b: l(self.b, o.b), w: l(self.w, o.w), a: l(self.a, o.a), uv: l(self.uv, o.uv) }
    }
    fn rgb_at(self, k: f32) -> (f32, f32, f32) {
        (self.r * k, self.g * k, self.b * k)
    }
}

pub struct Palette {
    pub name: &'static str,
    pub colors: &'static [PColor],
    pub scanner_color: f32,
}

static NEON: &[PColor] = &[
    PColor::full(0.0, 1.0, 1.0, 0.0, 0.0, 0.2),
    PColor::full(1.0, 0.0, 1.0, 0.0, 0.0, 0.4),
    PColor::full(0.1, 0.2, 1.0, 0.0, 0.0, 0.5),
    PColor::full(0.5, 0.0, 1.0, 0.0, 0.0, 0.8),
];
static WARM: &[PColor] = &[
    PColor::full(1.0, 0.0, 0.0, 0.0, 0.15, 0.0),
    PColor::full(1.0, 0.30, 0.0, 0.0, 0.55, 0.0),
    PColor::full(1.0, 0.0, 0.40, 0.0, 0.0, 0.0),
    PColor::full(0.8, 0.0, 0.10, 0.0, 0.0, 0.0),
];
static COOL: &[PColor] = &[
    PColor::full(0.0, 0.20, 1.0, 0.0, 0.0, 0.0),
    PColor::full(0.0, 0.80, 1.0, 0.0, 0.0, 0.0),
    PColor::full(0.45, 0.0, 1.0, 0.0, 0.0, 0.3),
    PColor::full(0.0, 0.05, 0.7, 0.0, 0.0, 0.0),
];
static ACID: &[PColor] = &[
    PColor::full(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
    PColor::full(0.6, 1.0, 0.0, 0.0, 0.0, 0.0),
    PColor::full(1.0, 0.9, 0.0, 0.0, 0.2, 0.0),
    PColor::full(0.1, 1.0, 0.3, 0.0, 0.0, 0.1),
];

static PALETTES: &[Palette] = &[
    Palette { name: "Neon+UV", colors: NEON, scanner_color: SC_COLOR_PINK },
    Palette { name: "Warm", colors: WARM, scanner_color: SC_COLOR_RED },
    Palette { name: "Cool", colors: COOL, scanner_color: SC_COLOR_BLUE },
    Palette { name: "Acid", colors: ACID, scanner_color: SC_COLOR_GREEN },
];

const PHRASE_BEATS: f64 = 16.0;
const PALETTE_ROTATE_BEATS: f64 = 128.0;
const CONF_GATE: f32 = 0.40;

// ===========================================================================
// Musical / control helpers
// ===========================================================================

fn beats(ctx: &TickContext) -> f64 {
    let a = &ctx.audio;
    if a.bpm > 0.0 { a.beat_clock } else { ctx.time * 2.0 }
}

fn locked(a: &AudioFeatures) -> bool {
    a.beat_confidence > CONF_GATE && a.bpm > 0.0
}

/// Perceived energy after the operator's sensitivity.
fn energy(ctx: &TickContext) -> f32 {
    (ctx.audio.energy * ctx.controls.sensitivity).clamp(0.0, 1.0)
}

fn onset(ctx: &TickContext) -> f32 {
    (ctx.audio.onset * ctx.controls.sensitivity).clamp(0.0, 1.0)
}

fn beat_in_bar(ctx: &TickContext) -> i64 {
    if locked(&ctx.audio) { (beats(ctx).floor() as i64).rem_euclid(4) } else { -1 }
}

/// Decaying per-beat accent when locked; onset-driven otherwise.
fn beat_accent(ctx: &TickContext, decay: f32) -> f32 {
    let a = &ctx.audio;
    if locked(a) { (-(a.beat_phase / decay)).exp() } else { onset(ctx) }
}

/// A crisp, beat-locked strobe gate on a beat subdivision. Returns 0.0 unless
/// the tempo is locked — so it can never free-run into a random flicker.
/// `subdiv` = 1 → quarter notes, 2 → eighths. Result already includes the
/// operator's strobe amount.
fn beat_strobe(ctx: &TickContext, subdiv: f64, duty: f32) -> f32 {
    let a = &ctx.audio;
    if !locked(a) {
        return 0.0;
    }
    let ph = ((a.beat_phase as f64) * subdiv).rem_euclid(1.0) as f32;
    if ph < duty { ctx.controls.strobe.clamp(0.0, 1.0) } else { 0.0 }
}

fn breathe(t: f64, period_s: f64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::TAU * t / period_s).cos()) as f32
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Peak score: intense AND grooving (a drop/chorus). Replaces the old flaky
/// discrete `drop` event as the gate for the big gestures.
fn peak(ctx: &TickContext) -> f32 {
    let a = &ctx.audio;
    a.intensity * (0.30 + 0.70 * a.four_on_floor)
}

/// Big-moment gate: a real peak or sustained high energy.
fn special(ctx: &TickContext) -> bool {
    peak(ctx) > 0.45 || energy(ctx) > 0.82
}

fn active_palette(ctx: &TickContext) -> &'static Palette {
    let idx = (beats(ctx) / PALETTE_ROTATE_BEATS) as usize % PALETTES.len();
    &PALETTES[idx]
}

fn pal_color(pal: &Palette, idx: i64) -> PColor {
    let n = pal.colors.len();
    let k = (((idx as f64) * 0.618_034).rem_euclid(1.0) * n as f64) as usize % n;
    pal.colors[k]
}

/// Held phrase colour with a 1-beat crossfade at each phrase boundary.
fn phrase_color(ctx: &TickContext) -> PColor {
    let pal = active_palette(ctx);
    let p = beats(ctx) / PHRASE_BEATS;
    let idx = p.floor() as i64;
    let into = (p.fract() * PHRASE_BEATS) as f32;
    pal_color(pal, idx - 1).lerp(pal_color(pal, idx), smoothstep(into.min(1.0)))
}

// ===========================================================================
// Effects
// ===========================================================================

/// RootPars. Normally a held colour wash with a soft per-beat pulse. In
/// `white_strobe` mode (the Strobe look) they become hard white beat-punches
/// instead — a distinct, peak-only gesture.
pub struct ReactivePars {
    pub floor: f32,
    pub gain: f32,
    pub pulse: f32,
    pub white_strobe: bool,
}

impl Effect for ReactivePars {
    fn channel_count(&self) -> usize {
        8
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let a = &ctx.audio;

        if self.white_strobe {
            // Hard white beat-punch: bright on the beat, dim between. Falls back
            // to energy-driven white when the beat isn't locked.
            let s = beat_strobe(ctx, 1.0, 0.16);
            let lvl = if locked(a) {
                0.12 + 0.88 * s
            } else {
                0.25 + 0.5 * energy(ctx)
            };
            let l = lvl.clamp(0.0, 1.0);
            return vec![l, 0.0, 0.0, 0.0, 0.0, l, 0.0, 0.0];
        }

        let c = phrase_color(ctx);
        let breath = 0.5 + 0.5 * breathe(ctx.time, 4.0);
        let base = self.floor + self.gain * energy(ctx) * (0.7 + 0.3 * breath);

        let bib = beat_in_bar(ctx);
        let pulse_scale = if bib == 0 { 1.0 } else { 0.55 };
        let mut dim = base + self.pulse * beat_accent(ctx, 0.26) * pulse_scale;
        if ctx.slot % 2 == 1 {
            dim *= 0.82;
        }
        dim = dim.clamp(0.0, 1.0);

        // White punch only on a drop's downbeat.
        let mut w = c.w;
        if special(ctx) && bib == 0 && a.beat_phase < 0.10 {
            w = 1.0;
            dim = dim.max(0.9);
        }

        vec![dim, 0.0, (c.r * dim).clamp(0.0, 1.0), (c.g * dim).clamp(0.0, 1.0), (c.b * dim).clamp(0.0, 1.0), w.clamp(0.0, 1.0), c.a.clamp(0.0, 1.0), c.uv.clamp(0.0, 1.0)]
    }
}

/// PixStrobe modes — each look uses a different one so they read differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixMode {
    /// Soft colour bed (optionally with a single moving comet pixel).
    Bed { comet: bool },
    /// A beat-synced 2-pixel colour block chasing across the bar — rhythmic.
    Chase,
    /// Dim colour bed under a hard white eighth-note strobe (peak look).
    Strobe,
}

pub struct ReactivePix {
    pub mode: PixMode,
    pub bed: f32,
    /// White blinder amount fired on drops (0 disables). Used in Bed mode.
    pub blinder: f32,
}

impl ReactivePix {
    fn set_seg(v: &mut [f32], seg: usize, rgb: (f32, f32, f32)) {
        let base = 8 + seg * 3;
        v[base] = rgb.0.clamp(0.0, 1.0);
        v[base + 1] = rgb.1.clamp(0.0, 1.0);
        v[base + 2] = rgb.2.clamp(0.0, 1.0);
    }
}

impl Effect for ReactivePix {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let a = &ctx.audio;
        let c = phrase_color(ctx);
        let mut v = vec![0.0f32; 32];

        match self.mode {
            PixMode::Bed { comet } => {
                let bed = (self.bed * (0.45 + 0.55 * energy(ctx)) + 0.18 * self.bed * beat_accent(ctx, 0.30)).clamp(0.0, 1.0);
                let cpos = if comet {
                    Some(if locked(a) { (beats(ctx).floor() as i64).rem_euclid(8) as usize } else { ((ctx.time * 4.0) as i64).rem_euclid(8) as usize })
                } else {
                    None
                };
                for seg in 0..8 {
                    let k = if cpos == Some(seg) { (bed + 0.5).min(1.0) } else { bed };
                    Self::set_seg(&mut v, seg, c.rgb_at(k));
                }
                // White blinder reserved for drops — crisp quarter-note flashes.
                if self.blinder > 0.0 && peak(ctx) > 0.45 {
                    let s = beat_strobe(ctx, 1.0, 0.16) * self.blinder * peak(ctx);
                    for cell in v.iter_mut().take(8) {
                        *cell = s.clamp(0.0, 1.0);
                    }
                }
            }
            PixMode::Chase => {
                // A 2-pixel block in the phrase colour advancing one step per
                // beat (locked) — a clean rhythmic gesture, not flicker.
                let step = if locked(a) { (beats(ctx).floor() as i64).rem_euclid(8) } else { ((ctx.time / 0.4) as i64).rem_euclid(8) };
                let lvl = (self.bed * (0.55 + 0.45 * energy(ctx))).clamp(0.0, 1.0);
                for seg in 0..8 {
                    let s = seg as i64;
                    let on = s == step || s == (step + 1).rem_euclid(8);
                    let k = if on { lvl } else { lvl * 0.12 };
                    Self::set_seg(&mut v, seg, c.rgb_at(k));
                }
            }
            PixMode::Strobe => {
                let bed = (self.bed * 0.4).clamp(0.0, 1.0);
                for seg in 0..8 {
                    Self::set_seg(&mut v, seg, c.rgb_at(bed));
                }
                // Hard white eighth-note strobe while the peak is live.
                if energy(ctx) > 0.6 || peak(ctx) > 0.45 {
                    let s = beat_strobe(ctx, 2.0, 0.18) * self.blinder.max(0.7);
                    for cell in v.iter_mut().take(8) {
                        *cell = s.clamp(0.0, 1.0);
                    }
                }
            }
        }
        v
    }
}

/// QuadPhase derby: rotation rides energy, colour steps per phrase, internal
/// strobe only on a drop.
pub struct ReactiveDerby {
    pub active: bool,
}

impl Effect for ReactiveDerby {
    fn channel_count(&self) -> usize {
        4
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        if !self.active {
            return vec![0.0, 0.0, 0.0, 0.0];
        }
        let phrase = (beats(ctx) / PHRASE_BEATS).floor();
        let color = ((phrase * 0.27).rem_euclid(1.0)) as f32;
        let rotation = (0.28 + 0.55 * energy(ctx)).clamp(0.0, 1.0);
        let strobe = if peak(ctx) > 0.5 { 0.6 } else { 0.0 };
        vec![color, rotation, strobe, 1.0]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    Park,
    Drift,
    Stepped,
    Sweep,
}

/// SC-X50 scanner: shutter always open, smooth bar-synced moves, colour held
/// per phrase, dimmer riding energy slowly. Breakout (gobo+prism+white) on a
/// special moment.
pub struct ReactiveScanner {
    pub reverse: bool,
    pub movement: Movement,
    pub bright: f32,
}

impl ReactiveScanner {
    fn pan_tilt(&self, ctx: &TickContext) -> (f32, f32) {
        let b = beats(ctx);
        let tau = std::f64::consts::TAU;
        let (pan, tilt) = match self.movement {
            Movement::Park => (0.5, 0.70),
            Movement::Drift => (
                0.5 + 0.16 * (tau * ctx.time / 18.0).sin() as f32,
                0.64 + 0.12 * (tau * ctx.time / 24.0).sin() as f32,
            ),
            Movement::Stepped => {
                let idx = b.floor() as i64;
                let preset = |i: i64| -> (f32, f32) {
                    match i.rem_euclid(4) {
                        0 => (0.64, 0.55),
                        1 => (0.36, 0.55),
                        2 => (0.50, 0.78),
                        _ => (0.44, 0.58),
                    }
                };
                let (p0, t0) = preset(idx);
                let (p1, t1) = preset(idx + 1);
                let f = smoothstep(b.fract() as f32);
                (p0 + (p1 - p0) * f, t0 + (t1 - t0) * f)
            }
            Movement::Sweep => {
                let i = energy(ctx);
                let amp = 0.10 + 0.24 * i;
                (
                    0.5 + amp * (tau * b / 8.0).sin() as f32,
                    0.62 + (0.08 + 0.14 * i) * (tau * b / 12.0).sin() as f32,
                )
            }
        };
        let pan = if self.reverse { 1.0 - pan } else { pan };
        (pan.clamp(0.0, 1.0), tilt.clamp(0.45, 0.88))
    }
}

impl Effect for ReactiveScanner {
    fn channel_count(&self) -> usize {
        11
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let pal = active_palette(ctx);
        let (pan, tilt) = self.pan_tilt(ctx);
        let sp = special(ctx);
        let color = if sp { SC_COLOR_WHITE } else { pal.scanner_color };
        let gobo = if sp { SC_GOBO_1 } else { SC_GOBO_OPEN };
        let prism = if sp { SC_PRISM_ROT } else { SC_PRISM_OFF };
        let base = if self.movement == Movement::Park { 0.30 } else { 0.50 };
        let dim = (self.bright * (base + 0.50 * energy(ctx))).clamp(0.0, 1.0);
        scanner_frame(pan, tilt, color, SC_SHUTTER_OPEN, dim, gobo, SC_GOBOROT_NONE, prism, SC_FOCUS_MID)
    }
}

// ===========================================================================
// Looks — each emphasises a different gesture so they read as distinct.
// ===========================================================================

pub const OFF_NAME: &str = "Aus";
pub const DARK_NAME: &str = "Dark";
pub const GROOVE_NAME: &str = "Groove";
pub const CLUB_NAME: &str = "Club";
pub const STROBE_NAME: &str = "Strobe";

fn pars(floor: f32, gain: f32, pulse: f32, white_strobe: bool) -> Binding {
    Binding::group(
        vec![&fixtures::ROOTPAR_1, &fixtures::ROOTPAR_2, &fixtures::ROOTPAR_3, &fixtures::ROOTPAR_4],
        ReactivePars { floor, gain, pulse, white_strobe },
    )
}

fn pix(mode: PixMode, bed: f32, blinder: f32) -> Vec<Binding> {
    vec![
        Binding::single(&fixtures::PIXSTROBE_1, ReactivePix { mode, bed, blinder }),
        Binding::single(&fixtures::PIXSTROBE_2, ReactivePix { mode, bed, blinder }),
    ]
}

fn derbies(active: bool) -> Vec<Binding> {
    vec![
        Binding::single(&fixtures::QUADPHASE_1, ReactiveDerby { active }),
        Binding::single(&fixtures::QUADPHASE_2, ReactiveDerby { active }),
    ]
}

fn scanners(movement: Movement, bright: f32) -> Vec<Binding> {
    vec![
        Binding::single(&fixtures::SCANNER_1, ReactiveScanner { reverse: false, movement, bright }),
        Binding::single(&fixtures::SCANNER_2, ReactiveScanner { reverse: true, movement, bright }),
    ]
}

fn compose(name: &'static str, mut bindings: Vec<Binding>, extra: Vec<Vec<Binding>>) -> Theme {
    for mut e in extra {
        bindings.append(&mut e);
    }
    Theme::new(name, Transition::Crossfade { duration_ms: 800 }, bindings)
}

pub fn off() -> Theme {
    Theme::new(OFF_NAME, Transition::Instant, vec![])
}

/// Dark — deep held colour, slow breathing wash, slow crossing beams, no
/// derby/strobe. Calm and intentional (not a dim mess).
pub fn dark() -> Theme {
    compose(
        DARK_NAME,
        vec![pars(0.14, 0.20, 0.0, false)],
        vec![
            pix(PixMode::Bed { comet: false }, 0.10, 0.0),
            derbies(false),
            scanners(Movement::Drift, 0.45),
        ],
    )
}

/// Groove — colourful and rhythmic: beat block-chase on the bars, derbies,
/// soft par pulse, smooth stepped scanners. No big sweeps, no blinder.
pub fn groove() -> Theme {
    compose(
        GROOVE_NAME,
        vec![pars(0.12, 0.45, 0.16, false)],
        vec![
            pix(PixMode::Chase, 0.5, 0.0),
            derbies(true),
            scanners(Movement::Stepped, 0.62),
        ],
    )
}

/// Club — movement-forward: bold scanner sweeps, strong par pulse, colour bed +
/// comet, white blinder reserved for drops.
pub fn club() -> Theme {
    compose(
        CLUB_NAME,
        vec![pars(0.18, 0.60, 0.22, false)],
        vec![
            pix(PixMode::Bed { comet: true }, 0.38, 0.5),
            derbies(true),
            scanners(Movement::Sweep, 0.82),
        ],
    )
}

/// Strobe — the peak: hard white beat-punch pars, white eighth-note pix strobe,
/// fast wide white sweeps. Auto only enters this on real peaks.
pub fn strobe() -> Theme {
    compose(
        STROBE_NAME,
        vec![pars(0.0, 0.0, 0.0, true)],
        vec![
            pix(PixMode::Strobe, 0.3, 0.9),
            derbies(true),
            scanners(Movement::Sweep, 0.95),
        ],
    )
}

// ===========================================================================
// Generative engine (Auto mode): Director + ShowParams + Gen* effects.
//
// Built only on the RELIABLE signals — the beat grid (incl. the corrected,
// even `beat_clock` and best-effort `beat_in_bar` downbeat), `intensity`, and
// `four_on_floor`. Key design choices:
//   * `intensity` only picks a held energy **tier** (the look) — it does NOT
//     modulate frame-to-frame brightness (that read flickery/muddy). Brightness
//     is the bold per-tier level plus a clean per-beat pulse.
//   * Gestures are **bar-aware** (chases reset on the downbeat) and step on the
//     even corrected beat clock, so they don't drift.
//   * Scanners/derbies are varied per phrase (movement pattern, gobo, colour)
//     with sharp beat-snapped moves — not just slow drifting.
//   * The white strobe is reserved for genuine peaks (intense AND grooving).
// ===========================================================================

/// Beats per phrase — one colour + gesture set is held this long, then varies.
const GEN_PHRASE_BEATS: f64 = 16.0;

/// Bold, held brightness per energy tier (intensity picks the tier, NOT the
/// frame-to-frame brightness — that's what made it flickery/muddy).
const TIER_LEVEL: [f32; 4] = [0.45, 0.70, 0.90, 1.0];

/// Scanner gobo / colour-wheel / derby-colour choices the Director draws from.
const GOBOS: [f32; 4] = [SC_GOBO_1, SC_GOBO_2, SC_GOBO_3, SC_GOBO_4];
const SCAN_COLORS: [f32; 5] = [SC_COLOR_RED, SC_COLOR_BLUE, SC_COLOR_GREEN, SC_COLOR_PINK, SC_COLOR_YELLOW];

/// Energy-tier labels (index matches `ShowParams::tier` and tier-lock 0..3).
pub const TIER_NAMES: &[&str] = &["Calm", "Groove", "Club", "Peak"];

/// Palette labels (index matches `ShowParams::palette_idx` and palette-lock 0..3).
pub const PALETTE_NAMES: &[&str] = &["Neon", "Warm", "Cool", "Acid"];

/// Show parameters the Director hands to the generative effects each tick.
#[derive(Clone, Copy, Debug)]
pub struct ShowParams {
    pub tier: u8,        // 0..3 energy tier (held; the "look")
    pub palette_idx: u8, // 0..3 active palette
    pub level: f32,      // bold held brightness for the tier
    pub color: PColor,
    pub color2: PColor,
    pub par_mode: u8,
    pub pix_mode: u8,
    pub movement: u8,    // scanner pattern variant
    pub gobo: f32,       // scanner gobo (per phrase; OPEN at low tiers)
    pub scan_color: f32, // scanner colour-wheel value (per phrase)
    pub derby_color: f32,// QuadPhase colour value (per phrase)
    pub strobe: f32,     // 0..1, non-zero only at peaks (× operator amount)
    /// Coordinated swell at each phrase start (stronger on a tier increase) —
    /// drives wind-ups, phrase transitions and cross-fixture accents. Decays.
    pub lift: f32,
}

impl Default for ShowParams {
    fn default() -> Self {
        Self {
            tier: 0,
            palette_idx: 0,
            level: 0.0,
            color: PColor::full(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            color2: PColor::full(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            par_mode: 0,
            pix_mode: 0,
            movement: 0,
            gobo: SC_GOBO_OPEN,
            scan_color: SC_COLOR_WHITE,
            derby_color: 0.0,
            strobe: 0.0,
            lift: 0.0,
        }
    }
}

/// Deterministic per-phrase hash (SplitMix64) — variety that's stable/repeatable.
fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut x = z;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Stateful interpreter: music → show parameters. Owned by the engine.
pub struct Director {
    intensity: f32,
    peak: f32,
    phrase: i64,
    tier: u8,        // committed auto tier (phrase-quantised)
    prev_tier: u8,
    lift: f32,
    color: PColor,
    prev_color: PColor,
    color2: PColor,
    par_mode: u8,
    pix_mode: u8,
    movement: u8,
    gobo: f32,
    scan_color: f32,
    derby_color: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self::new()
    }
}

impl Director {
    pub fn new() -> Self {
        Self {
            intensity: 0.0,
            peak: 0.0,
            phrase: i64::MIN,
            tier: 0,
            prev_tier: 0,
            lift: 0.0,
            color: PALETTES[0].colors[0],
            prev_color: PALETTES[0].colors[0],
            color2: PALETTES[0].colors[1],
            par_mode: 0,
            pix_mode: 0,
            movement: 1,
            gobo: SC_GOBO_OPEN,
            scan_color: SC_COLOR_WHITE,
            derby_color: 0.0,
        }
    }

    pub fn update(&mut self, a: &AudioFeatures, ctl: &crate::state::ReactiveControls, time: f64) -> ShowParams {
        self.intensity += 0.08 * ((a.intensity * ctl.sensitivity).clamp(0.0, 1.0) - self.intensity);

        let beats = if a.bpm > 0.0 { a.beat_clock } else { time * 2.0 };
        let phrase = (beats / GEN_PHRASE_BEATS).floor() as i64;
        if phrase != self.phrase {
            // New phrase: commit the energy tier (so the *look* only changes on
            // a musical boundary, never flickering) and roll fresh variety.
            self.phrase = phrase;
            self.prev_tier = self.tier;
            self.tier = tier_for(self.intensity);
            self.prev_color = self.color;
            let h = splitmix(phrase as u64);
            // Palette: operator lock wins, else slow rotation.
            let pal_idx = if ctl.palette_lock >= 0 {
                (ctl.palette_lock as usize) % PALETTES.len()
            } else {
                (beats / PALETTE_ROTATE_BEATS) as usize % PALETTES.len()
            };
            let pal = &PALETTES[pal_idx];
            let n = pal.colors.len();
            // Colour harmony: pick a primary, and a *distinct* partner (never the
            // same swatch) for depth/contrast.
            let ci = (h % n as u64) as usize;
            let cj = (ci + 1 + ((h >> 8) % (n as u64 - 1)) as usize) % n;
            self.color = pal.colors[ci];
            self.color2 = pal.colors[cj];
            self.par_mode = ((h >> 16) % 4) as u8;
            self.pix_mode = ((h >> 20) % 4) as u8;
            self.movement = ((h >> 24) % 5) as u8;
            // Gobos only at higher tiers (clean open beams when calm).
            self.gobo = if self.tier >= 2 { GOBOS[((h >> 32) % 4) as usize] } else { SC_GOBO_OPEN };
            self.scan_color = SCAN_COLORS[((h >> 36) % 5) as usize];
            self.derby_color = ((h >> 40) % 100) as f32 / 100.0;
            // Coordinated lift at the phrase start — bigger on a tier increase.
            self.lift = if self.tier > self.prev_tier { 1.0 } else { 0.45 };
        }
        // Effective tier: operator lock overrides the auto tier instantly.
        let tier = if ctl.tier_lock >= 0 {
            (ctl.tier_lock as u8).min(3)
        } else {
            self.tier
        };
        let palette_idx = if ctl.palette_lock >= 0 {
            (ctl.palette_lock as usize) % PALETTES.len()
        } else {
            (beats / PALETTE_ROTATE_BEATS) as usize % PALETTES.len()
        };

        let into = ((beats / GEN_PHRASE_BEATS).fract() * GEN_PHRASE_BEATS) as f32;
        let color = self.prev_color.lerp(self.color, smoothstep(into.min(1.0)));

        // Peak = intense AND grooving → reserves strobes for drops/choruses.
        let peak_t = self.intensity * (0.30 + 0.70 * a.four_on_floor);
        self.peak += 0.05 * (peak_t - self.peak);
        let strobe = (smoothstep((self.peak - 0.42) / 0.15) * ctl.strobe).clamp(0.0, 1.0);

        self.lift *= 0.96; // decays over ~1 bar

        ShowParams {
            tier,
            palette_idx: palette_idx as u8,
            level: TIER_LEVEL[tier as usize],
            color,
            color2: self.color2,
            par_mode: self.par_mode,
            pix_mode: self.pix_mode,
            movement: self.movement,
            gobo: self.gobo,
            scan_color: self.scan_color,
            derby_color: self.derby_color,
            strobe,
            lift: self.lift,
        }
    }
}

/// Map smoothed intensity to an energy tier (0 calm .. 3 peak).
fn tier_for(intensity: f32) -> u8 {
    if intensity < 0.22 {
        0
    } else if intensity < 0.45 {
        1
    } else if intensity < 0.70 {
        2
    } else {
        3
    }
}

/// Current beat-of-bar (0 = downbeat), using detected downbeat when confident,
/// else free-running the corrected beat grid. Drives bar-aware gestures.
fn bar_pos(ctx: &TickContext) -> i64 {
    let bib = ctx.audio.beat_in_bar;
    if bib >= 0 {
        bib as i64
    } else {
        (beats(ctx).floor() as i64).rem_euclid(4)
    }
}

/// True when there's effectively no music — drives a graceful ambient idle
/// instead of freezing on the last frame.
fn gen_idle(ctx: &TickContext) -> bool {
    !locked(&ctx.audio) && ctx.audio.energy < 0.04
}

/// Locked beat-subdivision strobe gate (0/1), no operator-amount applied.
fn locked_eighth(ctx: &TickContext, duty: f32) -> f32 {
    let a = &ctx.audio;
    if !locked(a) {
        return 0.0;
    }
    let ph = ((a.beat_phase as f64 * 2.0).fract()) as f32;
    if ph < duty { 1.0 } else { 0.0 }
}

/// RootPars: bold held colour wash. Brightness is the tier level (held) plus a
/// clean per-beat pulse — NOT modulated by frame-to-frame intensity. The chase
/// is bar-aware (resets on the downbeat). White punch on the drop downbeat.
pub struct GenPars {
    pub count: usize,
}

impl Effect for GenPars {
    fn channel_count(&self) -> usize {
        8
    }
    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let s = &ctx.show;
        let a = &ctx.audio;
        let slot = ctx.slot.min(self.count.max(1) - 1) as i64;

        // Idle: slow deep breathing wash (no music to react to).
        if gen_idle(ctx) {
            let dim = (0.10 + 0.10 * breathe(ctx.time, 7.0)).clamp(0.0, 1.0);
            let c = if slot % 2 == 1 { s.color2 } else { s.color };
            return vec![dim, 0.0, c.r * dim, c.g * dim, c.b * dim, 0.0, 0.0, c.uv * dim];
        }

        let bp = bar_pos(ctx);
        let pulse = beat_accent(ctx, 0.24);

        // Pattern: chases only engage from tier 1 up (calm = unison wash).
        let lit = if s.tier == 0 {
            1.0
        } else {
            match s.par_mode {
                1 => if bp == slot { 1.0 } else { 0.18 },                    // bar-aware 4-chase
                2 => if (bp + slot).rem_euclid(2) == 0 { 1.0 } else { 0.25 }, // alternate halves
                3 => if bp == slot { 1.0 } else if (bp - 1).rem_euclid(4) == slot { 0.45 } else { 0.15 }, // trailing chase
                _ => 1.0,                                                     // unison wash
            }
        };
        // Bold held level + clean beat pulse + a coordinated phrase-start swell.
        let mut dim = (s.level * lit + 0.30 * pulse + 0.30 * s.lift).clamp(0.0, 1.0);

        let c = if slot % 2 == 1 { s.color2 } else { s.color };
        let mut w = c.w;
        // Bold white punch on the drop's downbeat.
        if s.strobe > 0.3 && bp == 0 && a.beat_phase < 0.12 {
            w = 1.0;
            dim = 1.0;
        }
        vec![
            dim,
            0.0,
            (c.r * dim).min(1.0),
            (c.g * dim).min(1.0),
            (c.b * dim).min(1.0),
            w.min(1.0),
            (c.a * dim).min(1.0),
            (c.uv * dim).min(1.0),
        ]
    }
}

/// PixStrobe: bold colour bed with a per-phrase gesture; white blinder cells
/// only at peaks (beat-locked). Brightness is the held tier level.
pub struct GenPix;

impl Effect for GenPix {
    fn channel_count(&self) -> usize {
        32
    }
    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let s = &ctx.show;
        let beats = beats(ctx);
        let mut v = vec![0.0f32; 32];
        if gen_idle(ctx) {
            // Idle: a very dim slow colour bed.
            let k = 0.06 + 0.06 * breathe(ctx.time, 7.0);
            for seg in 0..8 {
                let base = 8 + seg * 3;
                v[base] = s.color.r * k;
                v[base + 1] = s.color.g * k;
                v[base + 2] = s.color.b * k;
            }
            return v;
        }
        let bed = (s.level + 0.25 * s.lift).min(1.0);
        let accent = beat_accent(ctx, 0.30);
        for seg in 0..8 {
            let k = match s.pix_mode {
                1 => {
                    // beat block-chase (two cells), even on the corrected clock
                    let pos = (beats.floor() as i64).rem_euclid(8);
                    if seg as i64 == pos || seg as i64 == (pos + 1).rem_euclid(8) { bed } else { bed * 0.10 }
                }
                2 => {
                    // centre-out pulse on the beat
                    let d = ((seg as f32) - 3.5).abs() / 3.5;
                    (bed * (1.0 - d) * (0.55 + 0.45 * accent)).max(bed * 0.08)
                }
                3 => {
                    // bar halves alternate
                    let half = if seg < 4 { 0 } else { 1 };
                    if (bar_pos(ctx)).rem_euclid(2) == half { bed } else { bed * 0.12 }
                }
                _ => bed * (0.7 + 0.3 * accent), // solid bed with a soft pulse
            };
            let col = if seg % 2 == 1 { s.color2 } else { s.color };
            let base = 8 + seg * 3;
            v[base] = (col.r * k).min(1.0);
            v[base + 1] = (col.g * k).min(1.0);
            v[base + 2] = (col.b * k).min(1.0);
        }
        if s.strobe > 0.05 {
            let st = (locked_eighth(ctx, 0.18) * s.strobe).min(1.0);
            for cell in v.iter_mut().take(8) {
                *cell = st;
            }
        }
        v
    }
}

/// QuadPhase derby: from tier 1 up. Colour held per phrase, rotation steps up
/// by tier with a spin burst on the downbeat; internal strobe on peaks.
pub struct GenDerby;

impl Effect for GenDerby {
    fn channel_count(&self) -> usize {
        4
    }
    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let s = &ctx.show;
        if s.tier < 1 {
            return vec![0.0, 0.0, 0.0, 0.0];
        }
        let base_rot = [0.0, 0.30, 0.55, 0.80][s.tier as usize];
        // Spin burst on the downbeat and a swell on the phrase lift.
        let burst = if bar_pos(ctx) == 0 { 0.18 * beat_accent(ctx, 0.18) } else { 0.0 };
        let rotation = (base_rot + burst + 0.15 * s.lift).clamp(0.0, 1.0);
        let strobe = if s.strobe > 0.4 { 0.6 } else { 0.0 };
        vec![s.derby_color, rotation, strobe, 1.0]
    }
}

/// SC-X50 scanner: varied, bar-aware movement (sharp beat-snapped presets,
/// sweeps, crosses, circles), gobo + colour change per phrase, prism breakout
/// at peaks. Shutter stays open; brightness is the held tier level.
pub struct GenScanner {
    pub reverse: bool,
}

impl Effect for GenScanner {
    fn channel_count(&self) -> usize {
        11
    }
    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let s = &ctx.show;
        let beats = beats(ctx);
        let tau = std::f64::consts::TAU;
        let bp = bar_pos(ctx);

        // Idle: slow drift, dim, beam open, palette colour.
        if gen_idle(ctx) {
            let pan = 0.5 + 0.12 * (tau * ctx.time / 20.0).sin() as f32;
            let pan = if self.reverse { 1.0 - pan } else { pan };
            return scanner_frame(pan, 0.70, s.scan_color, SC_SHUTTER_OPEN, 0.22, SC_GOBO_OPEN, SC_GOBOROT_NONE, SC_PRISM_OFF, SC_FOCUS_MID);
        }

        let (mut pan, mut tilt) = match s.movement {
            0 => {
                // Bar-stepped presets — snap to a new position each beat (the
                // mirror moves, the beam stays on). Sharp and rhythmic.
                match bp {
                    0 => (0.50, 0.55),
                    1 => (0.22, 0.62),
                    2 => (0.78, 0.62),
                    _ => (0.50, 0.80),
                }
            }
            1 => {
                // Bold wide sweep over 8 beats.
                (0.5 + 0.34 * (tau * beats / 8.0).sin() as f32, 0.62 + 0.18 * (tau * beats / 12.0).sin() as f32)
            }
            2 => {
                // Fast cross (counter-rotating via reverse).
                (0.5 + 0.30 * (tau * beats / 4.0).sin() as f32, 0.66)
            }
            3 => {
                // Circle: pan/tilt 90° out of phase over 8 beats.
                (0.5 + 0.26 * (tau * beats / 8.0).cos() as f32, 0.66 + 0.16 * (tau * beats / 8.0).sin() as f32)
            }
            _ => {
                // Tilt-wave, pan held centre.
                (0.5, 0.55 + 0.30 * (0.5 + 0.5 * (tau * beats / 8.0).sin() as f32))
            }
        };
        // Phrase lift: a quick tilt-up swell at each phrase start.
        tilt += 0.12 * s.lift;
        if self.reverse {
            pan = 1.0 - pan;
        }
        pan = pan.clamp(0.0, 1.0);
        tilt = tilt.clamp(0.45, 0.90);

        let peak = s.strobe > 0.3;
        let color = if peak { SC_COLOR_WHITE } else { s.scan_color };
        let gobo = if peak { SC_GOBO_1 } else { s.gobo };
        let prism = if peak { SC_PRISM_ROT } else { SC_PRISM_OFF };
        // Bold held dimmer (tier level); scanners only really come up from tier 1.
        let dim = ((if s.tier == 0 { 0.30 } else { s.level }) + 0.20 * s.lift).clamp(0.0, 1.0);
        scanner_frame(pan, tilt, color, SC_SHUTTER_OPEN, dim, gobo, SC_GOBOROT_NONE, prism, SC_FOCUS_MID)
    }
}

/// The generative show — what Auto renders. Reads `ctx.show` (Director output).
pub fn generative() -> Theme {
    Theme::new(
        "Generativ",
        Transition::Crossfade { duration_ms: 600 },
        vec![
            Binding::group(
                vec![&fixtures::ROOTPAR_1, &fixtures::ROOTPAR_2, &fixtures::ROOTPAR_3, &fixtures::ROOTPAR_4],
                GenPars { count: 4 },
            ),
            Binding::single(&fixtures::PIXSTROBE_1, GenPix),
            Binding::single(&fixtures::PIXSTROBE_2, GenPix),
            Binding::single(&fixtures::QUADPHASE_1, GenDerby),
            Binding::single(&fixtures::QUADPHASE_2, GenDerby),
            Binding::single(&fixtures::SCANNER_1, GenScanner { reverse: false }),
            Binding::single(&fixtures::SCANNER_2, GenScanner { reverse: true }),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::NUM_BANDS;
    use crate::state::ReactiveControls;
    use std::sync::Arc;

    fn ctx_c(audio: AudioFeatures, slot: usize, time: f64, controls: ReactiveControls) -> TickContext {
        TickContext {
            tick: 0,
            time,
            show_time: time,
            wall_clock: chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            dt: 0.025,
            slot,
            audio: Arc::new(audio),
            controls,
            show: ShowParams::default(),
        }
    }
    fn ctx(audio: AudioFeatures, slot: usize, time: f64) -> TickContext {
        ctx_c(audio, slot, time, ReactiveControls::default())
    }

    fn loud_locked() -> AudioFeatures {
        AudioFeatures {
            energy: 0.85,
            intensity: 0.85,
            four_on_floor: 0.7,
            beat_in_bar: 0,
            bands: [0.7; NUM_BANDS],
            bands_fast: [0.7; NUM_BANDS],
            onset: 0.4,
            beat_now: true,
            beat_phase: 0.0,
            beat_clock: 16.0,
            bpm: 128.0,
            beat_confidence: 0.9,
            t_capture: 0.0,
        }
    }

    /// Every effect (every look's effects) emits valid DMX for silence and full
    /// scale, locked and unlocked, across slots, beat phases, and control values.
    #[test]
    fn all_effects_in_range() {
        let effects: Vec<(&str, Box<dyn Effect>)> = vec![
            ("pars-wash", Box::new(ReactivePars { floor: 0.18, gain: 0.6, pulse: 0.22, white_strobe: false })),
            ("pars-white", Box::new(ReactivePars { floor: 0.0, gain: 0.0, pulse: 0.0, white_strobe: true })),
            ("pix-bed", Box::new(ReactivePix { mode: PixMode::Bed { comet: true }, bed: 0.38, blinder: 0.5 })),
            ("pix-chase", Box::new(ReactivePix { mode: PixMode::Chase, bed: 0.5, blinder: 0.0 })),
            ("pix-strobe", Box::new(ReactivePix { mode: PixMode::Strobe, bed: 0.3, blinder: 0.9 })),
            ("derby", Box::new(ReactiveDerby { active: true })),
            ("scan-sweep", Box::new(ReactiveScanner { reverse: true, movement: Movement::Sweep, bright: 0.9 })),
            ("scan-step", Box::new(ReactiveScanner { reverse: false, movement: Movement::Stepped, bright: 0.6 })),
        ];
        let mut states = vec![AudioFeatures::silent()];
        for &ph in &[0.0, 0.2, 0.5, 0.9] {
            states.push(AudioFeatures { beat_phase: ph, ..loud_locked() });
        }
        let controls = [
            ReactiveControls::default(),
            ReactiveControls { master: 0.5, sensitivity: 1.8, strobe: 0.0, tier_lock: -1, palette_lock: -1 },
            ReactiveControls { master: 1.0, sensitivity: 0.5, strobe: 1.0, tier_lock: 2, palette_lock: 1 },
        ];
        for feat in &states {
            for ctl in &controls {
                for (name, fx) in &effects {
                    for slot in 0..4 {
                        for &t in &[0.0, 0.3, 1.1, 2.7, 5.5] {
                            let out = fx.tick(&ctx_c(feat.clone(), slot, t, *ctl));
                            assert_eq!(out.len(), fx.channel_count(), "{name} count");
                            for (i, val) in out.iter().enumerate() {
                                assert!(val.is_finite() && (0.0..=1.0).contains(val), "{name} ch{i}={val}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// Confidence gating: no beat pulse when unsure; pulse on a locked downbeat.
    #[test]
    fn beat_pulse_is_confidence_gated() {
        let pars = ReactivePars { floor: 0.1, gain: 0.0, pulse: 0.4, white_strobe: false };
        let unlocked = AudioFeatures { beat_confidence: 0.0, onset: 0.0, ..AudioFeatures::silent() };
        assert!(pars.tick(&ctx(unlocked, 0, 0.0))[0] < 0.12);
        let locked_dn = AudioFeatures { beat_confidence: 0.9, bpm: 128.0, beat_phase: 0.0, ..AudioFeatures::silent() };
        assert!(pars.tick(&ctx(locked_dn, 0, 0.0))[0] > 0.3);
    }

    /// The strobe never free-runs: with the beat unlocked, the white cells stay
    /// dark even at a full drop (no "random" flicker). Locked, they flash.
    #[test]
    fn strobe_requires_lock() {
        let p = ReactivePix { mode: PixMode::Strobe, bed: 0.3, blinder: 0.9 };
        let unlocked_drop = AudioFeatures { energy: 0.9, intensity: 0.9, four_on_floor: 0.9, beat_confidence: 0.0, bpm: 0.0, ..AudioFeatures::silent() };
        let any_unlocked = (0..50).any(|k| p.tick(&ctx(AudioFeatures { beat_phase: (k as f32 * 0.02).fract(), ..unlocked_drop.clone() }, 0, k as f64 * 0.02))[..8].iter().any(|w| *w > 0.05));
        assert!(!any_unlocked, "strobe must not fire when the beat is unlocked");

        let locked_drop = AudioFeatures { energy: 0.9, intensity: 0.9, four_on_floor: 0.9, beat_confidence: 0.9, bpm: 128.0, ..AudioFeatures::silent() };
        let any_locked = (0..50).any(|k| p.tick(&ctx(AudioFeatures { beat_phase: (k as f32 * 0.02).fract(), ..locked_drop.clone() }, 0, k as f64 * 0.02))[..8].iter().any(|w| *w > 0.4));
        assert!(any_locked, "strobe must flash when locked at a peak");
    }

    /// Strobe amount control at 0 disables the strobe entirely.
    #[test]
    fn strobe_control_disables() {
        let p = ReactivePix { mode: PixMode::Strobe, bed: 0.3, blinder: 0.9 };
        let off = ReactiveControls { strobe: 0.0, ..ReactiveControls::default() };
        let locked_drop = AudioFeatures { energy: 0.9, intensity: 0.9, four_on_floor: 0.9, beat_confidence: 0.9, bpm: 128.0, ..AudioFeatures::silent() };
        let any = (0..50).any(|k| p.tick(&ctx_c(AudioFeatures { beat_phase: (k as f32 * 0.02).fract(), ..locked_drop.clone() }, 0, k as f64 * 0.02, off))[..8].iter().any(|w| *w > 0.05));
        assert!(!any, "strobe=0 must silence the white cells");
    }

    fn dfeat(intensity: f32, fof: f32, beat_clock: f64) -> AudioFeatures {
        AudioFeatures {
            intensity,
            four_on_floor: fof,
            bpm: 128.0,
            beat_confidence: 0.9,
            beat_clock,
            ..AudioFeatures::silent()
        }
    }

    /// The generative Director must reserve the strobe for peaks (intense AND
    /// grooving) and never strobe in calm passages.
    #[test]
    fn director_reserves_strobe_for_peaks() {
        let mut d = Director::new();
        let mut s = ShowParams::default();
        for k in 0..300 {
            s = d.update(&dfeat(0.2, 0.0, k as f64 * 0.1), &ReactiveControls::default(), k as f64 * 0.025);
        }
        assert!(s.strobe < 0.05, "calm must not strobe: {}", s.strobe);
        for k in 0..300 {
            s = d.update(&dfeat(0.95, 0.75, 30.0 + k as f64 * 0.1), &ReactiveControls::default(), 8.0 + k as f64 * 0.025);
        }
        assert!(s.strobe > 0.3, "a real peak should strobe: {}", s.strobe);

        // Operator strobe amount 0 disables it even at a peak.
        let mut d2 = Director::new();
        let mut s2 = ShowParams::default();
        for k in 0..300 {
            s2 = d2.update(&dfeat(0.95, 0.75, k as f64 * 0.1), &ReactiveControls { strobe: 0.0, ..ReactiveControls::default() }, k as f64 * 0.025);
        }
        assert!(s2.strobe < 0.01, "strobe=0 must disable: {}", s2.strobe);
    }

    /// Successive phrases must vary their gesture set (colour/pattern/movement).
    #[test]
    fn director_varies_per_phrase() {
        let mut d = Director::new();
        let mut sets = std::collections::HashSet::new();
        for p in 0..12i64 {
            let s = d.update(&dfeat(0.6, 0.5, p as f64 * 16.0 + 1.0), &ReactiveControls::default(), p as f64);
            sets.insert((s.par_mode, s.pix_mode, s.movement));
        }
        assert!(sets.len() >= 4, "phrases should vary, got {} distinct sets", sets.len());
    }

    /// Operator overrides: a tier lock forces the look regardless of (quiet)
    /// intensity, and a palette lock forces the colour family.
    #[test]
    fn director_tier_and_palette_lock() {
        let mut d = Director::new();
        let ctl = ReactiveControls { tier_lock: 2, palette_lock: 1, ..ReactiveControls::default() };
        let mut s = ShowParams::default();
        for k in 0..40i64 {
            s = d.update(&dfeat(0.05, 0.0, (k * 16) as f64), &ctl, k as f64);
        }
        assert_eq!(s.tier, 2, "tier_lock must force the tier even when quiet");
        let warm = PALETTES[1].colors;
        let in_warm = warm.iter().any(|c| {
            (c.r - s.color.r).abs() < 1e-3 && (c.g - s.color.g).abs() < 1e-3 && (c.b - s.color.b).abs() < 1e-3
        });
        assert!(in_warm, "palette_lock must force the Warm palette, got {:?}", s.color);
    }

    fn ctx_show(audio: AudioFeatures, slot: usize, time: f64, show: ShowParams) -> TickContext {
        let mut c = ctx(audio, slot, time);
        c.show = show;
        c
    }

    /// Every generative effect emits valid in-range DMX across show params,
    /// beat phases and slots.
    #[test]
    fn gen_effects_in_range() {
        let effects: Vec<(&str, Box<dyn Effect>)> = vec![
            ("genpars", Box::new(GenPars { count: 4 })),
            ("genpix", Box::new(GenPix)),
            ("genderby", Box::new(GenDerby)),
            ("genscan", Box::new(GenScanner { reverse: true })),
        ];
        let shows = [
            ShowParams::default(),
        ShowParams { tier: 3, palette_idx: 0, level: 1.0, color: PALETTES[0].colors[0], color2: PALETTES[1].colors[2], par_mode: 1, pix_mode: 2, movement: 3, gobo: SC_GOBO_2, scan_color: SC_COLOR_BLUE, derby_color: 0.4, strobe: 0.8, lift: 1.0 },
            ShowParams { tier: 1, palette_idx: 2, level: 0.7, color: PALETTES[2].colors[1], color2: PALETTES[3].colors[0], par_mode: 3, pix_mode: 1, movement: 0, gobo: SC_GOBO_OPEN, scan_color: SC_COLOR_RED, derby_color: 0.1, strobe: 0.0, lift: 0.0 },
        ];
        for sh in &shows {
            for feat in [AudioFeatures::silent(), loud_locked()] {
                for (name, fx) in &effects {
                    for slot in 0..4 {
                        for &t in &[0.0, 0.3, 1.1, 2.7] {
                            let out = fx.tick(&ctx_show(feat.clone(), slot, t, *sh));
                            assert_eq!(out.len(), fx.channel_count(), "{name} count");
                            for (i, v) in out.iter().enumerate() {
                                assert!(v.is_finite() && (0.0..=1.0).contains(v), "{name} ch{i}={v}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn looks_build() {
        for t in [off(), dark(), groove(), club(), strobe()] {
            if t.name != OFF_NAME {
                assert!(t.bindings.len() >= 5, "{} too few bindings", t.name);
            }
        }
    }
}
