//! System-audio loopback capture via `cpal` (dev only, behind `loopback`).
//!
//! On Linux/PipeWire/PulseAudio the *monitor* of the default sink appears as an
//! input device — set it as the default source (or pick it in `pavucontrol`)
//! and this captures whatever the machine is playing. On macOS use a loopback
//! driver (BlackHole/Loopback) as the default input.
//!
//! NOTE: this module is only compiled with `--features loopback` and is not
//! exercised by the sandbox/CI test suite (no audio device there). It is the
//! Phase-1 path you run on your own machine.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::source::{AudioSource, SampleSink};

pub struct LoopbackSource {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    sample_rate: u32,
    channels: usize,
}

impl LoopbackSource {
    /// Open an input device for capture.
    ///
    /// Device selection: if `BB_AUDIO_DEVICE` is set, the first input device
    /// whose name contains that substring (case-insensitive) is used; otherwise
    /// the system default input. **Always** capture a *sink monitor* here, never
    /// a real microphone — see the note below on audio quality.
    ///
    /// All available input devices are logged at startup so you can see exactly
    /// what's there and copy the right name into `BB_AUDIO_DEVICE`.
    ///
    /// ## Why capturing the wrong device wrecks playback
    /// Opening a microphone-type input can force the shared audio device into a
    /// low-quality duplex mode. The worst case is **Bluetooth**: any open mic
    /// switches the headset from A2DP (stereo, full-band) to HFP/HSP (mono,
    /// ~8–16 kHz telephone quality), so your music suddenly sounds like a phone
    /// call. The cure is to capture the **monitor of your output sink** instead
    /// (PipeWire/PulseAudio: set "Monitor of <sink>" as the default recording
    /// device, or pass its name via `BB_AUDIO_DEVICE`). A monitor is a passive
    /// tap on playback and never changes the output profile or sample rate.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();

        // Log every input device + its default config so the operator can pick.
        log::info!("Loopback: available input devices —");
        if let Ok(devices) = host.input_devices() {
            for d in devices {
                let name = d.name().unwrap_or_else(|_| "<unknown>".into());
                match d.default_input_config() {
                    Ok(c) => log::info!(
                        "  • {name}  ({} Hz, {} ch, {:?})",
                        c.sample_rate().0,
                        c.channels(),
                        c.sample_format()
                    ),
                    Err(_) => log::info!("  • {name}  (no default config)"),
                }
            }
        }

        let wanted = std::env::var("BB_AUDIO_DEVICE").ok().filter(|s| !s.is_empty());
        let device = match &wanted {
            Some(substr) => {
                let lc = substr.to_lowercase();
                host.input_devices()
                    .ok()
                    .and_then(|mut it| {
                        it.find(|d| {
                            d.name()
                                .map(|n| n.to_lowercase().contains(&lc))
                                .unwrap_or(false)
                        })
                    })
                    .ok_or_else(|| format!("no input device matching BB_AUDIO_DEVICE=\"{substr}\""))?
            }
            None => host
                .default_input_device()
                .ok_or_else(|| "no default input device".to_string())?,
        };

        // Use the device's native config so nothing is force-resampled.
        let config = device
            .default_input_config()
            .map_err(|e| format!("default_input_config: {e}"))?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let name = device.name().unwrap_or_else(|_| "<unknown>".into());
        log::info!("Loopback: capturing \"{name}\" @ {sample_rate} Hz, {channels} ch");
        if wanted.is_none() {
            log::info!(
                "Loopback: using the DEFAULT input. If your music quality drops, set \
                 BB_AUDIO_DEVICE to your sink's monitor (see the devices listed above)."
            );
        }
        Ok(Self {
            device,
            config,
            sample_rate,
            channels,
        })
    }
}

impl AudioSource for LoopbackSource {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn start(self: Box<Self>, sink: SampleSink) {
        // cpal `Stream` is not `Send` on all platforms, so build and own it on
        // a dedicated thread that then parks to keep the stream alive.
        std::thread::Builder::new()
            .name("audio-loopback".into())
            .spawn(move || {
                let channels = self.channels;
                let err_fn = |e| log::warn!("loopback stream error: {e}");
                let sink_f32 = sink.clone();

                let stream = match self.config.sample_format() {
                    cpal::SampleFormat::F32 => self.device.build_input_stream(
                        &self.config.clone().into(),
                        move |data: &[f32], _: &_| push_downmixed(&sink_f32, data, channels),
                        err_fn,
                        None,
                    ),
                    cpal::SampleFormat::I16 => self.device.build_input_stream(
                        &self.config.clone().into(),
                        move |data: &[i16], _: &_| {
                            let f: Vec<f32> =
                                data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                            push_downmixed(&sink_f32, &f, channels);
                        },
                        err_fn,
                        None,
                    ),
                    cpal::SampleFormat::U16 => self.device.build_input_stream(
                        &self.config.clone().into(),
                        move |data: &[u16], _: &_| {
                            let f: Vec<f32> = data
                                .iter()
                                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                                .collect();
                            push_downmixed(&sink_f32, &f, channels);
                        },
                        err_fn,
                        None,
                    ),
                    other => {
                        log::error!("unsupported loopback sample format: {other:?}");
                        return;
                    }
                };

                match stream {
                    Ok(s) => {
                        if let Err(e) = s.play() {
                            log::error!("failed to start loopback stream: {e}");
                            return;
                        }
                        // Keep the stream (and thread) alive forever.
                        loop {
                            std::thread::park();
                        }
                    }
                    Err(e) => log::error!("failed to build loopback stream: {e}"),
                }
            })
            .expect("failed to spawn loopback thread");
    }
}

/// Downmix interleaved frames to mono and push to the sink.
fn push_downmixed(sink: &SampleSink, interleaved: &[f32], channels: usize) {
    if channels <= 1 {
        sink.push_mono(interleaved);
        return;
    }
    let mono: Vec<f32> = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .collect();
    sink.push_mono(&mono);
}
