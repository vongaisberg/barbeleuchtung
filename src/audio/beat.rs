//! Tempo estimation and beat-phase tracking.
//!
//! Fed the spectral-flux onset-detection function one frame at a time, this:
//!   1. estimates a *candidate* tempo each ~⅛ s by autocorrelating the recent
//!      flux envelope over the lag range implied by `bpm_min..bpm_max`,
//!   2. stabilises it — a rolling **median** of recent candidates kills
//!      frame-to-frame jitter, an **octave fold** stops 2×/½× flips, and a
//!      **lock with hysteresis** holds the tempo steady, only following a
//!      genuinely sustained tempo change, and
//!   3. runs a phase-locked loop that advances a continuous beat phase at the
//!      *locked* tempo and is gently nudged toward incoming onsets.
//!
//! The result is a BPM that sits still on steady music (what "filtered" means)
//! while phase stays aligned to the actual beat. The locked period is the
//! anchor; onsets only correct phase, never yank the tempo.

use super::config::AudioConfig;

pub struct BeatTracker {
    analysis_hz: f32,
    lag_min: usize,
    lag_max: usize,
    /// Recent flux values (the onset detection function), newest at the back.
    flux_hist: Vec<f32>,
    hist_cap: usize,

    /// Frames between tempo re-estimations.
    reestimate_every: usize,
    frames_since_est: usize,

    /// Recent fractional best-lag estimates (one per re-estimation), for the
    /// stabilising median.
    est_hist: Vec<f32>,
    est_cap: usize,

    /// The locked beat period in frames (0 until first lock). This is the
    /// anchor the phase advances on; it changes slowly and rarely.
    locked_period: f32,
    /// Consecutive estimates that disagree with the lock — triggers a re-lock
    /// once it crosses `relock_after`.
    disagree: usize,
    relock_after: usize,

    bpm: f32,
    confidence: f32,

    /// Beat phase, 0..1; wraps once per beat.
    phase: f32,
    /// Count of PLL-corrected beats since lock — the integer part of `clock()`.
    beat_count: u64,
    /// Base PLL correction gain applied on each onset (scaled down by lock
    /// confidence so a tight lock isn't yanked around).
    pll_gain: f32,

    /// Lag (frames) at the centre of the Rayleigh tempo-preference weighting.
    center_lag: f32,
    /// Consecutive gated-off (silent/beatless) frames; once it crosses ~2 s the
    /// lock is dropped so the next active passage re-locks cleanly.
    inactive_frames: usize,
    /// Frames of sustained inactivity after which the lock is released.
    unlock_after: usize,
}

