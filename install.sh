#!/bin/sh
set -eu
ORANGEDECK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if ! command -v python3 >/dev/null 2>&1; then
  echo 'Python 3.9+ is required / Python 3.9 이상이 필요합니다.' >&2
  echo 'macOS: install Python 3.9+ from https://www.python.org/downloads/macos/ then retry.' >&2
  echo 'Linux: install python3 with your distribution package manager.' >&2
  exit 1
fi
exec python3 "$ORANGEDECK_ROOT/scripts/install.py" "$@"
