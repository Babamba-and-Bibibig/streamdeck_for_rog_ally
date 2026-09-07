#!/bin/sh
set -eu
ORANGEDECK_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ "$(uname -s)" != Darwin ]; then
  echo 'Use sh install.sh on Linux.' >&2
  exit 1
fi
sh "$ORANGEDECK_ROOT/install.sh" --role connector "$@" || {
  code=$?
  printf 'Setup stopped. Press Return / 설치 중단. Enter를 누르세요. '
  read -r answer
  exit "$code"
}
printf 'Press Return to close / Enter를 눌러 닫으세요. '
read -r answer
