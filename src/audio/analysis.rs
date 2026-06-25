//! The DSP pipeline: samples in, `AudioFeatures` out.
//!
//! Pure, allocation-light, and free of any audio-hardware or threading
//! concerns, so it can be unit-tested by feeding synthetic buffers. The
//! threading/orchestration lives in `mod.rs`; here we only transform numbers.
//!
//! Pipeline per frame (every `hop_size` new samples):
//! window → real FFT → magnitudes → log bands (AGC) → song-relative loudness →
//! spectral flux → adaptive onset → tempo/beat → arrangement intensity →
//! four-on-the-floor groove.

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

use super::beat::BeatTracker;
use super::config::{AudioConfig, NUM_BANDS};
use super::elements::ElementTracker;
use super::features::AudioFeatures;

/// First-order attack/release envelope follower.
#[derive(Clone, Copy)]
struct EnvFollower {
    value: f32,
    attack: f32,  // smoothing coefficient when rising
    release: f32, // smoothing coefficient when falling
}

impl EnvFollower {
    fn new(hop_s: f32, attack_s: f32, release_s: f32) -> Self {
        Self {
            value: 0.0,
            attack: coef(hop_s, attack_s),
            release: coef(hop_s, release_s),
        }
    }
    fn process(&mut self, x: f32) -> f32 {
        let c = if x > self.value { self.attack } else { self.release };
        self.value = x + c * (self.value - x);
        self.value
    }
}

/// Smoothing coefficient for a time constant: `exp(-hop/tau)`.
fn coef(hop_s: f32, tau_s: f32) -> f32 {
    if tau_s <= 0.0 {
        0.0
    } else {
        (-hop_s / tau_s).exp()
    }
}

/// Decaying-peak AGC reference: tracks a slowly-decaying maximum so a value
/// can be normalised to roughly `0..1` regardless of absolute level.
#[derive(Clone, Copy)]
struct AgcRef {
    peak: f32,
    decay: f32,
    floor: f32,
}

impl AgcRef {
    fn new(hop_s: f32, window_s: f32, floor: f32) -> Self {
        Self {
            peak: floor,
            // Per-frame decay so the peak halves roughly over the window.
            decay: (-hop_s / (window_s * 0.5)).exp(),
            floor,
        }
    }
    /// Update the reference with `x` and return `x` normalised to ~0..1.
    fn normalize(&mut self, x: f32) -> f32 {
        self.peak = (self.peak * self.decay).max(x).max(self.floor);
        (x / self.peak).clamp(0.0, 1.0)
    }
}

pub struct Analyzer {
    cfg: AudioConfig,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    in_buf: Vec<f32>,      // reusable FFT input (len fft_size)
    spectrum: Vec<Complex<f32>>, // reusable FFT output (len fft_size/2+1)
    mag: Vec<f32>,         // current magnitude spectrum
    prev_mag: Vec<f32>,    // previous magnitude spectrum (for flux)

    /// Sliding sample accumulator; we run a frame each time it reaches fft_size
    /// then drop `hop_size` from the front.
    accum: Vec<f32>,
    samples_seen: u64,

    band_bins: [(usize, usize); NUM_BANDS], // [lo, hi) bin index per band

    // envelopes
    band_slow: [EnvFollower; NUM_BANDS],
    band_fast: [EnvFollower; NUM_BANDS],
    energy_env: EnvFollower,

    // AGC
    band_agc: [AgcRef; NUM_BANDS],
    flux_agc: AgcRef,
    // kick-emphasised onset function for the beat tracker (the every-beat kick
    // is the most reliable tempo cue, but broadband flux buries it under snares)
    low_flux_agc: AgcRef,
    prev_low: f32,

    // onset detection
    flux_mean: f32,
    flux_var: f32,
    frames_since_onset: u32,
    min_onset_gap_frames: u32,
    onset_decay: f32,
    onset_level: f32,
    onset_flux_floor: f32,
    flux_log_lambda: f32,

    // activity gate (silence / beatless suppression)
    silence_rms_floor: f32,
    frames_below_floor: u32,        // consecutive frames with sub-floor RMS
    level_hold_frames: u32,         // hold the level gate open this long after a hit
    beat_onset_timeout_frames: u32,

    // beat
    beat: BeatTracker,

    // song-relative loudness (`energy`)
    aweight: Vec<f32>,       // per-bin A-weighting (linear)
    loud_init: bool,
    loud_floor_db: f32,
    loud_ceil_db: f32,
    loud_glide_db: f32,

