'use strict';

// ---------------------------------------------------------------------------
// WebSocket connection with exponential-backoff reconnect
// ---------------------------------------------------------------------------

const RECONNECT_BASE_MS = 500;
const RECONNECT_MAX_MS  = 16_000;

let ws          = null;
let reconnectMs = RECONNECT_BASE_MS;
let reconnectTimer = null;

function connect() {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  // Use relative path so it works when reverse proxied behind /licht
  // Ensure path always ends with /ws regardless of trailing slash
  let path = location.pathname;
  if (!path.endsWith('/')) {
    path += '/';
  }
  path += 'ws';
  const url = `${proto}://${location.host}${path}`;
  ws = new WebSocket(url);

  ws.addEventListener('open', () => {
    setStatus('Connected', 'connected');
    reconnectMs = RECONNECT_BASE_MS;
  });

  ws.addEventListener('message', (ev) => {
    try {
      const msg = JSON.parse(ev.data);
      if (msg.type === 'state') applyState(msg);
    } catch (e) {
      console.warn('Bad WS message:', ev.data, e);
    }
  });

  ws.addEventListener('close', scheduleReconnect);
  ws.addEventListener('error', scheduleReconnect);
}

function scheduleReconnect() {
  if (reconnectTimer) return;
  setStatus(`Reconnecting in ${(reconnectMs / 1000).toFixed(1)} s…`, 'disconnected');
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    reconnectMs = Math.min(reconnectMs * 2, RECONNECT_MAX_MS);
    connect();
  }, reconnectMs);
}

function send(obj) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify(obj));
  }
}

// ---------------------------------------------------------------------------
// State application
// ---------------------------------------------------------------------------

let currentTheme    = 0;
let currentFxTheme  = 0;
let currentBlackout = false;
let currentFogEnabled = false;

function applyState(state) {
  // Build theme buttons if not yet created.
  if (state.theme_names) buildThemeButtons(state.theme_names);
  // Build FX theme buttons if not yet created.
  if (state.fx_theme_names) buildFxThemeButtons(state.fx_theme_names);
  // Build faders if not yet created.
  if (state.fader_labels) buildFaders(state.fader_labels, state.faders);

  // Update active Decke theme highlight.
  currentTheme = state.theme;
  document.querySelectorAll('#theme-grid .theme-btn').forEach((btn, i) => {
    btn.classList.toggle('active', i === currentTheme);
  });

  // Update active FX theme highlight.
  currentFxTheme = state.fx_theme;
  document.querySelectorAll('#fx-theme-grid .theme-btn').forEach((btn, i) => {
    btn.classList.toggle('active', i === currentFxTheme);
  });

  // Update fader values (without re-triggering input events).
  state.faders.forEach((val, i) => {
    const el    = document.getElementById(`fader-${i}`);
    const label = document.getElementById(`fader-val-${i}`);
    if (el) {
      if (el.tagName === 'INPUT') {
        if (!el._dragging) el.value = val;
      } else {
        // Toggle button (fan or traffic-light bulb).
        const on = val >= 0.5;
        el.dataset.value = on ? '1' : '0';
        el.classList.toggle('active', on);
      }
    }
    if (label) label.textContent = Math.round(val * 100) + '%';
  });

  // Update blackout button.
  currentBlackout = state.blackout;
  const bo = document.getElementById('blackout-btn');
  if (bo) bo.classList.toggle('active', currentBlackout);

  // Update fog machine controls (skip while user is dragging a slider).
  if (state.fog_enabled !== undefined) {
    currentFogEnabled = state.fog_enabled;
    const fogBtn = document.getElementById('fog-btn');
    if (fogBtn) {
      fogBtn.classList.toggle('active', currentFogEnabled);
      fogBtn.textContent = currentFogEnabled ? 'FOG: ON' : 'FOG: OFF';
    }
    setFogSlider('fog-interval', 'fog-interval-val', state.fog_interval_min,
                 v => v.toFixed(1) + ' min');
    setFogSlider('fog-duration',  'fog-duration-val',  state.fog_duration_s,
                 v => Math.round(v) + ' s');
    setFogSlider('fog-level',     'fog-level-val',     state.fog_level * 100,
                 v => Math.round(v) + ' %');
  }

  if (state.universes) applyUniverses(state.universes);
}

function setFogSlider(sliderId, labelId, value, fmt) {
  const slider = document.getElementById(sliderId);
  const label  = document.getElementById(labelId);
  if (slider && !slider._dragging) slider.value = value;
  if (label) label.textContent = fmt(value);
}

