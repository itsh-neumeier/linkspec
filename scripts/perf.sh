#!/usr/bin/env bash
# Measure LinkSpec performance targets (docs/PERFORMANCE.md).
# Linux: needs /usr/bin/time; for the app an X server (Xvfb is enough).
# Usage: scripts/perf.sh [fixture]
set -euo pipefail
FIX="${1:-fixtures/core/c10_dock_usb3_and_dp.json}"
cargo build --release -q -p linkspec-cli -p linkspec-app
BIN=target/release
echo "== CLI cold start (10 runs, fake backend)"
for i in $(seq 10); do /usr/bin/time -f "%e s  %M KB" "$BIN/linkspec" --fake "$FIX" --no-history >/dev/null; done 2>&1 | sort | head -10
echo "== CLI against the real platform"
/usr/bin/time -f "%e s  %M KB" "$BIN/linkspec" --json --no-history >/dev/null || true
echo "== Binary sizes"
ls -la "$BIN/linkspec" "$BIN/linkspec-app" | awk '{print $5, $9}'
if [ -n "${DISPLAY:-}" ]; then
  echo "== App: time to first frame and idle RAM/CPU"
  start=$(date +%s.%N)
  LINKSPEC_PERF_EXIT_AFTER_FIRST_FRAME=1 "$BIN/linkspec-app" --window --fake "$FIX"
  end=$(date +%s.%N)
  echo "first frame: $(echo "$end - $start" | bc) s"
  "$BIN/linkspec-app" --window --fake "$FIX" & pid=$!
  sleep 10
  ps -o rss=,pcpu= -p "$pid" | awk '{printf "idle RSS: %.1f MB, CPU: %s %%\n", $1/1024, $2}'
  kill "$pid"
fi
