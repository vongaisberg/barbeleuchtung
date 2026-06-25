//! Live stage-fixture visualization.
//!
//! The DMX engine publishes its most recent rendered frame (one 512-byte
//! buffer per universe) into a lock-free [`FrameHandle`].  This module reads
//! that frame back through the fixture patch definitions and turns it into
//! display-friendly colours for the in-browser front-view at `/visualization`.
//!
//! The visual *layout* (where each fixture sits in the front view) lives in
//! `static/stage_layout.json` and is read at runtime, so it can be tweaked and
//! reloaded without recompiling.

use std::sync::Arc;

use actix_web::{HttpResponse, Responder};
use arc_swap::ArcSwap;
use serde::Serialize;

use crate::engine::NUM_UNIVERSES;
use crate::fixture::Fixture;
use crate::fixtures;

/// One rendered DMX frame: the engine's universe buffers, copied out each tick.
pub type FrameSnapshot = Vec<[u8; 512]>;

/// Lock-free handle the engine writes and the web layer reads.
pub type FrameHandle = Arc<ArcSwap<FrameSnapshot>>;

/// Create an all-zero frame handle sized for the patched universes.
pub fn new_frame_handle() -> FrameHandle {
    Arc::new(ArcSwap::from_pointee(vec![[0u8; 512]; NUM_UNIVERSES]))
}

// ---------------------------------------------------------------------------
// DMX read-back helpers
// ---------------------------------------------------------------------------

/// Read the coarse (8-bit) value of one logical channel of a fixture from a
/// frame.  16-bit channels return their coarse byte, which is plenty for a
/// display.  Out-of-range reads return 0.
fn chan(frame: &FrameSnapshot, fx: &Fixture, logical_index: usize) -> u8 {
    let Some(ch) = fx.channels.get(logical_index) else {
        return 0;
    };
    let uni = fx.universe as usize;
    let Some(buf) = frame.get(uni) else {
        return 0;
    };
    let idx = (fx.start_address - 1 + ch.dmx_offset) as usize;
    buf.get(idx).copied().unwrap_or(0)
}

fn clamp_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Map a 0..=255 colour-wheel position to an approximate display RGB.
/// Slot 0 is treated as open/white; the rest sweep a simple rainbow so the
/// movement of the wheel is at least visible, even if not colour-accurate.
fn color_wheel(v: u8) -> [u8; 3] {
    if v < 8 {
        return [255, 255, 255];
    }
    let h = (v as f32 / 255.0) * 360.0;
    hsv_to_rgb(h, 1.0, 1.0)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [
        clamp_u8((r + m) * 255.0),
        clamp_u8((g + m) * 255.0),
        clamp_u8((b + m) * 255.0),
    ]
}

// ---------------------------------------------------------------------------
// Per-fixture display state
// ---------------------------------------------------------------------------

/// Display state for a single fixture, serialised to the frontend.
#[derive(Serialize)]
pub struct FixtureView {
    name: &'static str,
    kind: &'static str,
    /// Representative RGB after applying the master dimmer (0..255).
    rgb: [u8; 3],
    /// Master intensity 0..1 (dimmer × shutter), for glow scaling.
    intensity: f32,
    /// Strobe channel value 0..255 (0 = no strobe).
    strobe: u8,
    /// Continuous-rotation speed 0..1 (derbies); 0 otherwise.
    rotation: f32,
    /// Per-pixel RGB colours (pixel bars, ceiling strips); empty otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    segments: Vec<[u8; 3]>,
    /// Warm/cool white cells for the PixStrobe centre strip; empty otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    whites: Vec<[u8; 3]>,
    /// Pan / tilt 0..1 for moving fixtures (scanners); 0 otherwise.
    pan: f32,
    tilt: f32,
}

