#!/usr/bin/env bash
#
# Launch barbeleuchtung with live audio captured from the DEFAULT SINK'S
# MONITOR — a passive tap on whatever is already playing.
#
# Why this and not `--features loopback`:
#   The cpal `loopback` path opens the default *input device* (a microphone).
#   On Bluetooth that forces the headset out of high-quality A2DP into the
#   HFP "phone call" profile (16 kHz mono) — wrecking playback. A sink monitor
#   never opens a mic, so output quality is untouched.
#
# Usage:
#   ./run-reactive.sh                 # release build, auto-detect default sink
#   RATE=44100 ./run-reactive.sh      # override capture rate
#   SINK=<name> ./run-reactive.sh     # capture a specific sink's monitor
#
set -uo pipefail

# load .env file
if [ -f .env ]; then
  set -o allexport
  source .env
  set +o allexport
fi

# Pick the sink to tap: explicit $SINK, else the current default sink.
SINK="${SINK:-$(pactl get-default-sink)}"
MON="${SINK}.monitor"
RATE="${RATE:-48000}"   # 48000 matches Bluetooth A2DP natively (no resample)

echo "barbeleuchtung reactive launcher"
echo "  capturing monitor : $MON"
echo "  rate / channels   : ${RATE} Hz / 2"
echo

# Sanity: make sure the monitor source actually exists.
if ! pactl list short sources | grep -q "[[:space:]]${MON}[[:space:]]"; then
  echo "WARNING: monitor source '$MON' not found in:"
  pactl list short sources | awk '{print "    "$2}'
  echo "Set SINK=<one of the *.monitor names minus .monitor> and retry."
  echo
fi

cargo build --release || exit 1

# parec taps the monitor (no mic → no Bluetooth profile switch); the program
# reads raw PCM from stdin. Env vars apply to the binary (last pipe stage).
exec parec -d "$MON" --format=float32le --rate="$RATE" --channels=2 \
  | BB_PCM_STDIN=1 BB_PCM_RATE="$RATE" BB_PCM_CHANNELS=2 BB_PCM_FORMAT=f32 \
    ./target/release/barbeleuchtung
