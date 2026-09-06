#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="${0:A:h:h}"
UI="$ROOT_DIR/target/release/orangedeck-ui"
CONFIG="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/config.toml}"
ORANGEDECK_INSTALLED="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/start-ui.sh"
if (( $# == 0 )) && [[ -x "$ORANGEDECK_INSTALLED" ]]; then
  exec "$ORANGEDECK_INSTALLED"
fi

if [[ ! -x "$UI" ]]; then
  print -u2 "Release UI is missing. Run: cargo build --release -p orangedeck-ui"
  exit 1
fi

if [[ ! -f "$CONFIG" ]]; then
  print -u2 "Pairing is missing. Import the Mac pairing bundle with:"
  print -u2 "\"$UI\" pair --bundle /path/to/orangedeck-pairing.toml"
  exit 1
fi

exec "$UI" run --config "$CONFIG"
