//! # FX Theme 4 – "Off With Their Heads!"
//!
//! High-energy red-and-white military march. Every fixture on stage fires.
//!
//! ## Design intent
//! The Queen of Hearts calls her guards to order. Hard 4/4 at 120 BPM.
//! Red is the base colour everywhere; white hits like an axe on the downbeat.
//!
//! - **RootPars**: Deep red wash with a marching 4/4 chase – one par lit at a
//!   time, stepping every beat. On beat 1 (downbeat) all four flash white
//!   simultaneously for one frame.
//!
//! - **PixStrobes**: CW blinder strip pulses white on beats 1 & 3; RGB pixels
//!   hold deep red; on the downbeat the whole strip strobes for 0.15 s.
//!
//! - **QuadPhases**: Medium-fast rotation, red color wheel, shutter snaps on
//!   beats 2 & 4 (backbeat) so the beams accent between the blinder flashes.
//!
//! - **Scanners**: Beat-synced hard position skips between three presets
//!   (cross-center, splayed wide, straight up). Color: red. Shutter stabs on
//!   every downbeat with a brief white flash (color momentarily to white for
//!   the stab frame). Every 4th bar the two scanners sweep through a rapid
//!   cross-beam X across stage center.

use crate::effect::{Effect, TickContext};
use crate::fixtures;
use crate::theme::{Binding, Theme, Transition};
use crate::themes::{
    SC_COLOR_RED, SC_FOCUS_MID, SC_FUNC_NONE, SC_GOBO_OPEN,
    SC_GOBOROT_NONE, SC_PROG_DMX, SC_PRISM_OFF, SC_SHUTTER_CLOSED, SC_SHUTTER_OPEN,
};

pub const NAME: &str = "Off With Their Heads!";

// ─── Shared tempo parameters ─────────────────────────────────────────────────

const BPM: f64 = 125.0;
const BEAT_S: f64 = 60.0 / BPM;       // 0.5 s per beat
const BAR_S: f64 = BEAT_S * 4.0;      // 2.0 s per bar

// ─── PixStrobe: red field + white blinder on beats 1 & 3 ─────────────────────

const PIX_RED_INTENSITY: f32 = 0.35;
/// CW blinder brightness on the beat flash.
const PIX_BLINDER_PEAK: f32 = 1.0;
/// How long (s) the blinder flash lasts.
const PIX_FLASH_DURATION_S: f64 = 0.07;
/// How long the downbeat strobe lasts.
const PIX_STROBE_DURATION_S: f64 = 0.15;

struct HeadsPixStrobe;

impl Effect for HeadsPixStrobe {
    fn channel_count(&self) -> usize {
        32
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut out = vec![0.0f32; 32];

        let t_in_bar  = ctx.time.rem_euclid(BAR_S);
        let t_in_beat = ctx.time.rem_euclid(BEAT_S);
        let beat_in_bar = (t_in_bar / BEAT_S).floor() as usize; // 0-3

        // ── CW blinder strip (channels 4–7): flash on beats 0 & 2 ────────
        let is_flash_beat = beat_in_bar == 0 || beat_in_bar == 2;
        let cw = if is_flash_beat && t_in_beat < PIX_FLASH_DURATION_S {
            PIX_BLINDER_PEAK
        } else {
            0.0
        };
        // On beat 0 (downbeat) keep strobing for PIX_STROBE_DURATION_S.
        let cw = if beat_in_bar == 0 && t_in_beat < PIX_STROBE_DURATION_S {
            let phase = (ctx.time * 20.0).fract(); // ≈ 20 Hz strobe during window
            if phase < 0.5 { PIX_BLINDER_PEAK } else { 0.0 }
        } else {
            cw
        };
        for i in 4..8 {
            out[i] = cw;
        }

        // ── RGB pixels (channels 8–31): static deep red ───────────────────
        for pixel in 0..8usize {
            let ch = 8 + pixel * 3;
            out[ch]     = PIX_RED_INTENSITY; // R
            out[ch + 1] = 0.0;              // G
            out[ch + 2] = 0.0;              // B
        }

        out
    }
}