    // --- intensity (arrangement-based) ---
    // Loudness is flat across modern masters, so the drive comes from sub-bass
    // presence + onset density + highs, each normalised over a long window.
    sub_bins: (usize, usize),
    high_bins: (usize, usize),
    struct_glide_db: f32,
    sub_init: bool,
    sub_floor_db: f32,
    sub_ceil_db: f32,
    high_init: bool,
    high_floor_db: f32,
    high_ceil_db: f32,
    bass_f: f32,             // fast EMA of normalised sub-bass
    bass_f_c: f32,
    onset_rate_acc: f32,     // leaky onset integrator
    onset_rate_c: f32,
    onset_rate_norm_div: f32,
    intensity_env: EnvFollower,
    elements: ElementTracker,

    // best-effort downbeat estimate: salience per beat-of-bar slot
    db_slots: [f32; 4],
    beat_in_bar: i32,        // 0=downbeat..3, or -1 when unsure
}

impl Analyzer {
    pub fn new(cfg: AudioConfig) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(cfg.fft_size);
        let in_buf = fft.make_input_vec();
        let spectrum = fft.make_output_vec();
        let nbins = spectrum.len();

        // Hann window.
        let window: Vec<f32> = (0..cfg.fft_size)
            .map(|i| {
                let x = std::f32::consts::PI * i as f32 / (cfg.fft_size as f32 - 1.0);
                x.sin().powi(2)
            })
            .collect();

        let bin_hz = cfg.sample_rate as f32 / cfg.fft_size as f32;
        let band_bins = log_band_bins(cfg.min_hz, cfg.max_hz, bin_hz, nbins);

        // Per-bin A-weighting (linear) for the informational loudness `energy`.
        let aweight: Vec<f32> = (0..nbins).map(|i| a_weight_linear(i as f32 * bin_hz)).collect();

        // Structural band bin ranges: sub-bass (kick/bass) and highs (risers).
        let bin = |hz: f32| ((hz / bin_hz).round() as usize).min(nbins.saturating_sub(1));
        let sub_bins = (bin(25.0), bin(150.0).max(bin(25.0) + 1));
        let high_bins = (bin(3500.0), bin(12000.0).max(bin(3500.0) + 1));

        let hop_s = cfg.hop_seconds();
        let band_slow = std::array::from_fn(|_| {
            EnvFollower::new(hop_s, cfg.band_slow_attack_s, cfg.band_slow_release_s)
        });
        let band_fast = std::array::from_fn(|_| {
            EnvFollower::new(hop_s, cfg.band_fast_attack_s, cfg.band_fast_release_s)
        });
        let energy_env = EnvFollower::new(hop_s, cfg.energy_attack_s, cfg.energy_release_s);

        let band_agc =
            std::array::from_fn(|_| AgcRef::new(hop_s, cfg.agc_window_s, cfg.agc_floor));
        let flux_agc = AgcRef::new(hop_s, cfg.agc_window_s, cfg.agc_floor);
        let low_flux_agc = AgcRef::new(hop_s, cfg.agc_window_s, cfg.agc_floor);

        let min_onset_gap_frames =
            ((cfg.min_onset_gap_ms / 1000.0) * cfg.analysis_hz()).round() as u32;

        let beat = BeatTracker::new(&cfg);

