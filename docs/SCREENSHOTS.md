# README 스크린샷 · Screenshot provenance

**CachyOS Handheld를 쓰는 ROG Ally에서 0.1.28 앱을 1280 × 800으로 실행**하고 화면을 촬영했습니다. 한국어 README에는 한국어 화면, English README에는 영어 화면을 사용합니다. 각 언어의 네 탭·대화 연결창·응답창·수정 파일창, 총 **14장**입니다.

촬영에는 기존 로컬 **demo** 통신 모듈만 사용했습니다. `SIMULATED MAC`, `LOCAL MOCK`, `/mock/` 프로젝트와 모의 대화·토큰을 보여줍니다. 에이전트들은 다섯 모의 대화를 연결한 예시입니다. 두 대화는 파일 수정이 없고, 세 대화에는 모의 파일 편집을 넣었습니다. 응답창에는 내장 모의 승인 시나리오를 사용합니다. 실제 Mac 주소·계정·페어링 토큰·개인 프로젝트·바탕화면은 포함하지 않았습니다.

비공개 소스 복사본에 탭 전환·촬영 자동화만 넣고 실제 앱의 프레임버퍼를 저장했습니다. 제품 화면 그리기 코드·배치·문구는 그대로이며, 기기 합성 사진이나 생성형 UI 이미지가 아닙니다. 영어 예시는 `demo --language en`의 모의 데이터입니다. 실제 사용자 대화를 번역하거나 수정한 것이 아닙니다. 이 촬영은 실제 Mac 알림·승인·편집기 왕복이나 SteamOS 설치 검증을 뜻하지 않습니다.

| 화면 / View | 한국어 | English |
| --- | --- | --- |
| 01 · LIVE | [live.png](screenshots/live.png) | [en-live.png](screenshots/en-live.png) |
| 02 · 에이전트들 / Agents | [shortcuts.png](screenshots/shortcuts.png) | [en-shortcuts.png](screenshots/en-shortcuts.png) |
| 03 · 프로젝트들 / Projects | [projects.png](screenshots/projects.png) | [en-projects.png](screenshots/en-projects.png) |
| 04 · 대화 / Conversations | [conversations.png](screenshots/conversations.png) | [en-conversations.png](screenshots/en-conversations.png) |
| 대화 연결 / Assign conversation | [key-editor.png](screenshots/key-editor.png) | [en-key-editor.png](screenshots/en-key-editor.png) |
| 응답·승인 / Response and approval | [response.png](screenshots/response.png) | [en-response.png](screenshots/en-response.png) |
| 수정 파일 / Changed files | [files.png](screenshots/files.png) | [en-files.png](screenshots/en-files.png) |

직접 둘러보려면 소스를 빌드한 뒤 `./scripts/run-demo.zsh`(한국어) 또는 `./scripts/run-demo.zsh en`(영어)을 실행합니다. 상단 언어 버튼으로 안내 문구를 바꿀 수도 있습니다. [개발 안내](PUBLICATION.md#development). 모의 값과 시각은 실행 중 변합니다.

**관리자:** 모든 이미지를 눈으로 검토하고 메타데이터를 확인한 후 해시를 등록합니다. PNG에는 `IHDR`, `IDAT`, `IEND`만 있으며 텍스트·EXIF 등 부가 메타데이터는 없습니다. 현재 공개 이미지는 위 14장입니다. 제거한 알림 탭 사진의 과거 해시는 이전 Git 기록을 검사하기 위해 유지합니다. `scripts/release_policy.py`에 등록한 경로와 SHA256이 모두 일치해야 허용하며, 같은 이름의 미검토 이미지도 차단합니다. 그 외 스크린샷과 촬영 자동화·작업 파일은 비공개로 제외합니다.

## English

These are **0.1.28 app captures at 1280 × 800 on a ROG Ally running CachyOS Handheld**, with matching Korean/English UI and synthetic conversations. Each language has four tabs, an assignment dialog, a response dialog and a file dialog: **fourteen captures** in total. A private source copy automated navigation and framebuffer capture without changing production drawing code. The deck connects five synthetic conversations, including recorded file changes and questions with no edits. Approval views use the existing loopback demo scenario. No real account, address, credential, project or desktop pixels were used. No generated UI or device mockups are included.

Build, then run `./scripts/run-demo.zsh en` to explore the English demo or omit `en` for Korean. The language toggle changes UI text; actual user content stays unchanged. Demo data and timestamps vary. Captures do not establish real Mac notification, approval or editor delivery, or SteamOS support.

The fourteen screenshot paths above are used in the current source. The removed Notifications captures retain their previously reviewed hashes for historical Git checks. PNGs have no ancillary metadata; SHA256 checks protect reviewed bytes in the Git index/history, ZIP and TAR exports. Review replacements visually and retain earlier approved hashes so historical revisions remain verifiable. A hash identifies reviewed bytes; it cannot establish the privacy of a new picture by itself.

## 기기 연결 그림 · Device diagrams

README의 [한국어 그림](diagrams/device-roles-ko.svg)과 [영어 그림](diagrams/device-roles-en.svg)은 두 기기의 역할과 설치 순서를 설명하려고 직접 그린 SVG입니다. Mac의 macOS에서 Codex·통신 모듈을, CachyOS Handheld의 ROG Ally에서 LIVE·에이전트들 화면을 실행하는 구성을 보여줍니다.

The two static SVG diagrams explain device roles and setup separately from the app screenshots. They contain no personal machine data, scripts, external resources or embedded images. Their exact paths and visually reviewed hashes are pinned in REVIEWED_DIAGRAMS; changed bytes require another review. The drawing is not evidence of a successful installation on another device.
