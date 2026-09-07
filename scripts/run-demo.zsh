#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="${0:A:h:h}"
CONNECTOR="$ROOT_DIR/target/release/orangedeck-connector"
UI="$ROOT_DIR/target/release/orangedeck-ui"
DEMO_LANGUAGE="${1:-ko}"
if [[ "$DEMO_LANGUAGE" != ko && "$DEMO_LANGUAGE" != en ]]; then
  print -u2 "Usage: ./scripts/run-demo.zsh [ko|en]"
  exit 1
fi

if [[ ! -x "$CONNECTOR" || ! -x "$UI" ]]; then
  print -u2 "Release binaries are missing. Run: cargo build --release --workspace"
  exit 1
fi

"$CONNECTOR" demo --port 45831 --language "$DEMO_LANGUAGE" &
CONNECTOR_PID=$!

cleanup() {
  kill "$CONNECTOR_PID" 2>/dev/null || true
  wait "$CONNECTOR_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

for attempt in {1..40}; do
  if curl --silent --fail --max-time 1 http://127.0.0.1:45831/readyz >/dev/null; then
    break
  fi
  if ! kill -0 "$CONNECTOR_PID" 2>/dev/null; then
    print -u2 "Mock Connector stopped before becoming ready"
    exit 1
  fi
  sleep 0.1
done

if ! curl --silent --fail --max-time 1 http://127.0.0.1:45831/readyz >/dev/null; then
  print -u2 "Mock Connector did not become ready on loopback port 45831"
  exit 1
fi

"$UI" demo --language "$DEMO_LANGUAGE"
