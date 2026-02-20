#![allow(dead_code)]
//! Hardcoded fixture patch for the bar.
//!
//! Edit this file to match your actual DMX patch. Each fixture definition
//! lists its Art-Net universe (0-based), its 0-based DMX start address, and
//! the ordered logical channels (8-bit or 16-bit) that effects will drive.
//!
//! # Fader mapping (independent faders)
//! Faders are also fixtures listed here. The fader index in `FADERS` must
//! match the order expected by the web UI and the state module.

use crate::fixture::{Fixture, FixtureChannel};

// ---------------------------------------------------------------------------
// RootPars
// ---------------------------------------------------------------------------

static ROOTPAR_CHANNELS: &[FixtureChannel] = &[
    FixtureChannel::new8(0), // Dimmer
    FixtureChannel::new8(1), // Strobe
    FixtureChannel::new8(2), // Red
    FixtureChannel::new8(3), // Green
    FixtureChannel::new8(4), // Blue
    FixtureChannel::new8(5), // White
    FixtureChannel::new8(6), // Amber
    FixtureChannel::new8(7), // UV
];

pub static ROOTPAR_1: Fixture = Fixture {
    name: "RootPar 1",
    universe: 1,
    start_address: 10,
    channels: ROOTPAR_CHANNELS,
};

pub static ROOTPAR_2: Fixture = Fixture {
    name: "RootPar 2",
    universe: 1,
    start_address: 20,
    channels: ROOTPAR_CHANNELS,
};
pub static ROOTPAR_3: Fixture = Fixture {
    name: "RootPar 3",
    universe: 1,
    start_address: 30,
    channels: ROOTPAR_CHANNELS,
};
pub static ROOTPAR_4: Fixture = Fixture {
    name: "RootPar 4",
    universe: 1,
    start_address: 40,
    channels: ROOTPAR_CHANNELS,
};

// ---------------------------------------------------------------------------
// QuadPhase
// ---------------------------------------------------------------------------

static QUADPHASE_CHANNELS: &[FixtureChannel] = &[
    FixtureChannel::new8(0), // Color
    FixtureChannel::new8(1), // Rotation
    FixtureChannel::new8(2), // Strobe
    FixtureChannel::new8(3), // Shutter
];

pub static QUADPHASE_1: Fixture = Fixture {
    name: "QuadPhase 1",
    universe: 1,
    start_address: 140,
    channels: QUADPHASE_CHANNELS,
};
pub static QUADPHASE_2: Fixture = Fixture {
    name: "QuadPhase 2",
    universe: 1,
    start_address: 150,
    channels: QUADPHASE_CHANNELS,
};

// ---------------------------------------------------------------------------
// PixStrobe
// ---------------------------------------------------------------------------

static PIXSTROBE_CHANNELS: &[FixtureChannel] = &[

// Bright strip in the middle
FixtureChannel::new8(0), // WW 1
FixtureChannel::new8(1), // WW 2
FixtureChannel::new8(2), // WW 3
FixtureChannel::new8(3), // WW 4
FixtureChannel::new8(4), // CW 1
FixtureChannel::new8(5), // CW 2
FixtureChannel::new8(6), // CW 3
FixtureChannel::new8(7), // CW 4
// RGB segments 2x4
FixtureChannel::new8(8), // R 1
FixtureChannel::new8(9), // G 1
FixtureChannel::new8(10), // B 1
FixtureChannel::new8(11), // R 2
FixtureChannel::new8(12), // G 2
FixtureChannel::new8(13), // B 2
FixtureChannel::new8(14), // R 3
FixtureChannel::new8(15), // G 3
FixtureChannel::new8(16), // B 3
FixtureChannel::new8(17), // R 4
FixtureChannel::new8(18), // G 4
FixtureChannel::new8(19), // B 4
FixtureChannel::new8(20), // R 5
FixtureChannel::new8(21), // G 5
FixtureChannel::new8(22), // B 5
FixtureChannel::new8(23), // R 6
FixtureChannel::new8(24), // G 6
FixtureChannel::new8(25), // B 6
FixtureChannel::new8(26), // R 7
FixtureChannel::new8(27), // G 7
FixtureChannel::new8(28), // B 7
FixtureChannel::new8(29), // R 8
FixtureChannel::new8(30), // G 8
FixtureChannel::new8(31), // B 8
];

pub static PIXSTROBE_1: Fixture = Fixture {
    name: "PixStrobe 1",
    universe: 1,
    start_address: 50,
    channels: PIXSTROBE_CHANNELS,
};
pub static PIXSTROBE_2: Fixture = Fixture {
    name: "PixStrobe 2",
    universe: 1,
    start_address: 100,
    channels: PIXSTROBE_CHANNELS,
};
// ---------------------------------------------------------------------------
// Dimmer fixtures
// ---------------------------------------------------------------------------

