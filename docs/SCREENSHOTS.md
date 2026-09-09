# 화면 둘러보기 · App gallery

**CachyOS Handheld를 쓰는 ROG Ally에서 앱을 1280 × 800으로 실행**하고 화면을 촬영했습니다. 한국어 README에는 한국어 화면, English README에는 영어 화면을 사용합니다. 각 언어의 네 탭·대화 연결창·응답창·수정 파일창, 총 **14장**입니다.

촬영에는 기존 로컬 **demo** 통신 모듈만 사용했습니다. `SIMULATED MAC`, `LOCAL MOCK`, `/mock/` 프로젝트와 모의 대화·토큰을 보여줍니다. 에이전트들은 다섯 모의 대화를 연결한 예시입니다. 두 대화는 파일 수정이 없고, 세 대화에는 모의 파일 편집을 넣었습니다. 응답창에는 내장 모의 승인 시나리오를 사용합니다. 실제 Mac 주소·계정·페어링 토큰·개인 프로젝트·바탕화면은 포함하지 않았습니다.

**모든 화면은 0.1.33 UI입니다.** 오른쪽 위의 **한국어 / English**로 언어를 바꿉니다. 에이전트들 아래 버튼으로 파일 목록을 열고, 원하는 파일을 선택하면 Mac 편집기로 이동합니다.

실제 앱에서 모의 데이터로 촬영한 화면입니다. 실제 사용자 대화를 번역하거나 수정한 것이 아닙니다. 각 그림을 눌러 크게 볼 수 있습니다.

| 화면 / View | 한국어 | English |
| --- | --- | --- |
| 01 · LIVE | [live.png](screenshots/live.png) | [en-live.png](screenshots/en-live.png) |
| 02 · 에이전트들 / Agents | [shortcuts.png](screenshots/shortcuts.png) | [en-shortcuts.png](screenshots/en-shortcuts.png) |
| 03 · 프로젝트들 / Projects | [projects.png](screenshots/projects.png) | [en-projects.png](screenshots/en-projects.png) |
| 04 · 대화 / Conversations | [conversations.png](screenshots/conversations.png) | [en-conversations.png](screenshots/en-conversations.png) |
| 대화 연결 / Assign conversation | [key-editor.png](screenshots/key-editor.png) | [en-key-editor.png](screenshots/en-key-editor.png) |
| 응답·승인 / Response and approval | [response.png](screenshots/response.png) | [en-response.png](screenshots/en-response.png) |
| 수정 파일 / Changed files | [files.png](screenshots/files.png) | [en-files.png](screenshots/en-files.png) |

[처음 설치](INSTALL.md) · [평소 사용법](QUICKSTART_KO.md) · [README로 돌아가기](../README.ko.md)

## English

These **fourteen native app captures show the 0.1.33 UI at 1280 × 800**, using simulated data on a ROG Ally running CachyOS Handheld. They cover all four tabs and the conversation assignment, response and file-list dialogs in both languages. Use **한국어 / English** at the top right to switch the interface language.

The lower Agents key opens the changed-file list. Select a file to open it in your Mac editor. These examples contain synthetic conversations, approvals and `/mock/` paths, with no personal account, address, credential, project or desktop pixels. Click a picture in the table to enlarge it.

[Installation](INSTALL.en.md) · [Controls and everyday use](CONTROLS.md) · [Back to README](../README.en.md)

## 기기 연결 그림 · Device diagrams

README의 [한국어 그림](diagrams/device-roles-ko.svg)과 [영어 그림](diagrams/device-roles-en.svg)은 두 기기의 역할과 설치 순서를 설명하려고 직접 그린 SVG입니다. Mac의 macOS에서 Codex·통신 모듈을, CachyOS Handheld의 ROG Ally에서 LIVE·에이전트들 화면을 실행하는 구성을 보여줍니다.

The two static SVG diagrams explain device roles and setup separately from the app screenshots. They contain no personal machine data, scripts, external resources or embedded images. The drawing is not evidence of a successful installation on another device.
