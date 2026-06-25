# Realtime Audio-Reactive Lightshow — Design & Plan

Status: **draft / in progress** · Owner: you + Claude · Last updated: 2026-06-22

## Goal

Make the rig generate a high-quality lightshow **live, by listening to the music** — not by
locking pre-authored timecoded shows to known songs (which is what the existing Spotify
integration does). It must work on *any* audio, be musically intelligent (beat, tempo,
spectral structure, drops/sections), and be **modular and configurable** so every stage can
be tuned and swapped later.

This lives in a **brand-new bank** of generative effects. The Alice-in-Wonderland theming is
intentionally dropped for these — reactive scenes are their own visual language.

## How this differs from what exists today

| | Existing Spotify path | New reactive path |
|---|---|---|
| Knows the song? | Yes — matches title, plays a hand-authored timecoded show | No — analyses audio in real time |
| Works on arbitrary music? | No | Yes |
| Drives | FX bank (`show_time` locked to `progress_ms`) | New **Reactive bank** |
| Input | Spotify `currently-playing` poll (5 s) | Live PCM samples (system loopback / network stream) |
| Effort to add a song | Author a whole timecoded `fx_*` file | Zero — it just reacts |

The two coexist. The reactive bank is additive over the engine's universe buffers exactly like
Decke and FX, so nothing about the existing banks changes.

## Audio source strategy (dev vs production)

Two deployment realities, one trait:

- **Dev / testing (outside Docker):** capture **system audio loopback** — on Linux this is the
  PipeWire/PulseAudio *monitor* of the default sink, which `cpal` sees as an input device. Reacts
  to whatever is playing on the machine (Spotify, files, browser, DJ software).
- **Production (inside the container):** the host pushes a **network PCM stream** into the
  container (the container has no audio device). A tiny framed-PCM-over-UDP protocol keeps latency
  low and tolerates packet loss; a small host-side streamer reads the mixer/loopback and sends it.

Both are implementations of one `AudioSource` trait that yields interleaved `f32` sample frames
at a known sample rate. The analysis pipeline never knows or cares which source it's fed. `cpal`
is compiled behind a `loopback` cargo feature so the default build (and CI / this sandbox) needs
no system audio libraries; production can run the network source with no extra deps.

## Architecture

Three threads, lock-free handoff, no blocking on the realtime paths:

```
 ┌──────────────┐   samples    ┌────────────────┐  AudioFeatures   ┌───────────────┐
 │ AudioSource  │ ───ring────▶ │ Analysis loop  │ ───ArcSwap────▶ │ DMX engine    │
 │ (capture cb) │  (SPSC)      │ (DSP @ ~hop)   │   (lock-free)   │ (40 Hz tick)  │
 └──────────────┘              └────────────────┘                 └───────────────┘
   loopback OR                  FFT→bands→flux→                     reads latest
   network OR                   onset→tempo/beat→                   features each
   synthetic                    envelopes→features                 tick; reactive
                                                                    bank effects map
                                                                    them to fixtures
```

1. **Capture thread** — driven by the source (cpal callback, UDP recv, or synthetic generator).
   Pushes mono-downmixed samples into a single-producer/single-consumer ring. Never blocks.
2. **Analysis thread** — pulls a `hop_size` of new samples, slides them into an `fft_size`
   analysis window, runs the DSP pipeline, and **publishes** a fresh immutable `AudioFeatures`
   snapshot via `arc_swap::ArcSwap`. Runs faster than the engine (e.g. hop 512 @ 44.1 kHz ≈ 86 Hz)
   so onsets are timed tightly.
3. **Engine thread** — unchanged 40 Hz loop. Each tick it `load()`s the latest `AudioFeatures`
   (one atomic pointer read) and hands it to effects via `TickContext.audio`. Effects in other
   banks ignore it; reactive effects read it.

### DSP pipeline (modular stages, each independently configurable)

Order, with the config knobs that tune each stage:

1. **Downmix + pre-gain** — stereo→mono, optional DC block. `pre_gain`.
2. **Window** — Hann over `fft_size` (2048 default). `fft_size`.
3. **Real FFT** — `realfft` (wraps `rustfft`), magnitude spectrum.
4. **Band split** — `num_bands` log-spaced bands (default 8: sub, bass, low-mid, mid, high-mid,
   presence, brilliance, air). `num_bands`, `min_hz`, `max_hz`.
5. **Loudness / RMS / peak** — broadband level for AGC + energy. `—`.
6. **Spectral flux** — half-wave-rectified positive change vs previous spectrum → onset detection
   function. `flux_lambda` (adaptive-threshold sensitivity).
7. **Onset detection** — adaptive median/moving-average threshold on the flux → discrete onset
   events with strength. `onset_threshold`, `onset_window`, `min_onset_gap_ms`.
8. **Tempo estimation** — autocorrelation / comb-filter bank over the recent flux envelope →
   BPM + confidence, constrained to `bpm_min..bpm_max` (default 70–180). `bpm_min`, `bpm_max`.
9. **Beat phase tracker** — phase-locked loop seeded by onsets and the tempo estimate → continuous
   `beat_phase ∈ [0,1)` and a `beat_now` pulse, so effects can anticipate the beat between onsets.
10. **Envelope followers** — per-feature attack/release smoothing; each band gets a *slow* (musical)
    and *fast* (transient) envelope. `attack_ms`, `release_ms` (per feature group).
11. **Adaptive normalization (AGC)** — rolling min/max (or percentile) per band + broadband, so the
    show looks the same whether the music is quiet or loud. `agc_window_s`, `agc_floor`.