static DIMMER_CHANNELS: &[FixtureChannel] = &[
    FixtureChannel::new8(0), // speed
];
pub static ARRI_1: Fixture = Fixture {
    name: "Arri 1",
    universe: 0,
    start_address: 2,
    channels: DIMMER_CHANNELS,
};
pub static ARRI_2: Fixture = Fixture {
    name: "Arri 2",
    universe: 0,
    start_address: 3,
    channels: DIMMER_CHANNELS,
};
pub static FAN_ZULUFT: Fixture = Fixture {
    name: "Zuluft",
    universe: 0,
    start_address: 4,
    channels: DIMMER_CHANNELS,
};
pub static FAN_BAR: Fixture = Fixture {
    name: "Abluft Bar",
    universe: 0,
    start_address: 5,
    channels: DIMMER_CHANNELS,
};
pub static FAN_GANG: Fixture = Fixture {
    name: "Abluft Gang",
    universe: 0,
    start_address: 6,
    channels: DIMMER_CHANNELS,
};

// ---------------------------------------------------------------------------
// Decke
// ---------------------------------------------------------------------------

// 10xRGBW
static DECKE_CHANNELS: &[FixtureChannel] = &[
    FixtureChannel::new16(0),  // R 1
    FixtureChannel::new16(2),  // G 1
    FixtureChannel::new16(4),  // B 1
    FixtureChannel::new16(6),  // W 1
    FixtureChannel::new16(8),  // R 2
    FixtureChannel::new16(10), // G 2
    FixtureChannel::new16(12), // B 2
    FixtureChannel::new16(14), // W 2
    FixtureChannel::new16(16), // R 3
    FixtureChannel::new16(18), // G 3
    FixtureChannel::new16(20), // B 3
    FixtureChannel::new16(22), // W 3
    FixtureChannel::new16(24), // R 4
    FixtureChannel::new16(26), // G 4
    FixtureChannel::new16(28), // B 4
    FixtureChannel::new16(30), // W 4
    FixtureChannel::new16(32), // R 5
    FixtureChannel::new16(34), // G 5
    FixtureChannel::new16(36), // B 5
    FixtureChannel::new16(38), // W 5
    FixtureChannel::new16(40), // R 6
    FixtureChannel::new16(42), // G 6
    FixtureChannel::new16(44), // B 6
    FixtureChannel::new16(46), // W 6
    FixtureChannel::new16(48), // R 7
    FixtureChannel::new16(50), // G 7
    FixtureChannel::new16(52), // B 7
    FixtureChannel::new16(54), // W 7
    FixtureChannel::new16(56), // R 8
    FixtureChannel::new16(58), // G 8
    FixtureChannel::new16(60), // B 8
    FixtureChannel::new16(62), // W 8
    FixtureChannel::new16(64), // R 9
    FixtureChannel::new16(66), // G 9
    FixtureChannel::new16(68), // B 9
    FixtureChannel::new16(70), // W 9
    FixtureChannel::new16(72), // R 10
    FixtureChannel::new16(74), // G 10
    FixtureChannel::new16(76), // B 10
    FixtureChannel::new16(78), // W 10
];
pub static LOUNGE_DECKE: Fixture = Fixture {
    name: "Lounge",
    universe: 9,
    start_address: 1,
    channels: DECKE_CHANNELS,
};
pub static BAR_DECKE: Fixture = Fixture {
    name: "Bar",
    universe: 14,
    start_address: 1,
    channels: DECKE_CHANNELS,
};
// ---------------------------------------------------------------------------
// All theme-controlled fixtures in one flat slice (for the engine).
// Order here determines the order of `FixtureBinding`s in each theme.
// ---------------------------------------------------------------------------

pub static ALL_FIXTURES: &[&Fixture] = &[
    &ARRI_1,
    &ARRI_2,
    &ROOTPAR_1,
    &ROOTPAR_2,
    &ROOTPAR_3,
    &ROOTPAR_4,
    &LOUNGE_DECKE,
    &BAR_DECKE,
];

// ---------------------------------------------------------------------------
// Fader fixtures – each fader on the UI controls exactly one of these.
// Index in this slice matches the fader index in AppState::fader_values.
// ---------------------------------------------------------------------------

pub static FADER_FIXTURES: &[&Fixture] = &[
    &ARRI_1,
    &ARRI_2,
    &FAN_ZULUFT,
    &FAN_GANG,
    &FAN_BAR,
];

/// Labels shown in the UI for each fader (same order as `FADER_FIXTURES`).
pub static FADER_LABELS: &[&str] = &[
    "Arri Links",
    "Arri Rechts",
    "Zuluft",
    "Abluft Bar",
    "Abluft Gang",
];
