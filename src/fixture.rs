/// Resolution of a single logical channel.
#[derive(Clone, Copy, Debug)]
pub enum ChannelWidth {
    /// One DMX slot: `value * 255.0` rounded to u8.
    Bit8,
    /// Two consecutive DMX slots (coarse, fine), linear mapping:
    /// coarse = `(v * 65535) as u16 >> 8`, fine = `(v * 65535) as u16 & 0xFF`.
    Bit16,
    /// Two consecutive DMX slots with γ = 2.2 dimmer curve applied before
    /// encoding.  Input [0, 1] is treated as perceptual intensity; the curve
    /// expands the dark end of the range so that equal-step fades feel uniform
    /// to the human eye.  Use this for LED fixtures with linear DMX response.
    Bit16Gamma,
}

/// A single logical output channel within a fixture's DMX footprint.
#[derive(Clone, Copy, Debug)]
pub struct FixtureChannel {
    pub width: ChannelWidth,
    /// Byte offset from the fixture's `start_address` (0-based).
    pub dmx_offset: u16,
}

impl FixtureChannel {
    pub const fn new8(dmx_offset: u16) -> Self {
        Self {
            width: ChannelWidth::Bit8,
            dmx_offset,
        }
    }

    pub const fn new16(dmx_offset: u16) -> Self {
        Self {
            width: ChannelWidth::Bit16,
            dmx_offset,
        }
    }

    pub const fn new16g(dmx_offset: u16) -> Self {
        Self {
            width: ChannelWidth::Bit16Gamma,
            dmx_offset,
        }
    }
}

/// A fixture permanently patched to a specific DMX address.
/// `channels` lists the logical channels in the order that the corresponding
/// effect will output them.
#[derive(Debug)]
pub struct Fixture {
    pub name: &'static str,
    pub universe: u16,
    /// **1-based** DMX start address, matching the conventional display in
    /// every lighting console and ArtNet tool (1 = first slot in the universe).
    /// `render_fixture` converts this to a 0-based buffer index internally.
    pub start_address: u16,
    pub channels: &'static [FixtureChannel],
}

impl Fixture {
    /// How many logical channels this fixture has.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

// ---------------------------------------------------------------------------
// Per-universe DMX output buffer (512 bytes, standard DMX frame size).
// ---------------------------------------------------------------------------

pub type UniverseBuffer = [u8; 512];

/// Render a slice of logical channel values (one f32 per channel, already
/// clamped to `0.0..=1.0`) into the given universe buffer.
///
/// Each 8-bit channel occupies one slot; each 16-bit channel occupies two
/// consecutive slots (coarse then fine).
pub fn render_fixture(fixture: &Fixture, values: &[f32], buf: &mut UniverseBuffer) {
    debug_assert_eq!(
        values.len(),
        fixture.channels.len(),
        "channel count mismatch for fixture '{}'",
        fixture.name
    );

    for (ch, &val) in fixture.channels.iter().zip(values.iter()) {
        let clamped = val.clamp(0.0, 1.0);
        // Convert 1-based DMX address to a 0-based buffer index.
        // DMX addresses are universally displayed as 1–512 in hardware and
        // software; the transport buffer is 0-indexed. Subtracting 1 here
        // keeps the fixture patch file in the familiar convention.
        let base = (fixture.start_address - 1) + ch.dmx_offset;

        match ch.width {
            ChannelWidth::Bit8 => {
                let idx = base as usize;
                if idx < 512 {
                    buf[idx] = (clamped * 255.0).round() as u8;
                }
            }
            ChannelWidth::Bit16 => {
                let coarse_idx = base as usize;
                let fine_idx = coarse_idx + 1;
                let raw = (clamped * 65535.0).round() as u16;
                let coarse = (raw >> 8) as u8;
                let fine = (raw & 0xFF) as u8;
                if coarse_idx < 512 {
                    buf[coarse_idx] = coarse;
                }
                if fine_idx < 512 {
                    buf[fine_idx] = fine;
                }
            }
            ChannelWidth::Bit16Gamma => {
                let coarse_idx = base as usize;
                let fine_idx = coarse_idx + 1;
                let raw = (clamped.powf(2.0) * 65535.0).round() as u16;
                let coarse = (raw >> 8) as u8;
                let fine = (raw & 0xFF) as u8;
                if coarse_idx < 512 {
                    buf[coarse_idx] = coarse;
                }
                if fine_idx < 512 {
                    buf[fine_idx] = fine;
                }
            }
        }
    }
}