12. **High-level descriptors** — `energy` (slow broadband envelope), `drop` (sharp energy rise after
    a lull → decaying pulse), `centroid` (brightness), section-novelty (longer-horizon spectral
    change). `drop_*`, `section_*`.

Everything downstream of the FFT is **pure math on plain buffers** → fully unit-testable with
synthetic audio (sine sweeps, click tracks at known BPM) with no audio hardware. That's how the
prototype is validated in CI/sandbox.

### `AudioFeatures` (the contract between DSP and effects)

Immutable snapshot, normalized to `0.0..1.0` unless noted:

```
t_capture: f64        // engine-clock seconds when this frame was analysed
rms, peak: f32        // broadband loudness (post-AGC)
energy: f32           // slow energy envelope (section level)
bands[N]: f32         // per-band slow envelope (musical)
bands_fast[N]: f32    // per-band fast envelope (transients)
flux: f32             // onset detection function (post-AGC)
onset: f32            // onset strength this frame, decays to 0
beat_now: bool        // true on the tick a beat lands
beat_phase: f32       // 0..1 within the current beat
bpm: f32              // current tempo estimate
beat_confidence: f32  // 0..1 tempo lock confidence
drop: f32             // decaying pulse after a detected drop
centroid: f32         // spectral brightness 0..1
```

A `silent()` constructor gives the all-zero baseline used before audio arrives and in non-reactive
contexts.

### Engine integration (smallest possible change)

- `TickContext` gains one field: `audio: Arc<AudioFeatures>`. The `Effect::tick` **signature is
  unchanged**, so none of the dozens of existing effect impls are touched — only the 3 places that
  *construct* a `TickContext` literal.
- A third bank `themes::all_reactive_themes()` with `AppState.active_reactive_theme_id` +
  crossfade, rendered additively right after the FX bank in `engine::run`. Same machinery as FX.
- `main.rs` builds an `AudioConfig`, picks a source (synthetic / loopback / network), calls
  `audio::spawn()` → gets the `ArcSwap` handle, and passes it into `engine::run`.

### Configurability

`AudioConfig` is one struct holding every knob above. Prototype: sensible hardcoded defaults.
Next: load/override from a TOML file; then expose the high-traffic knobs (sensitivity, BPM range,
per-band gain, smoothing) live in the web UI, and per-effect params (color palette, which bands
drive which fixtures, gain curves) on the reactive scenes — so you can "play around with everything."

## Reactive effects (initial generative vocabulary)

All read `ctx.audio`; all parameterised. Drop the narrative theming — these are about energy.

- **BandWash** → RootPars: map bands to color (bass→warm/red, mids→green, highs→blue), brightness
  follows that band's envelope; whole-rig dimmer scaled by `energy`.
- **BeatStrobe** → PixStrobes: white/segment flash on `beat_now`/`onset`, intensity by onset
  strength; rate-limited so it reads as punches, not seizure.
- **SpectrumBar** → PixStrobe RGB segments: 8 segments ← 8 bands, a live spectrum analyser in light.
- **EnergyDerby** → QuadPhase: rotation speed ∝ `energy`, color steps on beats, shutter strobe on `drop`.
- **DropFlash** → full stage hit + (later) scanner stab on `drop`.

A reactive *scene* is just a `Theme` binding these effects to fixtures — so you compose scenes the
same way you already author Decke/FX themes.

## Phased plan

**Phase 0 — Analysis core (this session).** `audio` module, `AudioSource` trait, `SyntheticSource`
(click track / tone generator), the full DSP pipeline (FFT→bands→flux→onset→tempo→beat→envelopes→AGC),
`AudioFeatures` + `ArcSwap` publisher. Unit tests: known-BPM click track → correct `bpm`/`beat_now`;
tone sweep → correct band activation. Compiles and tests pass with no audio hardware.

**Phase 1 — Live capture (your machine).** `cpal` `LoopbackSource` behind the `loopback` feature;
run against system monitor, eyeball features via a debug log / tiny meter.

**Phase 2 — Engine + reactive bank.** Add `TickContext.audio`, the reactive bank in the engine, and
2–3 effects above; wire `main.rs`. First real reactive scene driving RootPars + PixStrobes.

**Phase 3 — Musical intelligence polish.** Tighten tempo/beat PLL, drop & section detection, AGC;
add the richer effects (SpectrumBar, EnergyDerby, scanner stabs).

**Phase 4 — Network source for the container.** Framed-PCM-over-UDP `NetworkSource` + host-side
streamer; latency budget and a sync/heartbeat. Default-on in the Docker deployment.

**Phase 5 — Configurability surface.** TOML config load; web-UI controls for live knobs and
per-scene effect params.

## Key decisions & defaults (open to change)

- **DSP libs:** `realfft` (real-input FFT, pure Rust) + `arc-swap` (lock-free publish). `cpal`
  optional behind `loopback`. No C deps in the default build.
- **fft_size 2048, hop 512 @ 44.1 kHz** → ~86 Hz analysis, ~12 ms hop. Good onset timing; tune later.
- **8 log bands, 70–180 BPM** tempo window. All in `AudioConfig`.
- **Latency target** end-to-end < ~50 ms (capture buffer + hop + one engine tick) for tight beat sync.

## Risks / things to watch

- **Beat tracking is the hard part.** Real-time tempo/phase lock on messy bar audio is genuinely
  difficult; the modular design lets us start simple (flux + autocorrelation) and upgrade the tracker
  without touching effects.
- **Loopback in Docker doesn't exist** — production *must* use the network source; don't ship loopback
  to the container.
- **AGC tuning** decides whether quiet songs still drive the rig; needs real-bar testing.
- **cpal build deps** on the dev machine (ALSA headers) — isolated behind the feature flag.
