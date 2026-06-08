# Barbeleuchtung – Lighting Setup Reference

This document describes the bar’s DMX lighting rig as patched in **Barbeleuchtung** (`src/fixtures.rs`). It is written for humans and AI agents who will author new ceiling (**Decke**) themes or FX show presets.

**Source of truth for addresses and channel order:** `src/fixtures.rs`  
**Shared scanner / helper constants:** `src/themes/mod.rs`  
**Example FX shows:** `src/themes/fx_*.rs`

---

## Venue overview

The venue has three functional zones in one room:

| Zone | Location | Lighting |
|------|----------|----------|
| **Lounge** | One side of the bar | `LOUNGE_DECKE` – atmospheric / theatrical ceiling |
| **Bar** | **Centre** of the room | `BAR_DECKE` - is above the bar patrons, the barkeepers have their own functional, non-DMX-controlled light. Can contain effects but shouldn't be too energetic. Perfectly fine to keep it in sync to the lounge lighting.s |
| **Stage** | Other side of the bar | FX rig (RootPars, PixStrobes, QuadPhases, Scanners) |

Patrons walk **past the bar** when moving between lounge and stage. The lounge carries most colour drama; the FX rig accents the stage area.

```
  [ LOUNGE ]  ←—— patrons walk past bar ——→  [ STAGE / FX rig ]
       |                                          |
  LOUNGE_DECKE                              PixStrobes, RootPars,
  (10 × RGBW)                               Quads, Scanners
                    [ BAR – centre ]
                         |
                    BAR_DECKE
```

The walls carry Alice-in-Wonderland murals. Lounge lighting is often designed to make painted colours “vibrate” via complementary wash (e.g. teal makes warm reds pop; purple makes greens recede).

---

## Software architecture (for effect authors)

### Two independent theme banks

| Bank | Registry | Context time | Typical use |
|------|----------|--------------|-------------|
| **Decke** | `themes::all_themes()` | `TickContext::time` (seconds since engine boot) | Time-of-day ambience: warm work light, evening colour, closed |
| **FX** | `themes::all_fx_themes()` | `TickContext::show_time` (seconds since FX scene was selected) | Manual button presets + timecoded song shows |

Both banks render **additively** into the same universe buffers each tick (40 Hz). Fixtures not listed in a theme’s bindings stay at zero for that bank.

**Important for song-synced shows:** Always key musical timing off `ctx.show_time`, not `ctx.time`. `show_time` resets to ~0 when the operator selects a new FX scene (i.e. when they press the button).

### Effect API

Every fixture is driven by an `Effect` that implements:

```rust
fn channel_count(&self) -> usize;
fn tick(&self, ctx: &TickContext) -> Vec<f32>;  // one f32 per logical channel, 0.0..1.0
```

Values are **logical** (0.0–1.0). The engine clamps and maps them to 8- or 16-bit DMX in `render_fixture()`.

`TickContext` fields:

| Field | Meaning |
|-------|---------|
| `time` | Monotonic seconds since engine start |
| `show_time` | Seconds since active FX theme was selected |
| `wall_clock` | Local time of day |
| `dt` | ~0.025 s (40 Hz tick) |
| `slot` | Index within a group binding (usually 0 for single bindings) |

### Independent faders (not theme-controlled)

These fixtures are **not** part of theme bindings. The web UI faders write directly on top of theme output:

| Fader | Fixture | Universe | Address |
|-------|---------|----------|---------|
| Arri 1 | `ARRI_1` | 0 | 2 |
| Arri 2 | `ARRI_2` | 0 | 3 |
| Zuluft (supply fan) | `FAN_ZULUFT` | 0 | 4 |
| Abluft Bar | `FAN_BAR` | 0 | 5 |
| Abluft Gang | `FAN_GANG` | 0 | 6 |
| Traffic Red | `TRAFFIC_RED` | 2 | 1 |
| Traffic Green | `TRAFFIC_GREEN` | 2 | 2 |

Fans and traffic lights use on/off UI toggles; Arris use continuous dimmers.

### Fog machine

`FOG_MACHINE` on Universe 0, address 10. Automated bursts (interval, duration, level) when enabled in UI—not theme-driven.

## DMX universes

| Universe | Label | Contents |
|----------|-------|----------|
| 0 | Floor | Arris, fans, fog |
| 1 | FX | RootPars, PixStrobes, QuadPhases, Scanners |
| 2 | Traffic | Traffic-light bulbs |
| 9 | Lounge | `LOUNGE_DECKE` |
| 10 | Bar | `BAR_DECKE` |

All addresses below are **1-based** (console convention).

---

## Fixture reference

### LOUNGE_DECKE / BAR_DECKE – ceiling RGBW spots