        Self {
            fft,
            window,
            in_buf,
            spectrum,
            mag: vec![0.0; nbins],
            prev_mag: vec![0.0; nbins],
            accum: Vec::with_capacity(cfg.fft_size * 2),
            samples_seen: 0,
            band_bins,
            band_slow,
            band_fast,
            energy_env,
            band_agc,
            flux_agc,
            low_flux_agc,
            prev_low: 0.0,
            flux_mean: 0.0,
            flux_var: 0.0,
            frames_since_onset: u32::MAX / 2,
            min_onset_gap_frames,
            onset_decay: coef(cfg.hop_seconds(), cfg.onset_decay_s),
            onset_level: 0.0,
            onset_flux_floor: cfg.onset_flux_floor,
            flux_log_lambda: cfg.flux_log_lambda,
            silence_rms_floor: cfg.silence_rms_floor,
            frames_below_floor: u32::MAX / 2,
            // Hold the gate open ~0.6 s after the last above-floor frame: long
            // enough to bridge the gaps between sparse hits (e.g. a click track's
            // silent inter-click gaps), short enough that real silence closes it
            // quickly instead of free-running for a second-plus.
            level_hold_frames: (0.6 * cfg.analysis_hz()).round().max(1.0) as u32,
            beat_onset_timeout_frames: ((cfg.beat_onset_timeout_s * cfg.analysis_hz())
                .round()
                .max(1.0)) as u32,
            beat,
            aweight,
            loud_init: false,
            loud_floor_db: 0.0,
            loud_ceil_db: 0.0,
            loud_glide_db: 30.0 / (cfg.loudness_window_s * cfg.analysis_hz()).max(1.0),
            sub_bins,
            high_bins,
            struct_glide_db: 36.0 / (cfg.loudness_window_s * cfg.analysis_hz()).max(1.0),
            sub_init: false,
            sub_floor_db: 0.0,
            sub_ceil_db: 0.0,
            high_init: false,
            high_floor_db: 0.0,
            high_ceil_db: 0.0,
            bass_f: 0.0,
            bass_f_c: coef(hop_s, 0.18),
            onset_rate_acc: 0.0,
            onset_rate_c: coef(hop_s, cfg.onset_rate_s),
            onset_rate_norm_div: (cfg.onset_rate_s * cfg.onset_rate_max).max(1e-3),
            intensity_env: EnvFollower::new(hop_s, 0.12, 0.40),
            elements: ElementTracker::new(hop_s, cfg.analysis_hz()),
            db_slots: [0.0; 4],
            beat_in_bar: -1,
            cfg,
        }
    }

    /// Normalise a band's dB power within a long-window adaptive [floor, ceil].
    fn norm_band(
        loud_db: f32,
        init: &mut bool,
        floor: &mut f32,
        ceil: &mut f32,
        glide: f32,
        min_range_db: f32,
    ) -> f32 {
        if !*init {
            *floor = loud_db;
            *ceil = loud_db;
            *init = true;
        } else {
            *ceil = loud_db.max(*ceil - glide);
            *floor = loud_db.min(*floor + glide);
        }
        let span = (*ceil - *floor).max(min_range_db);
        ((loud_db - *floor) / span).clamp(0.0, 1.0)
    }

    /// Feed mono samples. For every completed hop a fresh `AudioFeatures` is
    /// produced and passed to `emit` (in chronological order).
    pub fn feed(&mut self, samples: &[f32], mut emit: impl FnMut(AudioFeatures)) {
        for &s in samples {
            self.accum.push(s * self.cfg.pre_gain);
            self.samples_seen += 1;
            if self.accum.len() >= self.cfg.fft_size {
                let feat = self.run_frame();
                emit(feat);
                // advance by hop
                self.accum.drain(..self.cfg.hop_size);
            }
        }
    }

    /// Convenience for tests: feed a whole buffer and collect every frame.
    pub fn analyze_buffer(&mut self, samples: &[f32]) -> Vec<AudioFeatures> {
        let mut out = Vec::new();
        self.feed(samples, |f| out.push(f));
        out
    }

    fn run_frame(&mut self) -> AudioFeatures {
        let n = self.cfg.fft_size;

        // --- broadband time-domain level (pre-AGC) for the activity gate ---
        // Must be measured before the AGC, which normalises everything to look
        // self-relative and so can't tell silence from quiet music.
        let mut sum_sq = 0.0f32;
        for i in 0..n {
            sum_sq += self.accum[i] * self.accum[i];
        }
        let rms = (sum_sq / n as f32).sqrt();
        if rms > self.silence_rms_floor {
            self.frames_below_floor = 0;
        } else {
            self.frames_below_floor = self.frames_below_floor.saturating_add(1);
        }
        let level_active = self.frames_below_floor < self.level_hold_frames;

        // --- window into FFT input ---
        for i in 0..n {
            self.in_buf[i] = self.accum[i] * self.window[i];
        }

        // --- FFT → magnitudes ---
        self.fft
            .process(&mut self.in_buf, &mut self.spectrum)
            .expect("realfft process");
        for (m, c) in self.mag.iter_mut().zip(self.spectrum.iter()) {
            *m = c.norm();
        }

        // --- spectral flux (positive change) → onset detection function ---
        // Log-compressed magnitudes (`log(1 + λ|X|)`) so loud broadband
        // transients don't swamp the onset function; λ = 0 falls back to linear.
        let lambda = self.flux_log_lambda;
        let comp = |x: f32| if lambda > 0.0 { (1.0 + lambda * x).ln() } else { x };
        let mut flux_raw = 0.0f32;
        for (m, pm) in self.mag.iter().zip(self.prev_mag.iter()) {
            let d = comp(*m) - comp(*pm);
            if d > 0.0 {
                flux_raw += d;
            }
        }
        flux_raw /= self.mag.len() as f32;
        self.prev_mag.copy_from_slice(&self.mag);

        // --- raw per-band energy (mean magnitude in band) ---
        let mut band_raw = [0.0f32; NUM_BANDS];
        for (b, &(lo, hi)) in self.band_bins.iter().enumerate() {
            if hi > lo {
                let s: f32 = self.mag[lo..hi].iter().copied().sum();
                band_raw[b] = s / (hi - lo) as f32;
            }
        }

        // --- kick-emphasised onset function for tempo (sub-bass flux) ---
        let low_now = band_raw[0] + band_raw[1];
        let low_flux = (low_now - self.prev_low).max(0.0);
        self.prev_low = low_now;
        let low_flux_norm = self.low_flux_agc.normalize(low_flux);

        // --- AGC normalize ---
        let mut bands = [0.0f32; NUM_BANDS];
        let mut bands_fast = [0.0f32; NUM_BANDS];
        for b in 0..NUM_BANDS {
            let norm = self.band_agc[b].normalize(band_raw[b]);
            bands[b] = self.band_slow[b].process(norm);
            bands_fast[b] = self.band_fast[b].process(norm);
        }
        let flux = self.flux_agc.normalize(flux_raw);

        // --- song-relative perceptual loudness (`energy`) ---
        // A-weighted spectral power → dB → normalised within the track's own
        // rolling loudness range, so intro reads low and drop reads high.
        let mut loud_power = 0.0f32;
        for (w, m) in self.aweight.iter().zip(self.mag.iter()) {
            let wm = *w * *m;
            loud_power += wm * wm;
        }
        loud_power /= self.mag.len() as f32;
        let loud_db = 10.0 * (loud_power + 1e-12).log10();
        if !self.loud_init {
            self.loud_floor_db = loud_db;
            self.loud_ceil_db = loud_db;
            self.loud_init = true;
        } else {
            self.loud_ceil_db = loud_db.max(self.loud_ceil_db - self.loud_glide_db);
            self.loud_floor_db = loud_db.min(self.loud_floor_db + self.loud_glide_db);
        }
        let span = (self.loud_ceil_db - self.loud_floor_db).max(self.cfg.loudness_min_range_db);
        let loud_norm = ((loud_db - self.loud_floor_db) / span).clamp(0.0, 1.0);
        let energy = self.energy_env.process(loud_norm);

        // --- onset detection (adaptive threshold on flux) ---
        // EMA mean/variance of flux for the rolling threshold.
        let alpha = 0.1f32;
        let dev = flux - self.flux_mean;
        self.flux_mean += alpha * dev;
        self.flux_var = (1.0 - alpha) * (self.flux_var + alpha * dev * dev);
        let std = self.flux_var.sqrt();
        // Absolute floor on the threshold so a quiet passage (where mean & std
        // both decay toward 0) can't fire onsets on AGC-amplified noise.
        let threshold =
            (self.flux_mean + self.cfg.onset_sensitivity * std).max(self.onset_flux_floor);

        self.frames_since_onset = self.frames_since_onset.saturating_add(1);
        let mut onset_strength = 0.0f32;
        if level_active && flux > threshold && self.frames_since_onset >= self.min_onset_gap_frames {
            self.frames_since_onset = 0;
            onset_strength = ((flux - threshold) / threshold.max(1e-4)).clamp(0.0, 1.0);
            self.onset_level = onset_strength.max(self.onset_level);
        }
        // decay the reported onset level
        self.onset_level *= self.onset_decay;

        // --- onset density (events/sec, normalised) ---
        self.onset_rate_acc *= self.onset_rate_c;
        if onset_strength > 0.0 {
            self.onset_rate_acc += 1.0;
        }
        let onset_rate = (self.onset_rate_acc / self.onset_rate_norm_div).clamp(0.0, 1.0);

        // --- beat / tempo ---
        // Gate the tracker off when the input is silent (level) or beatless (no
        // onset within the timeout) so it stops free-running like a metronome.
        let onset_recent = self.frames_since_onset < self.beat_onset_timeout_frames;
        let active = level_active && onset_recent;
        // Feed the tracker a kick-weighted onset function so the every-beat kick
        // anchors the tempo (broadband `flux` alone over-weights the backbeat).
        let beat_flux = (0.4 * flux + low_flux_norm).min(1.0);
        let beat_now = self.beat.update(beat_flux, onset_strength, active);

        // --- intensity: the continuous "how energetic now" driver ---
        // Loudness is flat across modern masters, so intensity is read from the
        // ARRANGEMENT: sub-bass presence (kick/bass) dominant, plus onset
        // density and a little high content — each normalised song-relative.
        let band_db = |bins: (usize, usize)| -> f32 {
            let (lo, hi) = bins;
            let mut p = 0.0f32;
            for m in &self.mag[lo..hi] {
                p += *m * *m;
            }
            10.0 * ((p / (hi - lo) as f32) + 1e-12).log10()
        };
        let sub_norm = Self::norm_band(
            band_db(self.sub_bins),
            &mut self.sub_init, &mut self.sub_floor_db, &mut self.sub_ceil_db,
            self.struct_glide_db, 14.0,
        );
        let high_norm = Self::norm_band(
            band_db(self.high_bins),
            &mut self.high_init, &mut self.high_floor_db, &mut self.high_ceil_db,
            self.struct_glide_db, 14.0,
        );
        self.bass_f += (1.0 - self.bass_f_c) * (sub_norm - self.bass_f);
        let intensity = self.intensity_env.process(
            (0.55 * self.bass_f + 0.28 * onset_rate + 0.17 * high_norm).clamp(0.0, 1.0),
        );

        // --- four-on-the-floor groove state (raw low-band onsets vs the beat) ---
        let four_on_floor = self.elements.update(&band_raw, beat_now);

        // --- best-effort downbeat estimate ---
        // On each beat, accumulate the sub-bass salience into the slot for this
        // beat-of-bar; the slot with the most bass on average is the downbeat.
        // (Imperfect on syncopated material — gated on a confident lock; when
        // unsure, `beat_in_bar` is -1 and effects free-run the bar grid.)
        if beat_now && self.beat.bpm() > 0.0 {
            let bc = self.beat.beat_count();
            let slot = (bc % 4) as usize;
            self.db_slots[slot] = self.db_slots[slot] * 0.92 + sub_norm * 0.08;
            let (mut best, mut best_i) = (f32::MIN, 0usize);
            for (i, v) in self.db_slots.iter().enumerate() {
                if *v > best {
                    best = *v;
                    best_i = i;
                }
            }
            let mean = self.db_slots.iter().sum::<f32>() / 4.0;
            let clear = best > mean * 1.15; // downbeat slot stands out
            self.beat_in_bar = if clear && self.beat.confidence() > 0.5 {
                ((bc as i64 - best_i as i64).rem_euclid(4)) as i32
            } else {
                -1
            };
        }

        AudioFeatures {
            t_capture: self.samples_seen as f64 / self.cfg.sample_rate as f64,
            energy,
            intensity,
            four_on_floor,
            beat_in_bar: self.beat_in_bar,
            bands,
            bands_fast,
            onset: self.onset_level,
            beat_now,
            beat_phase: self.beat.phase(),
            beat_clock: self.beat.clock(),
            bpm: self.beat.bpm(),
            beat_confidence: self.beat.confidence(),
        }
    }
}

