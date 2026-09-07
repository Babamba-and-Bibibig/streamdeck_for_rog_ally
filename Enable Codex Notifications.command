#!/usr/bin/env zsh
set -euo pipefail

ORANGEDECK_ROOT="${0:A:h}"
ORANGEDECK_CONNECTOR="$ORANGEDECK_ROOT/target/release/orangedeck-connector"
ORANGEDECK_INSTALLED="${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/enable-notifications.command"
if [[ -x "$ORANGEDECK_INSTALLED" ]]; then
  "$ORANGEDECK_INSTALLED"
  print "Open /hooks in your usual Codex session and review/trust OrangeDeck."
  read -r "?Press Return / Enter를 누르세요. "
  exit 0
fi

if [[ "$OSTYPE" != darwin* || ! -f "$ORANGEDECK_ROOT/Cargo.toml" ]]; then
  print -u2 "Mac에서 새 OrangeDeck 폴더 안의 파일을 실행하세요."
  exit 1
fi

cd "$ORANGEDECK_ROOT"
if [[ ! -x "$ORANGEDECK_CONNECTOR" ]]; then
  ./scripts/build-macos-connector.zsh
fi

print "평소 사용하는 Mac Codex의 완료·승인 알림을 OrangeDeck에 연결합니다."
print "기존 hooks.json은 보존·백업하며, config.toml과 인증 정보는 바꾸지 않습니다."
"$ORANGEDECK_CONNECTOR" codex-hooks --install --config "${XDG_CONFIG_HOME:-$HOME/.config}/orangedeck/connector.toml"
print ""
print "마지막으로 Mac Codex에서 /hooks를 열고 OrangeDeck 항목을 검토·신뢰하세요."
print "현재 세션에 보이지 않으면 작업이 끝난 뒤 같은 대화를 다시 열어 /hooks를 확인하세요."
print "승인 버튼은 이 연결을 통해 도착한 요청에만 적용됩니다."
read -r "?안내를 확인했으면 Return을 눌러 닫으세요. "