impl BeatTracker {
    pub fn new(cfg: &AudioConfig) -> Self {
        let analysis_hz = cfg.analysis_hz();
        // lag (in frames) = analysis_hz * 60 / bpm; high bpm → short lag.
        let lag_min = (analysis_hz * 60.0 / cfg.bpm_max).floor().max(1.0) as usize;
        let lag_max = (analysis_hz * 60.0 / cfg.bpm_min).ceil() as usize;
        // Keep ~3× the longest period so autocorrelation has support.
        let hist_cap = (lag_max * 3).max(64);
        let reestimate_every = (analysis_hz as usize / 8).max(1); // ~8 Hz
        // ~3 s of candidate estimates feed the median.
        let est_cap = ((analysis_hz / reestimate_every as f32) * 3.0).round().max(7.0) as usize;
        Self {
            analysis_hz,
            lag_min,
            lag_max,
            flux_hist: Vec::with_capacity(hist_cap),
            hist_cap,
            reestimate_every,
            frames_since_est: 0,
            est_hist: Vec::with_capacity(est_cap),
            est_cap,
            locked_period: 0.0,
            disagree: 0,
            // ~1 s of sustained disagreement before we accept a new tempo.
            relock_after: (analysis_hz as usize / reestimate_every).max(4),
            bpm: 0.0,
            confidence: 0.0,
            phase: 0.0,
            beat_count: 0,
            pll_gain: 0.12,
            center_lag: (analysis_hz * 60.0 / cfg.tempo_center_bpm).max(1.0),
            inactive_frames: 0,
            // ~2 s of silence/beatlessness before the lock is dropped.
            unlock_after: (analysis_hz * 2.0).round().max(1.0) as usize,
        }
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
    pub fn phase(&self) -> f32 {
        self.phase
    }
    /// Total corrected beats elapsed plus current phase. Because it counts
    /// *PLL-corrected* beats (not a free-running integral), it stays aligned to
    /// the music's beat grid — so chases stepped on `clock.floor()` are both
    /// even and on-beat.
    pub fn clock(&self) -> f64 {
        self.beat_count as f64 + self.phase as f64
    }
    /// Number of beats counted since lock (the integer part of `clock`).
    pub fn beat_count(&self) -> u64 {
        self.beat_count
    }

    /// Advance one analysis frame. `flux` is the (post-AGC) onset detection
    /// function; `onset_strength` is >0 on frames where an onset fired; `active`
    /// is `false` when the input is silent or beatless (no recent onsets), which
    /// gates the tracker off so it stops free-running like a metronome. Returns
    /// `true` exactly on frames where a beat lands.
    pub fn update(&mut self, flux: f32, onset_strength: f32, active: bool) -> bool {
        // --- activity gate: stop the metronome on silence / beatless passages ---
        // The locked period otherwise advances the phase forever, emitting
        // beats with no reference to whether any beat is actually playing.
        if !active {
            self.inactive_frames += 1;
            // Bleed confidence away so downstream knows the beat is gone.
            self.confidence *= 0.94;
            // After sustained inactivity, drop the lock entirely so the next
            // active passage re-locks from scratch instead of resuming a stale
            // tempo at a stale phase.
            if self.inactive_frames >= self.unlock_after {
                self.locked_period = 0.0;
                self.phase = 0.0;
                self.disagree = 0;
                self.est_hist.clear();
            }
            return false;
        }
        self.inactive_frames = 0;

        // --- history ---
        if self.flux_hist.len() == self.hist_cap {
            self.flux_hist.remove(0);
        }
        self.flux_hist.push(flux);

        // --- periodic tempo re-estimation ---
        self.frames_since_est += 1;
        if self.frames_since_est >= self.reestimate_every && self.flux_hist.len() >= self.lag_max * 2
        {
            self.frames_since_est = 0;
            self.estimate_tempo();
        }

        // --- advance phase on the locked period ---
        let mut beat = false;
        if self.locked_period > 0.0 {
            let inc = 1.0 / self.locked_period;
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                beat = true;
            }

            // --- PLL: nudge phase toward onsets (period stays locked) ---
            if onset_strength > 0.0 {
                let err = if self.phase < 0.5 { self.phase } else { self.phase - 1.0 };
                // Tighter lock → smaller correction, so steady music doesn't jitter.
                let g = self.pll_gain * onset_strength.min(1.0) * (1.0 - 0.6 * self.confidence);
                self.phase -= g * err;
                if self.phase < 0.0 {
                    self.phase += 1.0;
                } else if self.phase >= 1.0 {
                    self.phase -= 1.0;
                    beat = true;
                }
            }

            // Count each corrected beat once (drives the aligned `clock`).
            if beat {
                self.beat_count = self.beat_count.wrapping_add(1);
            }
        }

        // `beat_now` reports a beat on the grid; how much to *trust* it is
        // `confidence`, which consumers gate on (so the downbeat detector and
        // the beat clock stay fed even on an ambiguous-but-active passage).
        beat
    }