/// RootPar: [Dim, Strobe, R, G, B, W, Amber, UV].
fn view_rootpar(frame: &FrameSnapshot, fx: &Fixture) -> FixtureView {
    let dim = chan(frame, fx, 0) as f32 / 255.0;
    let strobe = chan(frame, fx, 1);
    let r = chan(frame, fx, 2) as f32;
    let g = chan(frame, fx, 3) as f32;
    let b = chan(frame, fx, 4) as f32;
    let w = chan(frame, fx, 5) as f32;
    let a = chan(frame, fx, 6) as f32;
    let uv = chan(frame, fx, 7) as f32;

    // Mix the secondary emitters into RGB so white/amber/UV are visible.
    let rr = (r + w + a + uv * 0.4) * dim;
    let gg = (g + w + a * 0.6) * dim;
    let bb = (b + w + uv) * dim;

    FixtureView {
        name: fx.name,
        kind: "par",
        rgb: [clamp_u8(rr), clamp_u8(gg), clamp_u8(bb)],
        intensity: dim,
        strobe,
        rotation: 0.0,
        segments: Vec::new(),
        whites: Vec::new(),
        pan: 0.0,
        tilt: 0.0,
    }
}

/// PixStrobe: a bright warm/cool white strobe strip down the middle
/// (logical channels 0-3 = WW 1-4, 4-7 = CW 1-4) flanked by an RGB pixel grid
/// arranged 2 rows × 4 columns (logical channels 8.. = R,G,B per segment).
///
/// We expose the four white *columns* separately from the eight RGB pixels so
/// the front view can draw the real layout: RGB row, white strip, RGB row.
fn view_pixstrobe(frame: &FrameSnapshot, fx: &Fixture) -> FixtureView {
    // Four white columns: each pairs one warm (WW_i) and one cool (CW_i) cell.
    const WARM: [f32; 3] = [255.0, 178.0, 107.0];
    const COOL: [f32; 3] = [205.0, 224.0, 255.0];
    let mut whites = Vec::with_capacity(4);
    for i in 0..4 {
        let ww = chan(frame, fx, i) as f32 / 255.0;
        let cw = chan(frame, fx, 4 + i) as f32 / 255.0;
        whites.push([
            clamp_u8(WARM[0] * ww + COOL[0] * cw),
            clamp_u8(WARM[1] * ww + COOL[1] * cw),
            clamp_u8(WARM[2] * ww + COOL[2] * cw),
        ]);
    }
    // Eight RGB pixels (raw, no white folded in).
    let mut segments = Vec::with_capacity(8);
    for seg in 0..8 {
        let base = 8 + seg * 3;
        segments.push([
            chan(frame, fx, base),
            chan(frame, fx, base + 1),
            chan(frame, fx, base + 2),
        ]);
    }
    // Representative colour / glow = brightest cell across whites + pixels.
    let mut rgb = [0u8; 3];
    let mut best = 0u32;
    for c in segments.iter().chain(whites.iter()) {
        let l = c[0] as u32 + c[1] as u32 + c[2] as u32;
        if l > best {
            best = l;
            rgb = *c;
        }
    }
    let intensity = (rgb[0].max(rgb[1]).max(rgb[2])) as f32 / 255.0;

    FixtureView {
        name: fx.name,
        kind: "pixbar",
        rgb,
        intensity,
        strobe: 0,
        rotation: 0.0,
        segments,
        whites,
        pan: 0.0,
        tilt: 0.0,
    }
}

/// QuadPhase derby: [Color, Rotation, Strobe, Shutter].
fn view_quadphase(frame: &FrameSnapshot, fx: &Fixture) -> FixtureView {
    let color = chan(frame, fx, 0);
    let rotation = chan(frame, fx, 1) as f32 / 255.0;
    let strobe = chan(frame, fx, 2);
    let shutter = chan(frame, fx, 3);
    // Shutter acts as master: open when non-zero.
    let intensity = shutter as f32 / 255.0;
    let base = color_wheel(color);
    let rgb = [
        clamp_u8(base[0] as f32 * intensity),
        clamp_u8(base[1] as f32 * intensity),
        clamp_u8(base[2] as f32 * intensity),
    ];
    FixtureView {
        name: fx.name,
        kind: "derby",
        rgb,
        intensity,
        strobe,
        rotation,
        segments: Vec::new(),
        whites: Vec::new(),
        pan: 0.0,
        tilt: 0.0,
    }
}