**Patch:** 10 logical spots × 4 channels (R, G, B, W) = 40 channels per fixture.  
**Universe:** Lounge = 9 @ 1, Bar = 10 @ 1.  
**Resolution:** 16-bit per channel with **γ = 2.2** (`Bit16Gamma`) for perceptually smooth dimming.

Channel order per spot (spot index 0–9):

```
[R, G, B, W] × 10   →   40 logical channels total
```

Use helpers in `src/themes/mod.rs`:

- `build_ceiling(|spot| Rgbw { ... })` – per-spot colour
- `uniform_ceiling(color)` – same colour on all spots
- `organic_lfo(t, spot, freq)` – golden-angle phase per spot
- `shaped_lfo(t, spot, period, k)` – hold + fade waveform

**Design notes from existing themes:**

- **Bar:** usually static warm white, 30–80 % intensity. Avoid RGB tinting during service hours—bartenders need accurate colour rendering.
- **Lounge:** per-spot independent motion via golden-angle phase offsets so the room never looks synchronized. Typical intensity ranges 25–100 % depending on time of day.
- **Spot index 0–9:** DMX order is sequential. **Spot 0** is at the **far end of the lounge, left side** (when facing the bar). Spots increment from there—confirm the exact winding order (e.g. along the long edge vs across) before doing positional chases along the ceiling.

---

### ROOTPAR 1–4 – RGBW PAR washes (FX backdrop)

**Hardware:** Generic 8-channel RGBWAU PAR (exact brand TBD).  
**Universe:** 1  
**Addresses:** 10, 20, 30, 40 (10 channels apart)

**Physical order (audience view, left → right):** RootPar **1 – 2 – 3 – 4**.

| Ch | Name | Notes |
|----|------|-------|
| 0 | Dimmer | Master intensity |
| 1 | Strobe | Keep at **0.0** in most shows; existing effects use dimmer flashes instead of the strobe channel |
| 2 | Red | |
| 3 | Green | |
| 4 | Blue | |
| 5 | White | |
| 6 | Amber | |
| 7 | UV | |

**Role:** colour wash behind the stage / PixStrobe panel. Often run as a 4-step beat chase (`index` 0–3 per fixture) so one PAR is “hot” per beat.

**Typical levels:**

| Mood | Dimmer | Notes |
|------|--------|-------|
| Daytime accent | ~20 % | Amber/orange static (`fx_looking_glass`) |
| Evening backdrop | ~40 % | Deep purple (`fx_cheshire_grin`, `fx_white_rabbit`) |
| Club / drop | 85–100 % | Magenta, white hits on downbeats |

**Colour mixing:** effects use linear RGB in 0–1 space, e.g. deep purple ≈ `[0.70, 0.0, 1.0]` RGB with dimmer 0.40. Consider using the white and amber channels as well.

---

### PIXSTROBE 1 & 2 – Eurolite LED IP PIX Strobe RGB CW+WW

**Hardware:** Eurolite LED IP PIX Strobe RGB CW+WW  
**Universe:** 1  
**Addresses:** PixStrobe 1 @ 50, PixStrobe 2 @ 100  
**Channels:** 32 (8 WW + 8 CW blinder + 8×3 RGB pixels)

| Ch | Name | Notes |
|----|------|-------|
| 0–3 | WW 1–4 | Warm-white blinder segments (often unused; held at 0) |
| 4–7 | CW 1–4 | Cool-white blinder segments – **very bright** even at low values |
| 8–31 | RGB × 8 | Pixel segments |

#### Physical layout (one unit)

Each PixStrobe is a **2 × 4** pixel grid. Column 0 is the **left** edge of that unit.

```
col:           0        1        2        3
top  (row 0):  px1      px3      px5      px7
bot  (row 1):  px2      px4      px6      px8
```

- **Pixel index** (0–7): `col * 2 + row`
- **DMX base channel** for pixel `p`: `8 + p * 3` (R, G, B)

#### Stage placement

| Unit | Side | “Inside” edge (toward stage centre) |
|------|------|-------------------------------------|
| **PixStrobe 1** | Stage **left** | Column **3** (rightmost on unit) |
| **PixStrobe 2** | Stage **right** | Column **0** (leftmost on unit) |

Animations that “radiate from centre” use opposite ripple directions on the two units so pulses meet in the middle of the combined panel.

#### Brightness guidance

These fixtures are **extremely bright** and face the audience/dancefloor.

| Use | Typical pixel level | Blinder CW |
|-----|---------------------|------------|
| Ambient / evening | 15–25 % | Off or brief accents |
| Club | 40–50 % | Short peaks only |
| Drop / hit | Up to 100 % | ≤ 0.45 peak  |

`fx_prada_v2` deliberately cut blinders (~45 % drops, ~20 % accents) after a hardware look. **When in doubt, start dim.**


---

### QUADPHASE 1 & 2 – American DJ Quad Phase

