#!/usr/bin/env zsh
set -euo pipefail

ORANGEDECK_ROOT="${0:A:h:h}"
ORANGEDECK_AGENT="$ORANGEDECK_ROOT/target/release/orangedeck-agent"
ORANGEDECK_CONFIG="${1:-$HOME/.config/orangedeck/agent.toml}"
ORANGEDECK_INSTALLED="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/start-agent.command"
if (( $# == 0 )) && [[ -x "$ORANGEDECK_INSTALLED" ]]; then
  exec "$ORANGEDECK_INSTALLED"
fi

if [[ "$OSTYPE" != darwin* ]]; then
  print -u2 "Run this launcher on the Mac. On the ROG Ally, use scripts/run-ally.zsh."
  exit 1
fi
if [[ ! -x "$ORANGEDECK_AGENT" ]]; then
  print -u2 "Mac Agent is missing. In $ORANGEDECK_ROOT run:"
  print -u2 "cargo build --locked --release -p orangedeck-agent"
  exit 1
fi
if [[ ! -f "$ORANGEDECK_CONFIG" ]]; then
  print -u2 "Agent config is missing: $ORANGEDECK_CONFIG"
  print -u2 "Complete the one-time setup in docs/MACOS_SETUP.md."
  exit 1
fi

print "OrangeDeck Mac Agent — keep this Terminal open. Stop with Ctrl-C."
exec "$ORANGEDECK_AGENT" serve --config "$ORANGEDECK_CONFIG"