// ─── QuadPhase: red, medium-fast, backbeat shutter ───────────────────────────

/// Shutter open at full value.
const QP_SHUTTER_OPEN: f32  = 1.0;
const QP_SHUTTER_CLOSED: f32 = 0.0;
const QP_COLOR_RED: f32 = 0.667; // mid-wheel red; adjust to fixture color chart
const QP_ROTATION_FAST: f32 = 0.70;
/// Duration (s) for which the shutter is open after the backbeat.
const QP_OPEN_DURATION_S: f64 = 0.18;

struct HeadsQuadPhase;

impl Effect for HeadsQuadPhase {
    fn channel_count(&self) -> usize { 4 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t_in_bar  = ctx.time.rem_euclid(BAR_S);
        let t_in_beat = ctx.time.rem_euclid(BEAT_S);
        let beat_in_bar = (t_in_bar / BEAT_S).floor() as usize;

        // Backbeats = beat 1 (snare) and beat 3.
        let is_backbeat = beat_in_bar == 1 || beat_in_bar == 3;
        let shutter = if is_backbeat && t_in_beat < QP_OPEN_DURATION_S {
            QP_SHUTTER_OPEN
        } else {
            QP_SHUTTER_CLOSED
        };

        vec![QP_COLOR_RED, QP_ROTATION_FAST, 0.0, shutter]
    }
}

// ─── RootPar: 4/4 marching chase + white downbeat flash ──────────────────────

const PAR_DIMMER_RED: f32  = 0.55;
const PAR_DIMMER_WHITE: f32 = 1.0;
/// How long (s) the all-white downbeat flash lasts.
const PAR_WHITE_DURATION_S: f64 = 0.06;

