#!/usr/bin/env zsh
set -euo pipefail

# Guided installs use their stable launcher. Legacy source installs build locally.
# Run Setup OrangeDeck.command first when updating a guided installation.
ORANGEDECK_ROOT="${0:A:h}"
ORANGEDECK_AGENT="$ORANGEDECK_ROOT/target/release/orangedeck-agent"
ORANGEDECK_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/agent.toml"
ORANGEDECK_INSTALLED="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/start-agent.command"
if [[ -x "$ORANGEDECK_INSTALLED" ]]; then
  exec "$ORANGEDECK_INSTALLED"
fi

if [[ "$OSTYPE" != darwin* || ! -f "$ORANGEDECK_ROOT/Cargo.toml" || ! -f "$ORANGEDECK_CONFIG" ]]; then
  print -u2 "Mac에서 새 소스 압축을 전부 푼 뒤 이 파일을 실행하세요."
  print -u2 "First install / 처음 설치: Setup OrangeDeck.command 또는 sh install.sh --role agent"
  read -r "?Press Return to close. "
  exit 1
fi

cd "$ORANGEDECK_ROOT"
if [[ ! -x "$ORANGEDECK_AGENT" ]]; then
  print "첫 실행: 이 새 소스 폴더 안에서 Mac Agent를 빌드합니다."
  print "기존 설치, 페어링, Codex 설정과 대화는 변경하지 않습니다."
  if ! ./scripts/build-macos-agent.zsh; then
    print -u2 "빌드 실패. 기존 Mac Agent는 그대로 사용할 수 있습니다."
    read -r "?Press Return to close. "
    exit 1
  fi
  "$ORANGEDECK_AGENT" doctor --config "$ORANGEDECK_CONFIG" --codex-monitor || {
    print -u2 "연결 진단 실패. 위 내용을 확인하세요. 새 대화는 만들지 않았습니다."
    read -r "?Press Return to close. "
    exit 1
  }
fi

print "OrangeDeck Mac Agent — 이 Terminal을 열어 두세요. 종료: Ctrl-C."
print "이전 Agent가 실행 중이면 그 Agent의 Terminal에서만 Ctrl-C로 종료하세요."
"$ORANGEDECK_AGENT" serve --config "$ORANGEDECK_CONFIG" || {
  orange_exit_code=$?
  print -u2 "Agent stopped with exit code $orange_exit_code."
  print -u2 "주소 사용 중 오류라면 이전 OrangeDeck Agent만 종료하고 이 파일을 다시 실행하세요."
  read -r "?Press Return to close. "
  exit "$orange_exit_code"
}
