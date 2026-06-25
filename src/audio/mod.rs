//! Realtime audio-reactive analysis.
//!
//! A [`AudioSource`] produces mono samples → a background analysis thread runs
//! the [`Analyzer`] DSP pipeline → the latest [`AudioFeatures`] are published
//! through a lock-free [`FeatureHandle`] the engine reads each tick.
//!
//! See `REACTIVE_LIGHTSHOW.md` for the full design. The DSP core in
//! [`analysis`] and [`beat`] is hardware-free and unit-tested below with the
//! synthetic source, so it can be validated without any audio device.

pub mod analysis;
pub mod beat;
pub mod config;
pub mod elements;
pub mod features;
pub mod source;

#[cfg(feature = "loopback")]
pub mod loopback;

use std::sync::Arc;

pub use analysis::Analyzer;
pub use config::{AudioConfig, NUM_BANDS};
// Some re-exports are only consumed by tests or the (feature-gated) loopback
// path; keep them in the public surface regardless.
#[allow(unused_imports)]
pub use features::{new_handle, silent_features, AudioFeatures, FeatureHandle};
#[allow(unused_imports)]
pub use source::{AudioSource, PcmFormat, SampleSink, StdinPcmSource, SyntheticSource};

/// Start the analysis pipeline for `source`, returning the shared handle the
/// engine reads. Spawns the analysis thread and the source's capture thread;
/// returns immediately.
pub fn spawn(cfg: AudioConfig, source: Box<dyn AudioSource>) -> FeatureHandle {
    let handle = new_handle();
    // ~2 s of headroom so a briefly-late consumer doesn't drop audio.
    let sink = SampleSink::new(cfg.sample_rate as usize * 2);

    let analysis_handle = handle.clone();
    let analysis_sink = sink.clone();
    let cfg_for_thread = cfg.clone();
    std::thread::Builder::new()
        .name("audio-analysis".into())
        .spawn(move || {
            let mut analyzer = Analyzer::new(cfg_for_thread);
            let mut buf = Vec::with_capacity(4096);
            loop {
                buf.clear();
                analysis_sink.drain_blocking(&mut buf, 4096);
                // Publish only the most recent frame produced from this batch;
                // the engine samples at 40 Hz and only needs the latest state.
                let mut latest = None;
                analyzer.feed(&buf, |f| latest = Some(f));
                if let Some(f) = latest {
                    analysis_handle.store(Arc::new(f));
                }
            }
        })
        .expect("failed to spawn audio analysis thread");

    source.start(sink);
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> AudioConfig {
        AudioConfig::default()
    }

    /// A 120 BPM click track must be tracked to ≈120 BPM.
    #[test]
    fn detects_click_tempo() {
        let c = cfg();
        let src = SyntheticSource {
            sample_rate: c.sample_rate,
            bpm: 120.0,
            tone_hz: 0.0,
            tone_amp: 0.0,
            seconds: 10.0,
        };
        let samples = src.render();
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&samples);
        assert!(!frames.is_empty());

        // Use the tempo from the back half, once the tracker has locked.
        let tail = &frames[frames.len() / 2..];
        let bpm = tail[tail.len() - 1].bpm;
        assert!(
            (bpm - 120.0).abs() < 10.0,
            "expected ~120 BPM, got {bpm}"
        );

        // And we should have fired some beats.
        let beats = frames.iter().filter(|f| f.beat_now).count();
        assert!(beats > 10, "expected several beats, got {beats}");
    }

    /// A 90 BPM click track must track distinctly from 120.
    #[test]
    fn detects_different_tempo() {
        let c = cfg();
        let src = SyntheticSource {
            sample_rate: c.sample_rate,
            bpm: 90.0,
            tone_hz: 0.0,
            tone_amp: 0.0,
            seconds: 10.0,
        };
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&src.render());
        let bpm = frames.last().unwrap().bpm;
        assert!((bpm - 90.0).abs() < 10.0, "expected ~90 BPM, got {bpm}");
    }

    /// Once locked, the reported BPM must sit still on steady music — this is
    /// the "stops jumping around" guarantee. We measure the spread of BPM over
    /// the final few seconds and require it to be tiny.
    #[test]
    fn locked_bpm_is_stable() {
        let c = cfg();
        let src = SyntheticSource {
            sample_rate: c.sample_rate,
            bpm: 128.0,
            tone_hz: 0.0,
            tone_amp: 0.0,
            seconds: 14.0,
        };
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&src.render());
        // Last ~4 s of BPM readings.
        let tail: Vec<f32> = frames[frames.len() * 3 / 4..]
            .iter()
            .map(|f| f.bpm)
            .filter(|b| *b > 0.0)
            .collect();
        assert!(tail.len() > 20, "not enough locked frames");
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        let var = tail.iter().map(|b| (b - mean) * (b - mean)).sum::<f32>() / tail.len() as f32;
        let sd = var.sqrt();
        assert!((mean - 128.0).abs() < 8.0, "expected ~128 BPM, got {mean}");
        assert!(sd < 1.5, "BPM should be stable once locked, stddev={sd}");
    }

    /// Low and high tones must light up the low and high band halves resp.
    #[test]
    fn bands_follow_frequency() {
        let low = tone_bands(120.0);
        assert!(
            low[..NUM_BANDS / 2].iter().sum::<f32>() > low[NUM_BANDS / 2..].iter().sum::<f32>(),
            "120 Hz tone should favour low bands: {low:?}"
        );

        let high = tone_bands(6000.0);
        assert!(
            high[NUM_BANDS / 2..].iter().sum::<f32>() > high[..NUM_BANDS / 2].iter().sum::<f32>(),
            "6 kHz tone should favour high bands: {high:?}"
        );
    }

    /// Analyse a steady tone; return the final per-band envelope.
    fn tone_bands(hz: f32) -> [f32; NUM_BANDS] {
        let c = cfg();
        let src = SyntheticSource {
            sample_rate: c.sample_rate,
            bpm: 0.0,
            tone_hz: hz,
            tone_amp: 0.5,
            seconds: 3.0,
        };
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&src.render());
        frames.last().unwrap().bands
    }

    /// PCM frame decoding: correct downmix, and partial frames are retained.
    #[test]
    fn pcm_decode_downmix_and_remainder() {
        use super::source::{decode_frames, PcmFormat};
        // 2-channel f32: frames [L,R] = [1.0,0.0],[0.0,1.0] → mono 0.5,0.5,
        // plus 3 trailing bytes of a partial frame that must be kept.
        let mut bytes = Vec::new();
        for &s in &[1.0f32, 0.0, 0.0, 1.0] {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC]); // partial
        let mono = decode_frames(PcmFormat::F32Le, 2, &mut bytes);
        assert_eq!(mono, vec![0.5, 0.5]);
        assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC], "partial frame must remain");

        // s16 mono passthrough.
        let mut b2 = Vec::new();
        b2.extend_from_slice(&16384i16.to_le_bytes()); // 0.5
        let m2 = decode_frames(PcmFormat::S16Le, 1, &mut b2);
        assert!((m2[0] - 0.5).abs() < 1e-3, "got {}", m2[0]);
    }

    /// The fixed energy must be **song-relative**: a quiet passage reads low and
    /// a loud passage reads high (the old AGC ratio pinned both near 1.0).
    #[test]
    fn loudness_is_song_relative() {
        let c = cfg();
        let sr = c.sample_rate as usize;
        let tau = std::f32::consts::TAU;
        let mut buf = Vec::new();
        for (amp, secs) in [(0.08f32, 4.0f32), (0.8, 4.0)] {
            let n = (sr as f32 * secs) as usize;
            for i in 0..n {
                let t = i as f32 / sr as f32;
                buf.push(amp * (tau * 220.0 * t).sin());
            }
        }
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&buf);
        let half = frames.len() / 2;
        let mean = |s: &[AudioFeatures]| s.iter().map(|f| f.energy).sum::<f32>() / s.len() as f32;
        let quiet = mean(&frames[half / 2..half]); // settled quiet part
        let loud = mean(&frames[half + half / 2..]); // settled loud part
        assert!(quiet < 0.35, "quiet section should read low, got {quiet}");
        assert!(loud > 0.6, "loud section should read high, got {loud}");
        assert!(loud - quiet > 0.3, "must distinguish loud from quiet ({quiet}→{loud})");
    }

    /// Offline evaluation on a real recording (mono f32le). Not run by default —
    /// it needs an audio file on disk. Reports tempo/beat behaviour over the
    /// track and, critically, appends silence to confirm the tracker stops
    /// emitting beats (the "metronome in silence" regression).
    ///
    /// Run with, e.g.:
    /// ```text
    /// BB_EVAL_FILE=Prada.f32 BB_EVAL_RATE=44100 \
    ///   cargo test --release eval_real_file -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn eval_real_file() {
        let path = std::env::var("BB_EVAL_FILE").unwrap_or_else(|_| "Prada.f32".into());
        let rate: u32 = std::env::var("BB_EVAL_RATE").ok().and_then(|s| s.parse().ok()).unwrap_or(44_100);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip: cannot read {path}: {e}");
                return;
            }
        };
        let mut music: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let dur = music.len() as f32 / rate as f32;
        eprintln!("loaded {path}: {} samples, {:.1}s @ {rate} Hz", music.len(), dur);

        let c = AudioConfig { sample_rate: rate, ..Default::default() };
        let hz = c.analysis_hz();

        // --- Pass 1: the track as-is, windowed tempo/beat report. ---
        {
            let mut a = Analyzer::new(c.clone());
            let frames = a.analyze_buffer(&music);
            let win = (hz * 8.0) as usize; // 8 s windows
            eprintln!("--- per 8 s window: median bpm | mean conf | beats | beats/s ---");
            for (w, chunk) in frames.chunks(win).enumerate() {
                let mut bpms: Vec<f32> = chunk.iter().map(|f| f.bpm).filter(|b| *b > 0.0).collect();
                bpms.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let med = bpms.get(bpms.len() / 2).copied().unwrap_or(0.0);
                let conf = chunk.iter().map(|f| f.beat_confidence).sum::<f32>() / chunk.len() as f32;
                let beats = chunk.iter().filter(|f| f.beat_now).count();
                eprintln!(
                    "  t={:>5.0}s  bpm={:>5.1}  conf={:.2}  beats={:>3}  {:.2}/s",
                    w as f32 * 8.0, med, conf, beats, beats as f32 / 8.0
                );
            }
        }

        // --- Pass 2: 20 s of music, then 6 s of digital silence. ---
        // Beats must essentially stop during the silence.
        {
            let head = (rate as f32 * 20.0) as usize;
            let mut sig: Vec<f32> = music.drain(..head.min(music.len())).collect();
            let sil_start_t = sig.len() as f32 / rate as f32;
            sig.extend(std::iter::repeat(0.0).take((rate as f32 * 6.0) as usize));
            let mut a = Analyzer::new(c.clone());
            let frames = a.analyze_buffer(&sig);
            let sil_start_frame = (sil_start_t * hz) as usize;
            let before = frames[..sil_start_frame].iter().filter(|f| f.beat_now).count();
            let after = frames[sil_start_frame..].iter().filter(|f| f.beat_now).count();
            // When does the last beat fire, relative to silence onset?
            let last_beat_t = frames.iter().rposition(|f| f.beat_now).map(|i| i as f32 / hz);
            eprintln!("--- silence test: 20 s music + 6 s silence ---");
            eprintln!(
                "  beats during music: {before}, beats during 6 s silence: {after}, \
                 last beat at t={:?}s (silence starts at {sil_start_t:.1}s)",
                last_beat_t
            );
            assert!(
                after <= 4,
                "tracker kept firing in silence ({after} beats) — should stop within the unlock window"
            );
        }
    }

    /// Downbeat + bar tracking: a 120 BPM click with a bass thump on beat 1 of
    /// every bar must (a) yield a confident `beat_in_bar` and (b) advance it as
    /// a proper 4-beat cycle (each beat +1 mod 4).
    #[test]
    fn downbeat_and_bar_cycle() {
        let c = cfg();
        let sr = c.sample_rate as f32;
        let tau = std::f32::consts::TAU;
        let beat = sr * 0.5; // 120 BPM
        let click_len = (sr * 0.04) as usize;
        let thump_len = (sr * 0.13) as usize;
        let total = (sr * 24.0) as usize;
        let mut buf = Vec::with_capacity(total);
        for i in 0..total {
            let pos_in_beat = (i as f32) % beat;
            let beat_idx = (i as f32 / beat) as usize;
            let mut s = 0.0;
            if (pos_in_beat as usize) < click_len {
                let k = pos_in_beat / sr;
                s += 0.6 * (-k * 60.0).exp() * (tau * 1200.0 * k).sin();
            }
            // strong 55 Hz thump on the downbeat (beat 0 of each 4/4 bar)
            if beat_idx % 4 == 0 && (pos_in_beat as usize) < thump_len {
                let k = pos_in_beat / sr;
                s += 0.9 * (-k * 12.0).exp() * (tau * 55.0 * k).sin();
            }
            buf.push(s.clamp(-1.0, 1.0));
        }
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&buf);
        // beat_in_bar at each beat, second half only (after lock settles).
        let tail: Vec<i32> = frames[frames.len() / 2..]
            .iter()
            .filter(|f| f.beat_now)
            .map(|f| f.beat_in_bar)
            .collect();
        let confident: Vec<i32> = tail.iter().copied().filter(|&b| b >= 0).collect();
        assert!(confident.len() > 8, "downbeat should become confident, got {confident:?}");
        // Consecutive confident beats should step +1 mod 4 most of the time.
        let mut good = 0;
        let mut tot = 0;
        for w in confident.windows(2) {
            tot += 1;
            if (w[0] + 1).rem_euclid(4) == w[1] {
                good += 1;
            }
        }
        assert!(good as f32 / tot as f32 > 0.7, "bar should cycle 0..3: {confident:?}");
    }

    /// Octave correction: a four-on-the-floor pattern (kick every beat) with a
    /// LOUD backbeat snare on 2 & 4 — which biases naive autocorrelation toward
    /// half-tempo — must still lock to the true tempo, not half of it.
    #[test]
    fn locks_to_true_tempo_not_half() {
        let c = cfg();
        let sr = c.sample_rate as f32;
        let tau = std::f32::consts::TAU;
        let bpm_true = 140.0;
        let beat = sr * 60.0 / bpm_true;
        let total = (sr * 14.0) as usize;
        let mut buf = Vec::with_capacity(total);
        for i in 0..total {
            let pos = (i as f32) % beat;
            let beat_idx = (i as f32 / beat) as usize;
            let k = pos / sr;
            let mut s = 0.0;
            // kick: 55 Hz thump on every beat
            if (pos as usize) < (sr * 0.05) as usize {
                s += 0.5 * (-k * 35.0).exp() * (tau * 55.0 * k).sin();
            }
            // snare: louder bright burst on beats 2 & 4 (the backbeat)
            if beat_idx % 2 == 1 && (pos as usize) < (sr * 0.06) as usize {
                let env = (-k * 45.0).exp();
                s += 0.9 * env * ((tau * 300.0 * k).sin() + (tau * 900.0 * k).sin() + (tau * 1800.0 * k).sin()) / 3.0;
            }
            buf.push(s.clamp(-1.0, 1.0));
        }
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&buf);
        let bpm = frames.last().unwrap().bpm;
        assert!(
            (bpm - bpm_true).abs() < 12.0,
            "should lock to ~{bpm_true} (true tempo), got {bpm} (half = {})",
            bpm_true / 2.0
        );
    }

    /// The regression guard for the "metronome in silence" bug: once a tempo is
    /// locked on real beats, the tracker must STOP emitting beats shortly after
    /// the music stops — it must not free-run the locked period through silence.
    #[test]
    fn beats_stop_after_silence() {
        let c = cfg();
        let sr = c.sample_rate as usize;
        // 8 s of 120 BPM clicks, then 4 s of digital silence.
        let mut buf = SyntheticSource {
            sample_rate: c.sample_rate,
            bpm: 120.0,
            tone_hz: 0.0,
            tone_amp: 0.0,
            seconds: 8.0,
        }
        .render();
        let silence_start = buf.len();
        buf.extend(std::iter::repeat(0.0).take(sr * 4));

        let hz = c.analysis_hz();
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&buf);

        let split = ((silence_start as f32 / sr as f32) * hz) as usize;
        let during_music = frames[..split].iter().filter(|f| f.beat_now).count();
        let during_silence = frames[split..].iter().filter(|f| f.beat_now).count();

        assert!(during_music > 8, "should track beats while music plays, got {during_music}");
        // A couple of beats may slip out during the brief gate-close window, but
        // the metronome must not keep ticking through 4 s of silence.
        assert!(
            during_silence <= 3,
            "tracker free-ran through silence: {during_silence} beats (bug regression)"
        );
    }

    /// Silence must not panic and must produce ~zero features.
    #[test]
    fn silence_is_quiet() {
        let c = cfg();
        let samples = vec![0.0f32; c.sample_rate as usize * 2];
        let mut a = Analyzer::new(c);
        let frames = a.analyze_buffer(&samples);
        let f = frames.last().unwrap();
        assert!(f.energy < 0.05, "silence energy should be ~0, got {}", f.energy);
        assert!(!f.beat_now || f.bpm == 0.0);
    }
}
