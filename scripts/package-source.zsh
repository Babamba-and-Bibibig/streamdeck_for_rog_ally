#!/usr/bin/env zsh
set -euo pipefail
ORANGEDECK_ROOT="${0:A:h:h}"
if (( $# > 0 )); then
  print -u2 "Use the verified, versioned files in dist/; custom output archives are no longer created."
  exit 1
fi
exec python3 "$ORANGEDECK_ROOT/download-page/manage.py" prepare
