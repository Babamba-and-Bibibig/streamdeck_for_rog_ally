# README screenshots / README 스크린샷

The gallery shows OrangeDeck **0.1.19** running natively on Linux at **1280 × 800**. The images were captured from the app's own framebuffer using a private copy with tab-navigation and screenshot automation. Production UI drawing code, layouts and labels were unchanged. No device mockups or generated UI images are used.

All displayed data comes from the existing loopback **demo** Agent: `SIMULATED MAC`, `LOCAL MOCK`, `/mock/` projects, sample conversations and simulated usage. The shortcut and notification images show the built-in simulated approval scenario. No real Codex account, Mac address, pairing token, personal project or desktop background was used. Simulation does not verify real Mac approval delivery or SteamOS installation.

| Tab | Image |
| --- | --- |
| 01 · LIVE | [live.png](screenshots/live.png) |
| 02 · Shortcuts / 단축키 | [shortcuts.png](screenshots/shortcuts.png) |
| 03 · Projects / 프로젝트들 | [projects.png](screenshots/projects.png) |
| 04 · Conversations / 대화 | [conversations.png](screenshots/conversations.png) |
| 05 · Notifications / 알림 | [notifications.png](screenshots/notifications.png) |

To explore the same interface locally, build with `cargo build --release --workspace --locked`, then run `./scripts/run-demo.zsh` and use the five tab buttons. See the [development instructions](../README.md#개발과-라이선스). Demo data changes while it runs, so readings and timestamps can differ.

For maintainers: inspect every image for visible private information before publication. PNGs contain only `IHDR`, `IDAT` and `IEND` chunks, without text, EXIF or other ancillary metadata. Git, source exports and archive validation permit only these five exact paths. `scripts/release_policy.py` pins their reviewed SHA256 hashes; modified bytes are rejected even at an approved filename. Keep earlier reviewed hashes when replacing images so complete history can still be checked. All other screenshots remain private and ignored. A hash confirms reviewed bytes, not the privacy of a newly captured picture.

---

이 갤러리는 **0.1.19 Linux 네이티브 앱을 1280 × 800으로 실행**하고 앱 자체의 화면을 캡처했습니다. 별도 비공개 복사본에 탭 전환·촬영 자동화만 넣었으며 제품의 화면 그리기 코드·배치·문구는 그대로입니다. 기기 합성 사진이나 생성형 UI 이미지가 아닙니다.

기존 로컬 **demo** Agent의 모의 프로젝트·대화·토큰과 승인 시나리오만 사용했습니다. 실제 Mac 주소, 계정, 페어링 토큰, 개인 작업과 바탕화면은 포함하지 않았습니다. 실제 Mac 승인 왕복이나 SteamOS 설치 검증을 뜻하지는 않습니다.

관리자는 새 이미지의 화면 내용과 메타데이터를 직접 검토한 후 해시를 등록해야 합니다. 위 다섯 파일만 공개 경로로 허용하며, 같은 이름으로 바꿔 넣은 미검토 이미지도 검사가 차단합니다. 그 외 개인 스크린샷은 계속 Git과 배포물에서 제외합니다.
