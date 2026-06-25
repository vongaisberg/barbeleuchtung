//! Audio input abstraction.
//!
//! Everything downstream (the DSP pipeline) consumes a mono stream of `f32`
//! samples and never knows where they came from. That lets dev (system
//! loopback), production (network PCM stream) and tests (synthetic generator)
//! share one analysis path.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

/// A bounded, thread-safe mono-sample queue feeding the analysis thread.
///
/// Single-producer (the source) / single-consumer (the analyzer) in practice.
/// A `Condvar` lets the consumer block until samples arrive instead of spinning.
#[derive(Clone)]
pub struct SampleSink {
    inner: Arc<(Mutex<VecDeque<f32>>, Condvar)>,
    capacity: usize,
}

impl SampleSink {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new((Mutex::new(VecDeque::with_capacity(capacity)), Condvar::new())),
            capacity,
        }
    }

    /// Push mono samples. If the queue is full (consumer fell behind) the
    /// oldest samples are dropped — realtime audio must never block the source.
    pub fn push_mono(&self, samples: &[f32]) {
        let (lock, cv) = &*self.inner;
        let mut q = lock.lock().unwrap();
        for &s in samples {
            if q.len() == self.capacity {
                q.pop_front();
            }
            q.push_back(s);
        }
        cv.notify_one();
    }

    /// Block until at least one sample is available, then drain up to `max`
    /// samples into `out`. Returns the number drained (0 only if `max` is 0).
    pub fn drain_blocking(&self, out: &mut Vec<f32>, max: usize) -> usize {
        let (lock, cv) = &*self.inner;
        let mut q = lock.lock().unwrap();
        while q.is_empty() {
            q = cv.wait(q).unwrap();
        }
        let n = q.len().min(max);
        out.extend(q.drain(..n));
        n
    }
}

/// An audio input. Implementations spawn their own capture thread(s) and push
/// mono samples into the provided [`SampleSink`], returning immediately.
pub trait AudioSource: Send {
    /// Sample rate, in Hz, of the stream this source produces.
    fn sample_rate(&self) -> u32;

    /// Begin producing samples into `sink`. Must not block the caller.
    fn start(self: Box<Self>, sink: SampleSink);
}

/// A deterministic synthetic source for tests and offline tuning.
///
/// Generates a click track at a fixed BPM (a short decaying sine burst on each
/// beat) optionally mixed with a steady tone, so the pipeline can be validated
/// with no audio hardware: a 120 BPM click must produce `bpm ≈ 120` and a
/// `beat_now` pulse twice a second.
pub struct SyntheticSource {
    pub sample_rate: u32,
    /// Click track tempo. Set 0 to disable clicks.
    pub bpm: f32,
    /// Steady sine tone frequency in Hz mixed under the clicks. 0 disables.
    pub tone_hz: f32,
    /// Tone amplitude, 0..1.
    pub tone_amp: f32,
    /// Total seconds to generate before stopping.
    pub seconds: f32,
}

impl SyntheticSource {
    /// Render the whole signal to a buffer (used directly by unit tests).
    pub fn render(&self) -> Vec<f32> {
        let sr = self.sample_rate as f32;
        let total = (sr * self.seconds) as usize;
        let beat_period = if self.bpm > 0.0 { sr * 60.0 / self.bpm } else { f32::INFINITY };
        // Click = 40 ms decaying 1.2 kHz burst.
        let click_len = (sr * 0.040) as usize;
        let click_hz = 1200.0_f32;
        let tau = std::f32::consts::TAU;

        let mut out = Vec::with_capacity(total);
        for i in 0..total {
            let t = i as f32 / sr;
            let mut s = 0.0;
            if self.tone_hz > 0.0 {
                s += self.tone_amp * (tau * self.tone_hz * t).sin();
            }
            if self.bpm > 0.0 {
                let pos_in_beat = (i as f32) % beat_period;
                if (pos_in_beat as usize) < click_len {
                    let k = pos_in_beat / sr; // seconds into click
                    let env = (-k * 60.0).exp(); // fast decay
                    s += 0.9 * env * (tau * click_hz * k).sin();
                }
            }
            out.push(s.clamp(-1.0, 1.0));
        }
        out
    }
}

/// Raw-PCM sample format on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcmFormat {
    /// 32-bit little-endian float, range -1.0..1.0.
    F32Le,
    /// 16-bit little-endian signed int.
    S16Le,
}

