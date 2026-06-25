//! Configuration for the audio-reactive analysis pipeline.
//!
//! Every tunable knob in the DSP chain lives here so the whole show can be
//! retuned from one place (and, later, from a TOML file or the web UI).

/// Number of log-spaced frequency bands the spectrum is split into.
///
/// Kept a compile-time constant so `AudioFeatures` can use fixed-size arrays
/// (no per-frame allocation on the realtime path). Change here to re-band.
pub const NUM_BANDS: usize = 8;

/// All parameters controlling capture framing and the DSP pipeline.
#[derive(Clone, Debug)]
pub struct AudioConfig {
    // --- capture / framing ---
    /// Input sample rate in Hz (set by the source; analysis honours it).
    pub sample_rate: u32,
    /// FFT window length in samples. Power of two recommended.
    pub fft_size: usize,
    /// Samples advanced between successive analysis frames (the hop).
    /// Smaller = higher analysis rate = tighter onset timing, more CPU.
    pub hop_size: usize,
    /// Linear gain applied to incoming samples before analysis.
    pub pre_gain: f32,

    // --- band split ---
    /// Lowest band edge in Hz.
    pub min_hz: f32,
    /// Highest band edge in Hz.
    pub max_hz: f32,

    // --- envelope smoothing (seconds) ---
    /// Attack/release for the slow ("musical") per-band envelopes.
    pub band_slow_attack_s: f32,
    pub band_slow_release_s: f32,
    /// Attack/release for the fast ("transient") per-band envelopes.
    pub band_fast_attack_s: f32,
    pub band_fast_release_s: f32,
    /// Attack/release for the broadband energy envelope (section level).
    pub energy_attack_s: f32,
    pub energy_release_s: f32,

    // --- onset detection ---
    /// Adaptive-threshold sensitivity: an onset fires when flux exceeds the
    /// rolling mean by this many (rolling) standard deviations.
    pub onset_sensitivity: f32,
    /// Minimum gap between onsets (debounce), milliseconds.
    pub min_onset_gap_ms: f32,
    /// How fast a fired onset's reported strength decays back to 0 (seconds).
    pub onset_decay_s: f32,
    /// Absolute floor on the (post-AGC, 0..1) flux an onset must exceed, on top
    /// of the adaptive threshold. Stops the adaptive threshold collapsing to ~0
    /// in quiet passages and firing onsets on AGC-amplified noise.
    pub onset_flux_floor: f32,
    /// Spectral-flux log-compression strength `λ`: flux is computed on
    /// `log(1 + λ·|X|)` magnitudes. Compresses dynamics so loud broadband
    /// transients don't dominate the onset function. 0 = linear (no compression).
    pub flux_log_lambda: f32,

    // --- activity gate (silence / beatless suppression) ---
    /// Broadband time-domain RMS below which the input is treated as silent:
    /// the beat tracker is gated off and its lock decays. Absolute (pre-AGC) so
    /// it actually sees silence — every post-AGC signal looks self-relative.
    pub silence_rms_floor: f32,
    /// If no onset has fired for this long, the current passage is treated as
    /// beatless and `beat_now` is suppressed (even when sound is present, e.g. a
    /// held pad or ambient breakdown).
    pub beat_onset_timeout_s: f32,

    // --- tempo / beat ---
    /// Tempo search range, beats per minute.
    pub bpm_min: f32,
    pub bpm_max: f32,
    /// Preferred tempo (BPM) at the centre of a Rayleigh weighting applied to
    /// the autocorrelation, so octave ambiguities resolve toward human-typical
    /// tempi instead of an arbitrary harmonic.
    pub tempo_center_bpm: f32,

    // --- AGC / normalization ---
    /// Rolling window (seconds) over which per-band & broadband levels are
    /// normalized so the show looks consistent across loud/quiet music.
    pub agc_window_s: f32,
    /// Noise floor below which AGC won't amplify (avoids blowing up silence).
    pub agc_floor: f32,

    // --- song-relative loudness / intensity ---
    /// Rolling window (seconds) over which the track's loudness floor/ceiling
    /// adapt, so loudness is normalised relative to the song's own dynamics.
    pub loudness_window_s: f32,
    /// Minimum loudness span (dB) between floor and ceiling, so a flat-loudness
    /// passage can't blow tiny fluctuations up to full scale.
    pub loudness_min_range_db: f32,
    /// Onset-density leaky-integrator time constant (s).
    pub onset_rate_s: f32,
    /// Onsets/second that maps to `onset_rate = 1.0`.
    pub onset_rate_max: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            fft_size: 2048,
            hop_size: 512,
            pre_gain: 1.0,

            min_hz: 30.0,
            max_hz: 16_000.0,

            band_slow_attack_s: 0.020,
            band_slow_release_s: 0.180,
            band_fast_attack_s: 0.004,
            band_fast_release_s: 0.060,
            energy_attack_s: 0.100,
            energy_release_s: 0.600,

            onset_sensitivity: 2.2,
            min_onset_gap_ms: 90.0,
            onset_decay_s: 0.120,
            onset_flux_floor: 0.06,
            flux_log_lambda: 5.0,

            silence_rms_floor: 0.0015,
            beat_onset_timeout_s: 2.5,

            bpm_min: 70.0,
            bpm_max: 180.0,
            tempo_center_bpm: 120.0,

            agc_window_s: 4.0,
            agc_floor: 1e-4,

            loudness_window_s: 45.0,
            loudness_min_range_db: 12.0,
            onset_rate_s: 2.0,
            onset_rate_max: 8.0,
        }
    }
}

impl AudioConfig {
    /// Analysis frames per second given the configured sample rate and hop.
    pub fn analysis_hz(&self) -> f32 {
        self.sample_rate as f32 / self.hop_size as f32
    }

    /// Seconds of audio advanced per analysis frame.
    pub fn hop_seconds(&self) -> f32 {
        self.hop_size as f32 / self.sample_rate as f32
    }
}