/// Scanner: [Pan, Tilt, Color, Shutter, Dimmer, Gobo, ...].
fn view_scanner(frame: &FrameSnapshot, fx: &Fixture) -> FixtureView {
    let pan = chan(frame, fx, 0) as f32 / 255.0;
    let tilt = chan(frame, fx, 1) as f32 / 255.0;
    let color = chan(frame, fx, 2);
    let shutter = chan(frame, fx, 3);
    let dim = chan(frame, fx, 4) as f32 / 255.0;

    // Shutter: 0-3 closed, 4-7 open, 8-215 strobe, 216+ open.
    let (open, strobe) = match shutter {
        0..=3 => (0.0, 0),
        8..=215 => (1.0, shutter),
        _ => (1.0, 0),
    };
    let intensity = dim * open;
    let base = color_wheel(color);
    let rgb = [
        clamp_u8(base[0] as f32 * intensity),
        clamp_u8(base[1] as f32 * intensity),
        clamp_u8(base[2] as f32 * intensity),
    ];
    FixtureView {
        name: fx.name,
        kind: "scanner",
        rgb,
        intensity,
        strobe,
        rotation: 0.0,
        segments: Vec::new(),
        whites: Vec::new(),
        pan,
        tilt,
    }
}

/// Decke / ceiling strip: 10 RGBW segments (16-bit gamma channels; we read the
/// coarse byte, which is plenty for a display).  White is folded into RGB.
fn view_decke(frame: &FrameSnapshot, fx: &Fixture) -> FixtureView {
    let mut segments = Vec::with_capacity(10);
    for seg in 0..10 {
        let base = seg * 4;
        let r = chan(frame, fx, base) as f32;
        let g = chan(frame, fx, base + 1) as f32;
        let b = chan(frame, fx, base + 2) as f32;
        let w = chan(frame, fx, base + 3) as f32;
        segments.push([
            clamp_u8(r + w),
            clamp_u8(g + w * 0.96),
            clamp_u8(b + w * 0.85),
        ]);
    }
    let mut rgb = [0u8; 3];
    let mut best = 0u32;
    for c in &segments {
        let l = c[0] as u32 + c[1] as u32 + c[2] as u32;
        if l > best {
            best = l;
            rgb = *c;
        }
    }
    let intensity = (rgb[0].max(rgb[1]).max(rgb[2])) as f32 / 255.0;
    FixtureView {
        name: fx.name,
        kind: "strip",
        rgb,
        intensity,
        strobe: 0,
        rotation: 0.0,
        segments,
        whites: Vec::new(),
        pan: 0.0,
        tilt: 0.0,
    }
}

/// The fixtures shown in the front view, in no particular order (the layout
/// file decides placement; this just decides which ones have live colour).
fn build_views(frame: &FrameSnapshot) -> Vec<FixtureView> {
    vec![
        view_rootpar(frame, &fixtures::ROOTPAR_1),
        view_rootpar(frame, &fixtures::ROOTPAR_2),
        view_rootpar(frame, &fixtures::ROOTPAR_3),
        view_rootpar(frame, &fixtures::ROOTPAR_4),
        view_pixstrobe(frame, &fixtures::PIXSTROBE_1),
        view_pixstrobe(frame, &fixtures::PIXSTROBE_2),
        view_quadphase(frame, &fixtures::QUADPHASE_1),
        view_quadphase(frame, &fixtures::QUADPHASE_2),
        view_scanner(frame, &fixtures::SCANNER_1),
        view_scanner(frame, &fixtures::SCANNER_2),
        view_decke(frame, &fixtures::LOUNGE_DECKE),
        view_decke(frame, &fixtures::BAR_DECKE),
    ]
}

#[derive(Serialize)]
struct FramePayload {
    fixtures: Vec<FixtureView>,
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// Serve the visualization single-page app.
pub async fn index() -> impl Responder {
    let html = include_str!("../static/visualization.html");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Serve the editable stage layout (read at runtime so edits don't need a
/// rebuild).  Falls back to the compiled-in copy if the file is missing.
pub async fn layout() -> impl Responder {
    let body = std::fs::read_to_string("static/stage_layout.json")
        .unwrap_or_else(|_| include_str!("../static/stage_layout.json").to_string());
    HttpResponse::Ok()
        .content_type("application/json; charset=utf-8")
        .body(body)
}

/// Live per-fixture colour snapshot, polled by the page.
pub async fn frame(data: actix_web::web::Data<crate::web::WebData>) -> impl Responder {
    let snap = data.frame.load_full();
    let payload = FramePayload {
        fixtures: build_views(&snap),
    };
    HttpResponse::Ok().json(payload)
}