/// Linear A-weighting at frequency `f` (Hz). Approximates the ear's frequency
/// response: strong attenuation of sub-bass and very-high content. Normalised
/// to ~1.0 at 1 kHz.
fn a_weight_linear(f: f32) -> f32 {
    if f < 10.0 {
        return 0.0;
    }
    let f2 = f * f;
    let num = 12194.0f32.powi(2) * f2 * f2;
    let den = (f2 + 20.6f32.powi(2))
        * ((f2 + 107.7f32.powi(2)) * (f2 + 737.9f32.powi(2))).sqrt()
        * (f2 + 12194.0f32.powi(2));
    // +2 dB normalisation built into A-weighting → linear factor 10^(2/20).
    (num / den) * 1.258_925
}

/// Compute `[lo, hi)` magnitude-bin ranges for `NUM_BANDS` log-spaced bands.
fn log_band_bins(min_hz: f32, max_hz: f32, bin_hz: f32, nbins: usize) -> [(usize, usize); NUM_BANDS] {
    let lmin = min_hz.max(1.0).ln();
    let lmax = max_hz.ln();
    let mut edges = [0usize; NUM_BANDS + 1];
    for (i, e) in edges.iter_mut().enumerate() {
        let f = (lmin + (lmax - lmin) * i as f32 / NUM_BANDS as f32).exp();
        *e = ((f / bin_hz).round() as usize).min(nbins.saturating_sub(1));
    }
    std::array::from_fn(|b| {
        let lo = edges[b];
        let hi = edges[b + 1].max(lo + 1).min(nbins);
        (lo, hi)
    })
}
