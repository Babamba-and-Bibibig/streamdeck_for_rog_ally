#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="${0:A:h:h}"
cd "$ROOT_DIR"
if [[ "$OSTYPE" != darwin* ]]; then
  print -u2 "This build launcher is for macOS."
  exit 1
fi
ORANGEDECK_CARGO="${commands[cargo]:-$HOME/.cargo/bin/cargo}"
if [[ ! -x "$ORANGEDECK_CARGO" ]]; then
  print -u2 "Cargo was not found. See docs/MACOS_SETUP.md."
  exit 1
fi
exec "$ORANGEDECK_CARGO" +1.95.0 build --locked --release -p orangedeck-connector