    /// Autocorrelate the recent flux envelope, then fold the candidate into the
    /// stabilising median + lock.
    fn estimate_tempo(&mut self) {
        let f = &self.flux_hist;
        let n = f.len();
        let mean = f.iter().copied().sum::<f32>() / n as f32;

        let mut best_lag = 0usize;
        let mut best_score = f32::MIN;
        let mut sum_score = 0.0f32;
        let mut count = 0u32;
        let mut scores = Vec::with_capacity(self.lag_max - self.lag_min + 1);

        for lag in self.lag_min..=self.lag_max.min(n / 2) {
            let mut acc = 0.0f32;
            for i in lag..n {
                acc += (f[i] - mean) * (f[i - lag] - mean);
            }
            acc /= (n - lag) as f32;
            // Rayleigh tempo prior: bias the correlation toward human-typical
            // tempi so octave ambiguities resolve sensibly. Peaks at center_lag.
            let r = self.center_lag / lag as f32;
            let weighted = acc * r * (-0.5 * r * r).exp();
            scores.push(weighted);
            sum_score += weighted;
            count += 1;
            if weighted > best_score {
                best_score = weighted;
                best_lag = lag;
            }
        }
        // Reject when there is no real periodicity: a flat/near-zero flux
        // envelope (silence, sustained noise) yields no positive correlation
        // peak, so we must not manufacture a tempo from it.
        if best_lag == 0 || count == 0 || best_score <= 0.0 {
            self.confidence *= 0.9;
            return;
        }

        // --- octave correction ---
        // The autocorrelation often peaks at half-tempo on four-on-the-floor
        // music (the bar/backbeat period). If the *double*-tempo lag has
        // comparable support (a real pulse there — kick on every beat) and the
        // locked tempo is below the usual club range, prefer the faster octave.
        let mut candidate = self.refine_peak(best_lag, mean);
        let bpm_of = |p: f32| self.analysis_hz * 60.0 / p;
        loop {
            let half = candidate * 0.5;
            if bpm_of(candidate) >= 112.0 || half < self.lag_min as f32 {
                break;
            }
            let s_full = score_at(&scores, self.lag_min, candidate);
            let s_half = score_at(&scores, self.lag_min, half);
            if s_half > 0.0 && s_half > 0.62 * s_full {
                candidate = half; // the double-tempo pulse is real → go faster
            } else {
                break;
            }
        }

        // --- peak prominence: how much the best lag stands out from the mean ---
        // Computed before locking so a weak, ambiguous peak can't seed a lock.
        let avg_score = sum_score / count as f32;
        let prominence = if best_score > 0.0 {
            (1.0 - (avg_score / best_score)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // --- rolling median of candidates kills per-estimate jitter ---
        if self.est_hist.len() == self.est_cap {
            self.est_hist.remove(0);
        }
        self.est_hist.push(candidate);
        let median = median(&self.est_hist);

        // --- lock with hysteresis + octave fold ---
        if self.locked_period <= 0.0 {
            // Only take the *first* lock once the peak is clearly periodic;
            // otherwise stay unlocked (and silent). A flat/noise envelope has low
            // prominence, so this blocks locking onto non-periodic input without
            // adding latency on real music (which is prominent immediately).
            if prominence < 0.15 {
                return;
            }
            self.locked_period = median;
        } else {
            // Fold the median into the locked octave so 2×/½× don't re-lock.
            let mut folded = median;
            while folded < self.locked_period * 0.75 {
                folded *= 2.0;
            }
            while folded > self.locked_period * 1.5 {
                folded *= 0.5;
            }
            let diff = (folded - self.locked_period).abs() / self.locked_period;
            if diff < 0.04 {
                // Agree: gentle pull, stay locked.
                self.locked_period += 0.10 * (folded - self.locked_period);
                self.disagree = 0;
            } else {
                // Disagree: only re-lock after sustained disagreement, and lock
                // onto the *raw* median (a real tempo change, not an octave fold).
                self.disagree += 1;
                if self.disagree >= self.relock_after {
                    self.locked_period = median;
                    self.disagree = 0;
                }
            }
        }

        self.bpm = self.analysis_hz * 60.0 / self.locked_period;

        // --- confidence: peak prominence × agreement of recent estimates ---
        let agreement = {
            let m = median.max(1.0);
            let spread = std_dev(&self.est_hist) / m; // coefficient of variation
            (1.0 - spread * 4.0).clamp(0.0, 1.0)
        };
        // Smooth so the reported confidence doesn't flicker either.
        let target = (0.5 * prominence + 0.5 * agreement).clamp(0.0, 1.0);
        self.confidence += 0.25 * (target - self.confidence);
    }

    /// Parabolic peak refinement around integer `lag`. Returns a fractional lag.
    fn refine_peak(&self, lag: usize, mean: f32) -> f32 {
        let score = |l: usize| -> f32 {
            let f = &self.flux_hist;
            let n = f.len();
            if l == 0 || l >= n {
                return 0.0;
            }
            let mut acc = 0.0f32;
            for i in l..n {
                acc += (f[i] - mean) * (f[i - l] - mean);
            }
            acc / (n - l) as f32
        };
        if lag <= self.lag_min || lag >= self.lag_max {
            return lag as f32;
        }
        let y0 = score(lag - 1);
        let y1 = score(lag);
        let y2 = score(lag + 1);
        let denom = y0 - 2.0 * y1 + y2;
        if denom.abs() < 1e-9 {
            return lag as f32;
        }
        let delta = 0.5 * (y0 - y2) / denom;
        lag as f32 + delta.clamp(-1.0, 1.0)
    }
}

/// Interpolated autocorrelation score at a fractional `lag`, from the per-lag
/// `scores` array that starts at `lag_min`.
fn score_at(scores: &[f32], lag_min: usize, lag: f32) -> f32 {
    if lag < lag_min as f32 || scores.is_empty() {
        return 0.0;
    }
    let idx = lag - lag_min as f32;
    let i = idx.floor() as usize;
    if i + 1 >= scores.len() {
        return *scores.get(i).unwrap_or(&0.0);
    }
    let frac = idx - i as f32;
    scores[i] * (1.0 - frac) + scores[i + 1] * frac
}

/// Median of a slice (copying; slices here are short — a few dozen elements).
fn median(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v: Vec<f32> = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let m = v.len() / 2;
    if v.len() % 2 == 0 {
        0.5 * (v[m - 1] + v[m])
    } else {
        v[m]
    }
}

fn std_dev(xs: &[f32]) -> f32 {
    if xs.len() < 2 {
        return 0.0;
    }
    let mean = xs.iter().copied().sum::<f32>() / xs.len() as f32;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / xs.len() as f32;
    var.sqrt()
}