// ---------------------------------------------------------------------------
// Slider drag tracking
//
// Vertical (and small-thumb) <input type="range"> elements regularly lose
// pointer capture mid-drag on mobile because the thumb moves out from under
// the finger. The browser then fires pointerup even though the touch is
// still down, our `_dragging` flag flips back to false, and the next state
// echo from the server snaps the slider back to its last value – making the
// slider look frozen after just a few ms of drag.
//
// This helper fixes that by:
//   1. Calling setPointerCapture on pointerdown so the slider element owns
//      the gesture for its entire lifetime, regardless of thumb position.
//   2. Handling pointercancel / lostpointercapture as well as pointerup, so
//      `_dragging` can't get stuck or be cleared prematurely.
//   3. Clearing `_dragging` only after a short grace period, so a late echo
//      from the server arriving 1-2 frames after release can't snap the
//      slider back to its last-sent value.
// ---------------------------------------------------------------------------
const DRAG_RELEASE_GRACE_MS = 150;

function attachDragTracking(el) {
  el._dragging = false;
  let releaseTimer = null;

  const begin = (e) => {
    if (releaseTimer) {
      clearTimeout(releaseTimer);
      releaseTimer = null;
    }
    el._dragging = true;
    if (e.pointerId !== undefined && el.setPointerCapture) {
      try { el.setPointerCapture(e.pointerId); } catch (_) { /* ignore */ }
    }
  };

  const end = () => {
    if (releaseTimer) clearTimeout(releaseTimer);
    releaseTimer = setTimeout(() => {
      el._dragging = false;
      releaseTimer = null;
    }, DRAG_RELEASE_GRACE_MS);
  };

  el.addEventListener('pointerdown',        begin);
  el.addEventListener('pointerup',          end);
  el.addEventListener('pointercancel',      end);
  el.addEventListener('lostpointercapture', end);
}

function sendFogSettings() {
  const interval = parseFloat(document.getElementById('fog-interval').value);
  const duration = parseFloat(document.getElementById('fog-duration').value);
  const level    = parseFloat(document.getElementById('fog-level').value) / 100;
  send({ type: 'fog_settings', interval_min: interval, duration_s: duration, level });
}

// ---------------------------------------------------------------------------
// Dynamic UI construction
// ---------------------------------------------------------------------------

let themesBuilt = false;
function buildThemeButtons(names) {
  if (themesBuilt) return;
  themesBuilt = true;
  const grid = document.getElementById('theme-grid');
  grid.innerHTML = '';
  names.forEach((name, i) => {
    const btn = document.createElement('button');
    btn.className = 'theme-btn';
    btn.textContent = name;
    btn.addEventListener('click', () => {
      send({ type: 'theme', id: i });
    });
    grid.appendChild(btn);
  });
}

let fxThemesBuilt = false;
function buildFxThemeButtons(names) {
  if (fxThemesBuilt) return;
  fxThemesBuilt = true;
  const grid = document.getElementById('fx-theme-grid');
  grid.innerHTML = '';
  names.forEach((name, i) => {
    const btn = document.createElement('button');
    btn.className = 'theme-btn';
    btn.textContent = name;
    btn.addEventListener('click', () => {
      send({ type: 'fx_theme', id: i });
    });
    grid.appendChild(btn);
  });
}

// Faders are sent to the backend as continuous values, but rendered in the UI
// according to the fixture role inferred from its label.
const FAN_FADER_LABELS   = new Set(['Zuluft', 'Bar', 'Gang']);
const TRAFFIC_RED_LABEL   = 'Traffic Red';
const TRAFFIC_GREEN_LABEL = 'Traffic Green';

let fadersBuilt = false;
function buildFaders(labels, initialValues) {
  if (fadersBuilt) return;
  fadersBuilt = true;
  const row = document.getElementById('fader-row');
  row.innerHTML = '';

  const sliders = [];
  const fans    = [];
  let trafficRed   = null;
  let trafficGreen = null;

  labels.forEach((label, i) => {
    const val = initialValues ? initialValues[i] : 0;
    const entry = { label, i, val };
    if (FAN_FADER_LABELS.has(label)) {
      fans.push(entry);
    } else if (label === TRAFFIC_RED_LABEL) {
      trafficRed = entry;
    } else if (label === TRAFFIC_GREEN_LABEL) {
      trafficGreen = entry;
    } else {
      sliders.push(entry);
    }
  });

  sliders.forEach(e => row.appendChild(buildSliderColumn(e)));
  if (fans.length > 0) row.appendChild(buildFanGroup(fans));
  if (trafficRed || trafficGreen) row.appendChild(buildTrafficLight(trafficRed, trafficGreen));
}