impl PcmFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "f32" | "f32le" | "float" | "float32" | "float32le" => Some(Self::F32Le),
            "s16" | "s16le" | "i16" | "int16" => Some(Self::S16Le),
            _ => None,
        }
    }
    pub fn bytes_per_sample(self) -> usize {
        match self {
            Self::F32Le => 4,
            Self::S16Le => 2,
        }
    }
}

/// Drain every *complete* interleaved frame from `accum`, downmix to mono, and
/// return the mono samples. Any trailing partial frame is left in `accum` for
/// the next read. Pure and unit-testable.
pub fn decode_frames(format: PcmFormat, channels: usize, accum: &mut Vec<u8>) -> Vec<f32> {
    let ch = channels.max(1);
    let frame_bytes = format.bytes_per_sample() * ch;
    if frame_bytes == 0 {
        return Vec::new();
    }
    let n_frames = accum.len() / frame_bytes;
    let mut mono = Vec::with_capacity(n_frames);
    for fr in 0..n_frames {
        let base = fr * frame_bytes;
        let mut sum = 0.0f32;
        for c in 0..ch {
            let off = base + c * format.bytes_per_sample();
            let s = match format {
                PcmFormat::F32Le => {
                    f32::from_le_bytes([accum[off], accum[off + 1], accum[off + 2], accum[off + 3]])
                }
                PcmFormat::S16Le => {
                    i16::from_le_bytes([accum[off], accum[off + 1]]) as f32 / 32768.0
                }
            };
            sum += s;
        }
        mono.push(sum / ch as f32);
    }
    // Keep the trailing partial frame.
    accum.drain(..n_frames * frame_bytes);
    mono
}

/// Reads raw interleaved PCM from **stdin** and feeds it to the analyzer.
///
/// This is the robust dev capture path: pipe your sink's monitor through the
/// OS's own recorder, which taps playback passively and never degrades output
/// quality (no cpal, no device grab, no Bluetooth profile switch). E.g.:
///
/// ```text
/// parec --format=float32le --rate=44100 --channels=2 \
///       --device=@DEFAULT_MONITOR@ | barbeleuchtung
/// # or with PipeWire:
/// pw-record --target=<sink>.monitor --rate=44100 --channels=2 - | barbeleuchtung
/// ```
///
/// It is also the shape of the production/container input: anything that writes
/// PCM to the process's stdin works.
pub struct StdinPcmSource {
    pub sample_rate: u32,
    pub channels: usize,
    pub format: PcmFormat,
}

impl AudioSource for StdinPcmSource {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn start(self: Box<Self>, sink: SampleSink) {
        std::thread::Builder::new()
            .name("audio-stdin".into())
            .spawn(move || {
                use std::io::Read;
                let stdin = std::io::stdin();
                let mut lock = stdin.lock();
                let frame_bytes = self.format.bytes_per_sample() * self.channels.max(1);
                let mut read_buf = vec![0u8; frame_bytes * 2048];
                let mut accum: Vec<u8> = Vec::with_capacity(read_buf.len() * 2);
                log::info!(
                    "Audio stdin: reading {:?} @ {} Hz, {} ch",
                    self.format,
                    self.sample_rate,
                    self.channels
                );
                loop {
                    match lock.read(&mut read_buf) {
                        Ok(0) => {
                            log::warn!("Audio stdin closed (EOF); capture stopped");
                            break;
                        }
                        Ok(n) => {
                            accum.extend_from_slice(&read_buf[..n]);
                            let mono = decode_frames(self.format, self.channels, &mut accum);
                            if !mono.is_empty() {
                                sink.push_mono(&mono);
                            }
                        }
                        Err(e) => {
                            log::error!("Audio stdin read error: {e}");
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn stdin audio thread");
    }
}

impl AudioSource for SyntheticSource {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn start(self: Box<Self>, sink: SampleSink) {
        let buf = self.render();
        let sr = self.sample_rate as usize;
        std::thread::Builder::new()
            .name("audio-synthetic".into())
            .spawn(move || {
                // Feed in ~10 ms chunks at real-time pace so the analyzer sees
                // a realistic stream rather than one giant burst.
                let chunk = (sr / 100).max(1);
                for c in buf.chunks(chunk) {
                    sink.push_mono(c);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            })
            .expect("failed to spawn synthetic audio thread");
    }
}