**Hardware:** [American DJ Quad Phase](https://www.adj.eu/mwdownloads/download/link/id/447) (4-channel DMX, 10 W RGBW LED, 65° beam).  
**Manual:** ADJ Quad Phase Instruction Manual (Rev. 9/10) — DMX traits on page 13.  
**Universe:** 1  
**Addresses:** 140, 150  
**Channels:** 4 — `[Color, Rotation, Strobe, Shutter]`

| Ch | Name | DMX range | Behaviour |
|----|------|-----------|-----------|
| 1 | Color | See colour wheel table | 15 discrete colours (DMX 1–255; 0 unused) |
| 2 | Rotation | 0–9 | No rotation |
| | | 10–120 | Clockwise, fast → slow |
| | | 121–134 | No rotation |
| | | 135–245 | Counter-clockwise, slow → fast |
| | | 246–249 | No rotation |
| | | 250–255 | Sound active |
| 3 | Strobe | 0 | Off |
| | | 1–255 | Strobe slow → fast |
| 4 | Shutter | 0–15 | **Off** (beam blacked out) |
| | | 16–255 | **Open** |

**Normalised shorthand for shutter:** `0.0` = closed, `≥ 16/255` (≈ `0.063`) = open. Most FX themes use `1.0` for open and `0.0` for closed.

**Role:** fill the space in front of the stage with moving beams. Use fog/haze to make beams visible. Deep **blue** (slot 3) reads as atmosphere without lighting faces; **red** / split colours for high energy.

#### Colour wheel (verified order)

Slot 1 is **16** DMX values wide; slots 2–14 are **17** values each; slot 15 fills **238–255**. Use the **centre** of a band for a stable colour. Values below are `centre_dmx / 255` for effect code (`0.0..1.0`).

| Slot | DMX range | Colour | Normalised centre |
|------|-----------|--------|-------------------|
| 1 | 1–16 | Red | `8.5/255` ≈ **0.033** |
| 2 | 17–33 | Green | `25/255` ≈ **0.098** |
| 3 | 34–50 | Blue | `42/255` ≈ **0.165** |
| 4 | 51–67 | White | `59/255` ≈ **0.231** |
| 5 | 68–84 | Red + green | `76/255` ≈ **0.298** |
| 6 | 85–101 | Blue + red | `93/255` ≈ **0.365** |
| 7 | 102–118 | Red + white | `110/255` ≈ **0.431** |
| 8 | 119–135 | Blue + green | `127/255` ≈ **0.498** |
| 9 | 136–152 | Green + white | `144/255` ≈ **0.565** |
| 10 | 153–169 | Blue + white | `161/255` ≈ **0.631** |
| 11 | 170–186 | Blue + green + red | `178/255` ≈ **0.698** |
| 12 | 187–203 | White + green + red | `195/255` ≈ **0.765** |
| 13 | 204–220 | White + red + blue | `212/255` ≈ **0.831** |
| 14 | 221–237 | White + blue + green | `229/255` ≈ **0.898** |
| 15 | 238–255 | White + red + green + blue (full mix) | `246.5/255` ≈ **0.967** |



#### Rotation (effect authoring)

Stop / no rotation lives in DMX **121–134** (centre ≈ `127/255` ≈ **0.498**). Existing shows approximate this with `0.5`:

```rust
// fx_prada pattern — id 0 forward, id 1 reverse
rotation_ch = 0.5 + 0.5 * speed.clamp(-1.0, 1.0)
// 0.5 ≈ stop; 1.0 → DMX ~255 (sound-active zone — avoid!)
// Practical fast spin: ~0.45–0.47 (CW) or ~0.75–0.85 (CCW)
```

| Intent | Suggested normalised range | DMX zone |
|--------|---------------------------|----------|
| Stop | 0.47–0.52 | 121–134 |
| CW slow → fast | 0.04–0.47 | 10–120 |
| CCW slow → fast | 0.53–0.96 | 135–245 |
| **Avoid** | ≥ 0.98 | 250–255 = sound active |

**Daytime rule:** `fx_looking_glass` keeps QuadPhases **off** (shutter closed)—rotating beams look cheap in daylight.

---

### SCANNER 1 & 2 – Stairville SC-X50 MkII (11-channel mode)

**Universe:** 1  
**Addresses:** Scanner 1 @ 160, Scanner 2 @ 180  
**Manual reference:** Thomann `c_271625_v2_r2_de_online.pdf`

| Ch | Name | Range / behaviour |
|----|------|-------------------|
| 0 | Pan | 0–180° (8-bit) |
| 1 | Tilt | 0–60° (8-bit) |
| 2 | Color wheel | See colour table below |
| 3 | Shutter / strobe | 0–3 closed, 4–7 open, 8–215 strobe, 216–255 open |
| 4 | Dimmer | 0–100 % |
| 5 | Gobo wheel | See gobo table below |
| 6 | Gobo rotation | 0–63 off, 64–147 CW, 148–231 CCW |
| 7 | Prism | 0–3 off, 4–127 +rot, 128–251 −rot, 252–255 static |
| 8 | Focus | 0.5 ≈ mid (stage mount) |
| 9 | Functions | **Keep at 0** (avoid blackout-on-move) |
| 10 | Programs | **Keep at 0** (DMX control) |

Use `scanner_frame(...)` in `src/themes/mod.rs` for the 11-channel vector.

#### Mounting

| Unit | Position | Notes |
|------|----------|-------|
| **Scanner 1** | Stage **left** | Mirror pair; “inside” edge aims toward stage centre |
| **Scanner 2** | Stage **right** | Set `reverse: true` in effects to mirror pan presets |

Scanners are **ceiling-mounted** pointing down. Per `fx_prada_v2`: rapid shutter stabs look bad on ceiling mounts—prefer position moves, colour changes, and slow dimmer ramps.

#### Pan / tilt calibration (normalised 0–1)

Shared across most FX shows:

| Preset | Scanner 1 (left) | Scanner 2 (right) | Purpose |
|--------|------------------|-------------------|---------|
| Centre (own side) | 0.62–0.65 | 0.35–0.38 | Beat positions |
| Splay (outward) | 0.20–0.22 | 0.78–0.80 | Wide hit |
| Centre (both) | 0.50 | 0.50 | Cross-beam X |

| Tilt | Value | Purpose |
|------|-------|---------|
| Stage | 0.50 | Audience / dancefloor |
| Up | 0.80 | Ceiling / riser |

#### Colour wheel (SC_COLOR_* constants)

| Constant | DMX band | Colour |
|----------|----------|--------|
| `SC_COLOR_WHITE` | 0–6 | White |
| `SC_COLOR_YELLOW` | 7–13 | Yellow |
| `SC_COLOR_PINK` | 14–20 | Pink |
| `SC_COLOR_GREEN` | 21–27 | Green |
| `SC_COLOR_RED` | 28–34 | Red |
| `SC_COLOR_BLUE` | 35–41 | Blue |
| `SC_COLOR_KGREEN` | 42–48 | Kelly green |
| `SC_COLOR_SALMON` | 49–55 | Salmon |
| `SC_COLOR_DKBLUE` | 56–63 | Dark blue |
| `SC_COLOR_RAINBOW` | 128–191 | Slow rainbow spin |

Use centre-of-band values from `src/themes/mod.rs` (e.g. pink = 17/255).

#### Gobo wheel

| Constant | DMX band | Gobo |
|----------|----------|------|
| `SC_GOBO_OPEN` | 0–7 | Open |
| `SC_GOBO_1` … `SC_GOBO_7` | 8–63 | Indexed gobos |
| `SC_GOBO_SPIN` | 128–191 | Rainbow spin |

**Hardware note:** Gobo **rotation** (`SC_GOBOROT_MED_POS`, `SC_GOBOROT_FAST_POS`) is intentionally disabled in code—all map to `SC_GOBOROT_NONE` because the gear is creaky. Do not enable gobo spin unless the hardware is serviced.

#### Shutter constants

| Constant | Value | Effect |
|----------|-------|--------|
| `SC_SHUTTER_CLOSED` | 0.0 | Blackout |
| `SC_SHUTTER_OPEN` | 216/255 | Open |
| `SC_SHUTTER_STROBE_MED` | 120/255 | Medium strobe |

Typical stab window on downbeat: **0.06–0.10 s** (`SCAN_STAB_S`).

---

## Authoring checklist

### New Decke theme

1. Add `src/themes/your_theme.rs` implementing `Effect` for `LOUNGE_DECKE` and/or `BAR_DECKE`.
2. Register in `themes/mod.rs` → `all_themes()` and `theme_names()`.
3. Use `build_ceiling()` / `Rgbw` helpers; remember bar ceiling is 16-bit γ-corrected.
4. Consider time-of-day intent (work light vs evening drama).

### New FX theme

1. Add `src/themes/fx_your_theme.rs`.
2. Bind fixtures in `theme()` with `Binding::single(...)`.
3. Use `ctx.show_time` for any timed or musical content.
4. Assign per-fixture indices (`step`, `id`, `reverse`, `index`) for chases and mirroring.
5. Register in `all_fx_themes()` / `fx_theme_names()`.
6. Test brightness on **PixStrobes first**—they overpower the rig.

### New timecoded song show

1. Document BPM, first downbeat offset, bar map in the file header (see `fx_prada.rs`).
2. Key all timing off `ctx.show_time`.
3. Share section helpers (`musical()`, `section_for_bar()`, etc.) within the file.
4. Calibrate scanner pan/tilt against real room sightlines.

---
