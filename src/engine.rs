use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Local;

use crate::artnet::{ArtNetSender, SubscriberTable};
use crate::effect::TickContext;
use crate::fixture::{render_fixture, UniverseBuffer};
use crate::fixtures;
use crate::scheduler::Scheduler;
use crate::state::AppState;
use crate::state::ReactiveLive;
use crate::theme::{all_fx_themes, all_reactive_themes, all_themes, Binding};

/// Target tick rate in Hz.
pub const TICK_RATE_HZ: u64 = 40;
const TICK_DURATION: Duration = Duration::from_micros(1_000_000 / TICK_RATE_HZ);

/// Number of Art-Net universes to maintain buffers for.
///
/// Derived from `fixtures::PATCHED_UNIVERSES` at compile time as
/// `max_universe + 1`, so adding a fixture on a new universe number only
/// requires updating that one table. The engine, the `AppState` mute vector,
/// and the web snapshot all stay in sync automatically.
pub const NUM_UNIVERSES: usize = {
    let mut max = 0usize;
    let mut i = 0;
    while i < fixtures::PATCHED_UNIVERSES.len() {
        let u = fixtures::PATCHED_UNIVERSES[i].0 as usize;
        if u > max {
            max = u;
        }
        i += 1;
    }
    max + 1
};

