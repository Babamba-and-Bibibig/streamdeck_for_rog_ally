# Codex 알림 연결

[처음 설치](../README.ko.md#설치하기) · [평소 사용](QUICKSTART_KO.md)

1. 생성된 `start-agent.command` 또는 소스 폴더의 `Start OrangeDeck Agent.command`로 Agent를 실행합니다.
2. **다른 터미널**에서 설치기가 알려준 `enable-notifications.command` 또는 소스 폴더의 `Enable Codex Notifications.command`를 실행합니다.
3. 평소 사용하는 Agent 기기의 Codex에서 `/hooks`를 열고 OrangeDeck의 정의·경로를 검토한 다음 신뢰합니다. 현재 대화에 보이지 않으면 작업이 끝난 뒤 같은 대화를 다시 열어 확인합니다.
4. UI가 연결된 상태에서 새 작업의 토큰·완료 알림을 확인합니다. 실제로 발생한 승인 요청을 읽고 직접 승인 또는 거절하여 원래 Codex에 반영되는지도 별도로 확인합니다.

설치기는 기존의 다른 훅을 보존하고 변경 전에 비공개 백업을 만듭니다. Codex의 `config.toml`·인증 정보·신뢰 기록을 임의로 변경하지 않습니다. `UserPromptSubmit`, `Stop`, `PermissionRequest`를 사용하고, 버전 확인 결과 지원될 때 `Interrupt`를 추가합니다. `/hooks`가 없는 Codex에서는 이 연결을 사용할 수 없습니다.

훅은 평소 Codex와 같은 사용자·같은 `CODEX_HOME`에서 설치해야 합니다. 생성된 실행기는 설치 시 선택된 실행 파일과 `CODEX_HOME` 환경값을 유지합니다. 다른 프로필로 바꾸면 해당 프로필에서 다시 설치하고 신뢰를 검토하세요.

0.1.19는 실제 Agent 설정 폴더의 `hooks/codex.sock`을 사용합니다. 0.1.18 이하에서 업데이트하면 **새 Agent 실행 → 알림 설치 실행기 재실행 → `/hooks` 재검토·신뢰**가 한 번 필요합니다. 직접 CLI를 사용할 때도 Agent와 같은 `--config`를 지정하세요. 생성된 실행기는 이를 자동으로 지정합니다.

승인에는 새 클릭·터치·물리 입력이 필요합니다. 연결 실패·시간 초과·사용자 미응답은 자동 승인이 아니며 Codex의 기본 승인 절차로 돌아갑니다. 일반 질문의 답변은 Codex에서 합니다.

근거: [공식 Codex 훅과 신뢰 안내](https://learn.chatgpt.com/docs/hooks).