function buildSliderColumn({ label, i, val }) {
  const col = document.createElement('div');
  col.className = 'fader-col';

  const nameLabel = document.createElement('span');
  nameLabel.className = 'fader-label';
  nameLabel.textContent = label;

  const trackDiv = document.createElement('div');
  trackDiv.className = 'fader-track';

  const slider = document.createElement('input');
  slider.type      = 'range';
  slider.className = 'fader-input';
  slider.min       = '0';
  slider.max       = '1';
  slider.step      = '0.01';
  slider.value     = val;
  slider.id        = `fader-${i}`;

  const valLabel = document.createElement('span');
  valLabel.className   = 'fader-value';
  valLabel.id          = `fader-val-${i}`;
  valLabel.textContent = Math.round(val * 100) + '%';

  attachDragTracking(slider);
  slider.addEventListener('input', () => {
    const v = parseFloat(slider.value);
    valLabel.textContent = Math.round(v * 100) + '%';
    send({ type: 'fader', id: i, value: v });
  });

  trackDiv.appendChild(slider);
  col.appendChild(nameLabel);
  col.appendChild(trackDiv);
  col.appendChild(valLabel);
  return col;
}

function buildFanGroup(entries) {
  const col = document.createElement('div');
  col.className = 'fader-group';

  const header = document.createElement('span');
  header.className = 'fader-label';
  header.textContent = 'Fans';

  const stack = document.createElement('div');
  stack.className = 'fan-stack';

  entries.forEach(({ label, i, val }) => {
    const on = val >= 0.5;
    const btn = document.createElement('button');
    btn.className     = 'fan-toggle';
    btn.id            = `fader-${i}`;
    btn.dataset.value = on ? '1' : '0';
    btn.classList.toggle('active', on);
    btn.textContent   = label;
    btn.addEventListener('click', () => {
      const newOn = btn.dataset.value !== '1';
      const newVal = newOn ? 1 : 0;
      btn.dataset.value = newOn ? '1' : '0';
      btn.classList.toggle('active', newOn);
      send({ type: 'fader', id: i, value: newVal });
    });
    stack.appendChild(btn);
  });

  col.appendChild(header);
  col.appendChild(stack);
  return col;
}

function buildTrafficLight(red, green) {
  const col = document.createElement('div');
  col.className = 'fader-group';

  const header = document.createElement('span');
  header.className = 'fader-label';
  header.textContent = 'Ampel';

  const housing = document.createElement('div');
  housing.className = 'traffic-light';

  // Red bulb on top, green on bottom: pressing one turns the other off so
  // only one bulb can be lit at a time.
  const redBtn   = red   ? makeTrafficBulb(red,   'red',   () => green && setBulbOff(greenBtn, green.i)) : null;
  const greenBtn = green ? makeTrafficBulb(green, 'green', () => red   && setBulbOff(redBtn,   red.i))   : null;
  if (redBtn)   housing.appendChild(redBtn);
  if (greenBtn) housing.appendChild(greenBtn);

  col.appendChild(header);
  col.appendChild(housing);
  return col;
}

function makeTrafficBulb({ label, i, val }, color, onActivate) {
  const on = val >= 0.5;
  const btn = document.createElement('button');
  btn.className     = `traffic-bulb ${color}`;
  btn.id            = `fader-${i}`;
  btn.dataset.value = on ? '1' : '0';
  btn.classList.toggle('active', on);
  btn.setAttribute('aria-label', label);
  btn.addEventListener('click', () => {
    const newOn = btn.dataset.value !== '1';
    const newVal = newOn ? 1 : 0;
    btn.dataset.value = newOn ? '1' : '0';
    btn.classList.toggle('active', newOn);
    send({ type: 'fader', id: i, value: newVal });
    if (newOn) onActivate();
  });
  return btn;
}

function setBulbOff(btn, i) {
  if (!btn || btn.dataset.value !== '1') return;
  btn.dataset.value = '0';
  btn.classList.remove('active');
  send({ type: 'fader', id: i, value: 0 });
}

// ---------------------------------------------------------------------------
// Blackout button
// ---------------------------------------------------------------------------

document.getElementById('blackout-btn').addEventListener('click', () => {
  currentBlackout = !currentBlackout;
  send({ type: 'blackout', active: currentBlackout });
});