/// Run the engine loop forever in the calling thread.
///
/// `state` is shared with the web server; `scheduler` is optional (pass
/// `None` to disable time-of-day switching). `socket` and `subscribers` are
/// shared with the Art-Net discovery thread for unicast transmission.
pub fn run(
    state: Arc<Mutex<AppState>>,
    scheduler: Option<Scheduler>,
    socket: std::net::UdpSocket,
    subscribers: Arc<std::sync::RwLock<SubscriberTable>>,
    audio: crate::audio::FeatureHandle,
    frame: crate::viz::FrameHandle,
    reactive_live: Arc<ReactiveLive>,
) {
    let mut sender = match ArtNetSender::new(socket, subscribers) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to create Art-Net sender: {e}");
            return;
        }
    };

    // Pre-allocate all universe buffers.
    let mut universe_buffers: Vec<Box<UniverseBuffer>> =
        (0..NUM_UNIVERSES).map(|_| Box::new([0u8; 512])).collect();

    let start = Instant::now();
    let mut tick: u64 = 0;
    let mut prev_wall = Local::now().time();

    // Stateful generative Director for the reactive bank's Auto mode.
    let mut director = crate::themes::reactive::Director::new();

    // Build the full theme list once. Effects are rebuilt each tick by calling
    // `all_themes()` freshly – this re-creates effect objects but is cheap
    // since all themes are just small Rust structs on the stack.
    // We keep two theme lists so we can always evaluate "from" and "to"
    // simultaneously during a crossfade.

    loop {
        let tick_start = Instant::now();

        let elapsed = start.elapsed().as_secs_f64();
        let dt = TICK_DURATION.as_secs_f64();
        let wall = Local::now().time();

        // Latest audio analysis: one lock-free atomic load, shared into the
        // context for the reactive bank (cheap Arc clone per fixture).
        let audio_now = audio.load_full();

        // --- Read & advance shared state (lock held as briefly as possible) ---
        let (active_id, crossfade_snapshot, active_fx_id, fx_crossfade_snapshot, fx_show_time, fader_values, blackout,
             fog_enabled, fog_interval_min, fog_duration_s, fog_level, universe_muted,
             active_reactive_id, reactive_crossfade_snapshot, reactive_controls, reactive_auto) = {
            let mut s = state.lock().unwrap();

            // Advance crossfade (marks it complete when done).
            s.tick_crossfade();

            // Run time-of-day ^ler.
            if let Some(sched) = &scheduler {
                sched.tick(prev_wall, wall, &mut s);
            }

            let cf = s.crossfade.clone();
            let fx_cf = s.fx_crossfade.clone();
            // FX show clock: a Spotify-driven override wins when present so the
            // timecoded show tracks the song's `progress_ms`; otherwise fall
            // back to wall time since the scene was selected.
            let fx_show = match &s.fx_show_clock {
                Some(clock) => clock.now(),
                None => s.fx_theme_started_at.elapsed().as_secs_f64(),
            };
            let faders = s.fader_values.clone();
            let bo = s.blackout;
            let id = s.active_theme_id;
            let fx_id = s.active_fx_theme_id;
            let fog = (s.fog_enabled, s.fog_interval_min, s.fog_duration_s, s.fog_level);
            let muted = s.universe_muted.clone();
            let reactive_id = s.active_reactive_theme_id;
            let reactive_cf = s.reactive_crossfade.clone();
            let controls = s.reactive_controls;
            let auto = s.reactive_auto;
            (id, cf, fx_id, fx_cf, fx_show, faders, bo, fog.0, fog.1, fog.2, fog.3, muted,
             reactive_id, reactive_cf, controls, auto)
        };

        // Run the generative Director (Auto mode) from the live audio.
        let show = director.update(&audio_now, &reactive_controls, elapsed);
        reactive_live.store(reactive_auto, show.tier, show.palette_idx);

        // Build the base context now that we have the live controls. Slot is
        // overridden per-fixture inside group bindings.
        let ctx = TickContext {
            tick,
            time: elapsed,
            wall_clock: wall,
            dt,
            slot: 0,
            show_time: elapsed,
            audio: audio_now,
            controls: reactive_controls,
            show,
        };

        // FX bank gets a context whose `show_time` is measured from the moment
        // the active FX scene was selected, so timecoded shows play in sync.
        let fx_ctx = TickContext { show_time: fx_show_time, ..ctx.clone() };

        // --- Zero all universe buffers each tick ---
        for buf in universe_buffers.iter_mut() {
            buf.fill(0);
        }

        if !blackout {
            // --- Decke bank ---
            if let Some(cf) = crossfade_snapshot {
                let blend = cf.factor();
                let from_themes = all_themes();
                let to_themes = all_themes();

                if let Some(from_theme) = from_themes.into_iter().nth(cf.from_theme_id) {
                    render_theme_into(&from_theme.bindings, &ctx, &mut universe_buffers, 1.0 - blend);
                }
                if let Some(to_theme) = to_themes.into_iter().nth(cf.to_theme_id) {
                    render_theme_into(&to_theme.bindings, &ctx, &mut universe_buffers, blend);
                }
            } else {
                let mut themes = all_themes();
                if active_id < themes.len() {
                    let theme = themes.swap_remove(active_id);
                    render_theme_into(&theme.bindings, &ctx, &mut universe_buffers, 1.0);
                }
            }

            // --- FX bank ---
            if let Some(cf) = fx_crossfade_snapshot {
                let blend = cf.factor();
                let from_themes = all_fx_themes();
                let to_themes = all_fx_themes();

                if let Some(from_theme) = from_themes.into_iter().nth(cf.from_theme_id) {
                    render_theme_into(&from_theme.bindings, &fx_ctx, &mut universe_buffers, 1.0 - blend);
                }
                if let Some(to_theme) = to_themes.into_iter().nth(cf.to_theme_id) {
                    render_theme_into(&to_theme.bindings, &fx_ctx, &mut universe_buffers, blend);
                }
            } else {
                let mut fx_themes = all_fx_themes();
                if active_fx_id < fx_themes.len() {
                    let fx_theme = fx_themes.swap_remove(active_fx_id);
                    render_theme_into(&fx_theme.bindings, &fx_ctx, &mut universe_buffers, 1.0);
                }
            }

            // --- Reactive bank ---
            // Generative, audio-reactive scenes driven by `ctx.audio` (live
            // analysis). Uses the base `ctx`; reactivity comes from the audio
            // snapshot, not `show_time`. Renders additively like the FX bank.
            // The operator's master brightness scales the whole bank.
            let master = reactive_controls.master.clamp(0.0, 1.0);
            if reactive_auto {
                // Auto = the generative engine (Director-driven), one theme.
                let gen_theme = crate::themes::reactive::generative();
                render_theme_into(&gen_theme.bindings, &ctx, &mut universe_buffers, master);
            } else if let Some(cf) = reactive_crossfade_snapshot {
                let blend = cf.factor();
                let from_themes = all_reactive_themes();
                let to_themes = all_reactive_themes();

                if let Some(from_theme) = from_themes.into_iter().nth(cf.from_theme_id) {
                    render_theme_into(&from_theme.bindings, &ctx, &mut universe_buffers, (1.0 - blend) * master);
                }
                if let Some(to_theme) = to_themes.into_iter().nth(cf.to_theme_id) {
                    render_theme_into(&to_theme.bindings, &ctx, &mut universe_buffers, blend * master);
                }
            } else {
                let mut reactive_themes = all_reactive_themes();
                if active_reactive_id < reactive_themes.len() {
                    let reactive_theme = reactive_themes.swap_remove(active_reactive_id);
                    render_theme_into(&reactive_theme.bindings, &ctx, &mut universe_buffers, master);
                }
            }

            // --- Independent faders ---
            for (fader_idx, &fixture) in fixtures::FADER_FIXTURES.iter().enumerate() {
                let value = fader_values.get(fader_idx).copied().unwrap_or(0.0);
                let values: Vec<f32> = vec![value; fixture.channel_count()];
                let uni = fixture.universe as usize;
                if uni < NUM_UNIVERSES {
                    render_fixture(fixture, &values, universe_buffers[uni].as_mut());
                }
            }

            // --- Fog machine ---
            // Fires periodically: every `fog_interval_min` minutes, runs for
            // `fog_duration_s` seconds at `fog_level`.  Computed purely from
            // elapsed time – no extra state required.
            let fog_output = if fog_enabled {
                let interval_s = fog_interval_min as f64 * 60.0;
                let t_cycle = elapsed.rem_euclid(interval_s);
                if t_cycle < fog_duration_s as f64 { fog_level } else { 0.0 }
            } else {
                0.0
            };
            let fog_uni = fixtures::FOG_MACHINE.universe as usize;
            if fog_uni < NUM_UNIVERSES {
                render_fixture(
                    &fixtures::FOG_MACHINE,
                    &[fog_output],
                    universe_buffers[fog_uni].as_mut(),
                );
            }
        }

        // --- Publish the rendered frame for the live visualization ---
        // A cheap copy of the buffers, swapped in lock-free so the web layer
        // always reads a complete, consistent frame.
        let snapshot: Vec<[u8; 512]> =
            universe_buffers.iter().map(|b| **b).collect();
        frame.store(Arc::new(snapshot));

        // --- Send all universes over Art-Net (skipping muted ones) ---
        // A muted universe gets no ArtDmx frame at all this tick; fixtures
        // patched on it will see signal loss and may fall back to standalone
        // behaviour.  Compare to `blackout`, which still transmits a frame
        // full of zeros.
        for (uni_idx, buf) in universe_buffers.iter().enumerate() {
            if universe_muted.get(uni_idx).copied().unwrap_or(false) {
                continue;
            }
            if let Err(e) = sender.send_universe(uni_idx as u16, buf.as_ref()) {
                log::warn!("Art-Net send error on universe {uni_idx}: {e}");
            }
        }

        tick += 1;
        prev_wall = wall;

        // Sleep for the remainder of the tick period.
        let elapsed_this_tick = tick_start.elapsed();
        if let Some(sleep_time) = TICK_DURATION.checked_sub(elapsed_this_tick) {
            std::thread::sleep(sleep_time);
        }
    }
}

