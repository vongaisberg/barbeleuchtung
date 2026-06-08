#![allow(dead_code)]

use chrono::NaiveTime;

/// Context passed to every effect on each engine tick.
#[derive(Clone, Debug)]
pub struct TickContext {
    /// Monotonic tick counter since engine start.
    pub tick: u64,
    /// Seconds elapsed since engine start (f64 for smooth animations).
    pub time: f64,
    /// Seconds elapsed since the currently-active FX theme was activated.
    /// For the Decke bank this mirrors `time`; for the FX bank it is reset to
    /// ~0 each time a new FX scene is selected.  Timecoded shows key off this
    /// so playback can be started in sync with scene activation rather than
    /// engine boot.
    pub show_time: f64,
    /// Current wall-clock time for time-of-day effects.
    pub wall_clock: NaiveTime,
    /// Delta time in seconds since the last tick.
    pub dt: f64,
    /// Slot information – where this fixture sits within a group/matrix.
    /// For single-fixture bindings this is always `0`.
    pub slot: usize,
}

/// Core effect abstraction. An effect maps a `TickContext` to a flat vector of
/// logical channel values, one `f32` per channel. Values are unclamped –
/// clamping to `0.0..=1.0` happens at the render stage.
pub trait Effect: Send + Sync {
    /// How many logical channels this effect produces.
    fn channel_count(&self) -> usize;

    /// Evaluate the effect for the given tick, returning one value per channel.
    fn tick(&self, ctx: &TickContext) -> Vec<f32>;
}

// ---------------------------------------------------------------------------
// Primitive effects
// ---------------------------------------------------------------------------

/// All channels held at a constant value for all time.
pub struct Constant {
    pub values: Vec<f32>,
}

impl Constant {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Convenience: N channels all at the same value.
    pub fn uniform(channels: usize, value: f32) -> Self {
        Self {
            values: vec![value; channels],
        }
    }
}

impl Effect for Constant {
    fn channel_count(&self) -> usize {
        self.values.len()
    }

    fn tick(&self, _ctx: &TickContext) -> Vec<f32> {
        self.values.clone()
    }
}

/// A sinusoidal oscillation applied to each channel independently.
/// Output = `offset + amplitude * sin(2π * freq * time + phase[i])`
pub struct Sine {
    pub channel_count: usize,
    /// Oscillation frequency in Hz.
    pub freq: f64,
    /// Per-channel phase offset in radians (length must equal `channel_count`).
    pub phases: Vec<f64>,
    /// DC offset added to the sine result (applied to all channels).
    pub offset: f32,
    /// Peak amplitude of the sine wave.
    pub amplitude: f32,
}

impl Sine {
    /// Simple single-channel sine with no phase offset.
    pub fn simple(freq: f64, offset: f32, amplitude: f32) -> Self {
        Self {
            channel_count: 1,
            freq,
            phases: vec![0.0],
            offset,
            amplitude,
        }
    }

    /// Multi-channel sine with equal phase separation (useful for RGB chases).
    pub fn multi_phase(channels: usize, freq: f64, offset: f32, amplitude: f32) -> Self {
        let phases = (0..channels)
            .map(|i| 2.0 * std::f64::consts::PI * i as f64 / channels as f64)
            .collect();
        Self {
            channel_count: channels,
            freq,
            phases,
            offset,
            amplitude,
        }
    }
}

impl Effect for Sine {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let base = 2.0 * std::f64::consts::PI * self.freq * ctx.time;
        (0..self.channel_count)
            .map(|i| {
                let phase = if i < self.phases.len() {
                    self.phases[i]
                } else {
                    0.0
                };
                self.offset + self.amplitude * (base + phase).sin() as f32
            })
            .collect()
    }
}

/// A sequential chase: at any given time only one channel is lit, sweeping
/// forward at `speed` channels per second. The active channel ramps to
/// `peak` with a configurable `width` (fraction of the cycle per channel).
pub struct Chase {
    pub channel_count: usize,
    /// How many channels advance per second.
    pub speed: f64,
    /// Fraction of one channel-period during which the channel is fully lit
    /// (0.0 to 1.0).  Values < 1.0 produce gaps; values > 1.0 overlap.
    pub width: f64,
    /// Peak value of the active channel.
    pub peak: f32,
}

impl Chase {
    pub fn new(channel_count: usize, speed: f64) -> Self {
        Self {
            channel_count,
            speed,
            width: 1.0,
            peak: 1.0,
        }
    }
}

impl Effect for Chase {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let n = self.channel_count as f64;
        // Fractional position within the full chase cycle (0.0..n).
        let pos = (ctx.time * self.speed).rem_euclid(n);
        (0..self.channel_count)
            .map(|i| {
                // Distance from the chase head, wrapping around.
                let mut dist = pos - i as f64;
                if dist < 0.0 {
                    dist += n;
                }
                if dist < self.width {
                    self.peak
                } else {
                    0.0
                }
            })
            .collect()
    }
}

/// A strobe effect: alternates between `peak` and `0.0` at `hz` flashes per
/// second. Can be used as a multiplicative envelope on top of another effect.
pub struct Strobe {
    pub channel_count: usize,
    /// Flashes per second.
    pub hz: f64,
    /// Duty cycle (0.0..1.0) – fraction of each period where the output is on.
    pub duty: f64,
    /// On-state value.
    pub peak: f32,
}

impl Strobe {
    pub fn new(channel_count: usize, hz: f64) -> Self {
        Self {
            channel_count,
            hz,
            duty: 0.5,
            peak: 1.0,
        }
    }
}

impl Effect for Strobe {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let phase = (ctx.time * self.hz).fract();
        let value = if phase < self.duty { self.peak } else { 0.0 };
        vec![value; self.channel_count]
    }
}