// ---------------------------------------------------------------------------
// Fog machine controls
// ---------------------------------------------------------------------------

document.getElementById('fog-btn').addEventListener('click', () => {
  currentFogEnabled = !currentFogEnabled;
  send({ type: 'fog_enabled', active: currentFogEnabled });
});

['fog-interval', 'fog-duration', 'fog-level'].forEach(id => {
  const el = document.getElementById(id);
  attachDragTracking(el);
  el.addEventListener('input', () => {
    // Update local label immediately for responsive feel.
    const val = parseFloat(el.value);
    const labelId = id + '-val';
    const label = document.getElementById(labelId);
    if (label) {
      if (id === 'fog-interval') label.textContent = val.toFixed(1) + ' min';
      if (id === 'fog-duration') label.textContent = Math.round(val) + ' s';
      if (id === 'fog-level')    label.textContent = Math.round(val) + ' %';
    }
    sendFogSettings();
  });
});

// ---------------------------------------------------------------------------
// Status bar helper
//
// The status bar contains a connection-state label on the left and the
// universe-mute pill row on the right. Only the connection label is rewritten
// by setStatus(); the pills are managed by applyUniverses() below so they
// survive every state push.
// ---------------------------------------------------------------------------

function setStatus(text, cls) {
  const bar  = document.getElementById('status-bar');
  const conn = document.getElementById('status-bar-conn');
  if (conn) conn.textContent = text;
  bar.className = 'status-bar ' + (cls || '');
}

// ---------------------------------------------------------------------------
// Universe-mute pills
//
// One small clickable pill per patched DMX universe, rendered in the status
// bar. Click toggles whether the engine sends ArtDmx for that universe at all
// (distinct from blackout, which still transmits all-zero frames).
//
// On every click we also briefly flash the universe's label in the status
// text — this is the "label tooltip" for touch devices where hovering the
// pill's `title` attribute isn't available.
// ---------------------------------------------------------------------------

let universesBuilt    = false;
let baseStatusText    = 'Connected';
let baseStatusClass   = 'connected';
let pillFlashTimer    = null;

function applyUniverses(universes) {
  const row = document.getElementById('universe-pills');
  if (!row) return;

  if (!universesBuilt) {
    universesBuilt = true;
    row.innerHTML = '';
    universes.forEach((u) => {
      const pill = document.createElement('button');
      pill.type            = 'button';
      pill.className       = 'universe-pill';
      pill.dataset.universe = String(u.universe);
      pill.dataset.label    = u.label;
      pill.textContent     = `U${u.universe}`;
      pill.title           = `${u.label} (universe ${u.universe})`;
      pill.setAttribute('aria-label', pill.title);
      pill.addEventListener('click', () => {
        const muted = pill.classList.contains('muted');
        // Optimistic update so the UI feels instant; the server echo will
        // reaffirm the same state on the next snapshot.
        pill.classList.toggle('muted', !muted);
        flashPillLabel(pill, !muted);
        send({
          type: 'universe_output',
          universe: parseInt(pill.dataset.universe, 10),
          muted: !muted,
        });
      });
      row.appendChild(pill);
    });
  }

  // Refresh per-pill state from the snapshot.
  universes.forEach((u) => {
    const pill = row.querySelector(`.universe-pill[data-universe="${u.universe}"]`);
    if (pill) pill.classList.toggle('muted', !!u.muted);
  });

  // Reflect the aggregate state in the status bar (and remember it so the
  // pill-flash transient can restore it afterwards).
  const mutedCount = universes.filter((u) => u.muted).length;
  if (mutedCount > 0) {
    baseStatusText  = `Connected · ${mutedCount} universe${mutedCount === 1 ? '' : 's'} muted`;
    baseStatusClass = 'connected warning';
  } else {
    baseStatusText  = 'Connected';
    baseStatusClass = 'connected';
  }
  if (!pillFlashTimer) setStatus(baseStatusText, baseStatusClass);
}

function flashPillLabel(pill, nowMuted) {
  const text = `${pill.dataset.label} (U${pill.dataset.universe}) — ${nowMuted ? 'muted' : 'sending'}`;
  setStatus(text, nowMuted ? 'connected warning' : 'connected');
  if (pillFlashTimer) clearTimeout(pillFlashTimer);
  pillFlashTimer = setTimeout(() => {
    pillFlashTimer = null;
    setStatus(baseStatusText, baseStatusClass);
  }, 1600);
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
connect();