/// [Dimmer, Strobe, R, G, B, W, Amber, UV]
const PAR_RED:   [f32; 8] = [PAR_DIMMER_RED,   0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
const PAR_WHITE: [f32; 8] = [PAR_DIMMER_WHITE,  0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
const PAR_OFF:   [f32; 8] = [0.0; 8];

struct HeadsRootPar {
    step: u64,
}

impl Effect for HeadsRootPar {
    fn channel_count(&self) -> usize { 8 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t_in_bar  = ctx.time.rem_euclid(BAR_S);
        let t_in_beat = ctx.time.rem_euclid(BEAT_S);
        let beat_in_bar = (t_in_bar / BEAT_S).floor() as usize;

        // All-white flash on downbeat (beat 0).
        if beat_in_bar == 0 && t_in_beat < PAR_WHITE_DURATION_S {
            return PAR_WHITE.to_vec();
        }

        // Marching chase: only the par matching the current beat lights up.
        let beat_global = (ctx.time / BEAT_S).floor() as u64;
        let active_step = beat_global % 4;
        if active_step == self.step {
            PAR_RED.to_vec()
        } else {
            PAR_OFF.to_vec()
        }
    }
}

// ─── Scanner: beat skips + cross-beam every 4th bar ──────────────────────────

// Three pan presets: center (aimed at stage-center crossing), wide-splay, up.
const SCAN_PAN_CENTER_L: f32  = 0.65; // Scanner 1 aimed rightward (toward center)
const SCAN_PAN_CENTER_R: f32  = 0.35; // Scanner 2 aimed leftward (toward center)
const SCAN_PAN_SPLAY_L: f32   = 0.20; // Scanner 1 aimed far left
const SCAN_PAN_SPLAY_R: f32   = 0.80; // Scanner 2 aimed far right
const SCAN_PAN_CENTER_BOTH: f32 = 0.50; // Both aimed center (for X sweep)

const SCAN_TILT_STAGE: f32  = 0.50; // mid-room beam
const SCAN_TILT_UP: f32     = 0.80; // near-ceiling
const SCAN_DIMMER: f32      = 1.0;
/// How long a shutter stab stays open (s).
const SCAN_STAB_S: f64 = 0.1;
/// Cross-beam sweep: how long (s) pan travels from splay to center during X.
const SCAN_CROSS_DURATION_S: f64 = 0.25;

struct HeadsScanner {
    reverse: bool, // true = scanner 2 (mirror image)
}

impl Effect for HeadsScanner {
    fn channel_count(&self) -> usize { 11 }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t_in_bar   = ctx.time.rem_euclid(BAR_S);
        let t_in_beat  = ctx.time.rem_euclid(BEAT_S);
        let beat_in_bar = (t_in_bar / BEAT_S).floor() as usize;
        // Which 4-bar phrase are we in? Every 4th bar triggers the cross-beam.
        let bar_global = (ctx.time / BAR_S).floor() as u64;
        let phrase_beat = ((bar_global % 4) * 4 + beat_in_bar as u64) as usize;

        // Cross-beam X fires on beat 0 of bar 3 in every 4-bar phrase.
        let cross_start_in_phrase = 3 * 4; // = beat 12
        let t_in_phrase = ctx.time.rem_euclid(BAR_S * 4.0);
        let t_cross = t_in_phrase - cross_start_in_phrase as f64 * BEAT_S;
        let in_cross = t_cross >= 0.0 && t_cross < SCAN_CROSS_DURATION_S;

        let (pan, tilt, color) = if in_cross {
            // Sweep from splay inward toward center during the cross window.
            let frac = (t_cross / SCAN_CROSS_DURATION_S) as f32;
            let pan_from = if self.reverse { SCAN_PAN_SPLAY_R } else { SCAN_PAN_SPLAY_L };
            let pan = pan_from + (SCAN_PAN_CENTER_BOTH - pan_from) * frac;
            (pan, SCAN_TILT_STAGE, SC_COLOR_RED)
        } else {
            // Normal beat-step logic – skip to a preset on each beat.
            let (pan, tilt) = match phrase_beat % 4 {
                0 => (
                    if self.reverse { SCAN_PAN_CENTER_R } else { SCAN_PAN_CENTER_L },
                    SCAN_TILT_STAGE,
                ),
                1 => (
                    if self.reverse { SCAN_PAN_SPLAY_R } else { SCAN_PAN_SPLAY_L },
                    SCAN_TILT_STAGE,
                ),
                2 => (SCAN_PAN_CENTER_BOTH, SCAN_TILT_UP),
                _ => (
                    if self.reverse { SCAN_PAN_CENTER_R } else { SCAN_PAN_CENTER_L },
                    SCAN_TILT_STAGE,
                ),
            };
            (pan, tilt, SC_COLOR_RED)
        };

        // Shutter: stab on each downbeat (beat 0 of the bar), closed otherwise
        // so the jump between positions is invisible.
        let shutter = if t_in_beat < SCAN_STAB_S { SC_SHUTTER_OPEN } else { SC_SHUTTER_CLOSED };

        vec![
            pan, tilt, color,
            shutter,
            SCAN_DIMMER,
            SC_GOBO_OPEN,
            SC_GOBOROT_NONE,
            SC_PRISM_OFF,
            SC_FOCUS_MID,
            SC_FUNC_NONE,
            SC_PROG_DMX,
        ]
    }
}

// ─── Theme factory ────────────────────────────────────────────────────────────

pub fn theme() -> Theme {
    Theme::new(
        NAME,
        Transition::Instant,
        vec![
            Binding::single(&fixtures::PIXSTROBE_1, HeadsPixStrobe),
            Binding::single(&fixtures::PIXSTROBE_2, HeadsPixStrobe),
            Binding::single(&fixtures::QUADPHASE_1, HeadsQuadPhase),
            Binding::single(&fixtures::QUADPHASE_2, HeadsQuadPhase),
            Binding::single(&fixtures::ROOTPAR_1, HeadsRootPar { step: 0 }),
            Binding::single(&fixtures::ROOTPAR_2, HeadsRootPar { step: 1 }),
            Binding::single(&fixtures::ROOTPAR_3, HeadsRootPar { step: 2 }),
            Binding::single(&fixtures::ROOTPAR_4, HeadsRootPar { step: 3 }),
            Binding::single(&fixtures::SCANNER_1, HeadsScanner { reverse: false }),
            Binding::single(&fixtures::SCANNER_2, HeadsScanner { reverse: true }),
        ],
    )
}
