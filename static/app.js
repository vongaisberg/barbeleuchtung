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
  const url   = `${proto}://${location.host}/ws`;
  ws          = new WebSocket(url);

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

let currentTheme   = 0;
let currentBlackout = false;

function applyState(state) {
  // Build theme buttons if not yet created.
  if (state.theme_names) buildThemeButtons(state.theme_names);
  // Build faders if not yet created.
  if (state.fader_labels) buildFaders(state.fader_labels, state.faders);

  // Update active theme highlight.
  currentTheme = state.theme;
  document.querySelectorAll('.theme-btn').forEach((btn, i) => {
    btn.classList.toggle('active', i === currentTheme);
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
