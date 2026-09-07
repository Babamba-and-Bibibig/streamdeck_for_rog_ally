#!/usr/bin/env zsh
set -euo pipefail

ORANGEDECK_ROOT="${0:A:h:h}"
ORANGEDECK_CONNECTOR="$ORANGEDECK_ROOT/target/release/orangedeck-connector"
ORANGEDECK_CONFIG="${1:-$HOME/.config/orangedeck/connector.toml}"
ORANGEDECK_INSTALLED="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/start-connector.command"
if (( $# == 0 )) && [[ -x "$ORANGEDECK_INSTALLED" ]]; then
  exec "$ORANGEDECK_INSTALLED"
fi

if [[ "$OSTYPE" != darwin* ]]; then
  print -u2 "Run this launcher on the Mac. On the ROG Ally, use scripts/run-ally.zsh."
  exit 1
fi
if [[ ! -x "$ORANGEDECK_CONNECTOR" ]]; then
  print -u2 "Mac Connector is missing. In $ORANGEDECK_ROOT run:"
  print -u2 "cargo build --locked --release -p orangedeck-connector"
  exit 1
fi
if [[ ! -f "$ORANGEDECK_CONFIG" ]]; then
  print -u2 "Connector config is missing: $ORANGEDECK_CONFIG"
  print -u2 "Complete the one-time setup in docs/MACOS_SETUP.md."
  exit 1
fi

print "OrangeDeck Mac Connector — keep this Terminal open. Stop with Ctrl-C."
exec "$ORANGEDECK_CONNECTOR" serve --config "$ORANGEDECK_CONFIG"