/// Evaluate every binding in a theme and *add* the (scaled) result into the
/// universe buffers. Using additive blending lets us blend two themes by
/// calling this twice with complementary `scale` values.
fn render_theme_into(
    bindings: &[Binding],
    ctx: &TickContext,
    universe_buffers: &mut [Box<UniverseBuffer>],
    scale: f32,
) {
    for binding in bindings {
        match binding {
            Binding::Single(b) => {
                render_one(b.fixture, b.effect.tick(ctx), ctx, scale, universe_buffers);
            }
            Binding::Group(g) => {
                for (index, &fixture) in g.fixtures.iter().enumerate() {

                    let slot_ctx = TickContext {
                        slot: index,
                        ..ctx.clone()
                    };
                    render_one(fixture, g.effect.tick(&slot_ctx), &slot_ctx, scale, universe_buffers);
                }
            }
        }
    }
}

/// Write one fixture's channel values (already produced by an effect) into
/// the universe buffers with saturating addition.
fn render_one(
    fixture: &crate::fixture::Fixture,
    raw: Vec<f32>,
    _ctx: &TickContext,
    scale: f32,
    universe_buffers: &mut [Box<UniverseBuffer>],
) {
    let uni = fixture.universe as usize;
    if uni >= NUM_UNIVERSES {
        return;
    }
    let scaled: Vec<f32> = raw.iter().map(|v| v.clamp(0.0, 1.0) * scale).collect();
    let mut tmp = [0u8; 512];
    render_fixture(fixture, &scaled, &mut tmp);
    for (dst, src) in universe_buffers[uni].iter_mut().zip(tmp.iter()) {
        *dst = dst.saturating_add(*src);
    }
}
