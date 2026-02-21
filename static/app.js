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
    const slider = document.getElementById(`fader-${i}`);
    const label  = document.getElementById(`fader-val-${i}`);
    if (slider && !slider._dragging) {
      slider.value = val;
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
}

function setFogSlider(sliderId, labelId, value, fmt) {
  const slider = document.getElementById(sliderId);
  const label  = document.getElementById(labelId);
  if (slider && !slider._dragging) slider.value = value;
  if (label) label.textContent = fmt(value);
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

let fadersBuilt = false;
function buildFaders(labels, initialValues) {
  if (fadersBuilt) return;
  fadersBuilt = true;
  const row = document.getElementById('fader-row');
  row.innerHTML = '';
  labels.forEach((label, i) => {
    const initVal = initialValues ? initialValues[i] : 0;

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
    slider.value     = initVal;
    slider.id        = `fader-${i}`;
    slider._dragging = false;

    const valLabel = document.createElement('span');
    valLabel.className   = 'fader-value';
    valLabel.id          = `fader-val-${i}`;
    valLabel.textContent = Math.round(initVal * 100) + '%';

    slider.addEventListener('pointerdown', () => { slider._dragging = true; });
    slider.addEventListener('pointerup',   () => { slider._dragging = false; });
    slider.addEventListener('touchmove',   (e) => { e.preventDefault(); }, { passive: false });
    slider.addEventListener('input', () => {
      const val = parseFloat(slider.value);
      valLabel.textContent = Math.round(val * 100) + '%';
      send({ type: 'fader', id: i, value: val });
    });

    trackDiv.appendChild(slider);
    col.appendChild(nameLabel);
    col.appendChild(trackDiv);
    col.appendChild(valLabel);
    row.appendChild(col);
  });
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
  el._dragging = false;
  el.addEventListener('pointerdown', () => { el._dragging = true; });
  el.addEventListener('pointerup',   () => { el._dragging = false; });
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
// ---------------------------------------------------------------------------

function setStatus(text, cls) {
  const bar = document.getElementById('status-bar');
  bar.textContent = text;
  bar.className = 'status-bar ' + (cls || '');
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
connect();
