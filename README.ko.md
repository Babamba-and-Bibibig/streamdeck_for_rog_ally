<h3 align="center">🇰🇷 한국어 · 현재 페이지　|　<a href="README.en.md">🌐 English</a></h3>

# OrangeDeck

**Mac에서 돌아가는 Codex를 ROG Ally로 확인하고 조작하세요.**

**OrangeDeck은 Mac과 Ally를 연결하는 통신·리모컨 앱입니다.** AI 작업은 Mac의 Codex가 합니다. Ally에는 Codex를 설치할 필요가 없습니다.

![Mac의 macOS에서 Codex와 통신 모듈을 실행하고, ROG Ally의 CachyOS Handheld에서 리모컨을 실행합니다. Tailscale로 연결합니다. Steam Deck과 SteamOS는 아직 확인하지 않았습니다.](docs/diagrams/device-roles-ko.svg)

[⬇ ZIP 다운로드](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally/archive/refs/heads/main.zip) · [설치하기](#설치하기) · [앱 화면 보기](#앱-화면-보기) · [간단 사용법](docs/QUICKSTART_KO.md)

## 설치할 기기

**이 설명서는 아래 두 기기를 연결하는 방법을 안내합니다.**

| 기기 | 운영체제 | 설치할 것 |
| --- | --- | --- |
| **작업용 Mac** — Mac Studio를 기준으로 개발 | **macOS** | Codex CLI + **통신 모듈(Connector)**. Codex 상태를 보내고 Ally의 버튼 입력을 받습니다. |
| **리모컨용 ROG Ally** | **CachyOS Handheld · 데스크톱 모드** | **OrangeDeck 리모컨(UI)**. 작업 상태를 보고 버튼을 누르는 앱입니다. |

**Steam Deck·SteamOS는 설치와 동작을 아직 확인하지 않았습니다.** 다른 Linux 배포판도 이 설명서의 설치 대상에 포함하지 않습니다. Windows는 지원하지 않습니다.

현재 버전: **0.1.23**.

## 설치하기

순서는 **Mac 설치 → 연결 파일 옮기기 → Ally 설치**입니다.

먼저 준비하세요.

- **두 기기:** [Tailscale](https://tailscale.com/download)을 설치하고 **같은 계정**으로 로그인합니다. 두 기기를 연결해 주는 프로그램입니다.
- **Mac:** [Codex CLI](https://learn.chatgpt.com/docs/cli)를 설치하고 로그인해 둡니다. 터미널에서 쓰는 Codex가 필요합니다.
- **두 기기:** Python 3.9 이상이 필요합니다. [확인·설치 방법](docs/INSTALL.md#준비물)을 참고하세요.
- **두 기기:** 위의 **ZIP 다운로드**를 누르고 압축을 풉니다. Git이나 SSH 키는 필요하지 않습니다.

### 1. Mac에 통신 모듈 설치

압축을 푼 폴더에서 아래 순서대로 진행하세요.

1. **Setup OrangeDeck.command**를 두 번 누릅니다. 작업 폴더를 물으면 **Mac에서 Codex로 작업할 폴더**를 넣습니다. 기기 이름과 프로젝트 이름은 Enter로 기본값을 써도 됩니다.
2. 설치가 끝나면 **Start OrangeDeck Connector.command**를 두 번 누릅니다. **열린 터미널은 켜 두세요.**
3. **Enable Codex Notifications.command**를 두 번 누릅니다. 그다음 평소 쓰는 Mac Codex에서 **`/hooks`**를 입력하고 OrangeDeck 연결을 확인한 뒤 신뢰합니다.

필요한 도구를 설치할지 물으면 내용을 읽고 `y`를 입력하세요. 첫 설치는 몇 분 이상 걸릴 수 있습니다. Mac 개발 도구 설치 창이 뜨면 설치를 마친 뒤 Setup 파일을 다시 여세요. [파일이 안 열릴 때](docs/INSTALL.md#mac-설치-파일이-안-열릴-때).

### 2. Mac의 연결 파일을 Ally로 옮기기

Mac **Finder → 이동 → 폴더로 이동…**에서 아래 경로를 붙여넣으세요.

```text
~/.config/orangedeck
```

그 안의 **`orangedeck-pairing.toml`**을 USB 등으로 **내 Ally의 다운로드 폴더**에 옮깁니다. 두 기기를 연결할 때 쓰는 파일입니다. **이 파일은 GitHub나 채팅에 올리지 마세요.**

### 3. Ally에 리모컨 설치

**CachyOS Handheld의 데스크톱 모드**에서 진행하세요. 압축을 푼 폴더 안의 빈 곳을 오른쪽 클릭해 **여기서 터미널 열기**를 누릅니다. `install.sh`가 있는 폴더여야 합니다.

```sh
sh install.sh --role ui
```

연결 파일을 물으면 방금 옮긴 **`orangedeck-pairing.toml`**을 터미널에 끌어 놓고 Enter를 누릅니다. IP 주소나 비밀번호를 따로 적을 필요는 없습니다. 설치가 끝나면 같은 터미널에서 실행하세요.

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

Ally에 **연결됨**이 뜨면 Mac에서 Codex 작업을 해 보세요. Ally의 숫자와 완료 알림이 바뀌는지 확인합니다. 승인 요청이 생기면 내용을 읽고 직접 눌러 Mac에도 전달되는지 확인하세요.

설정 폴더를 따로 지정했다면 설치 끝에 나온 경로를 사용하세요. [설치가 막힐 때·추가 설정·업데이트](docs/INSTALL.md).

## 앱 화면 보기

사진은 개인 작업 대신 **예제 데이터**로 실행한 앱 화면입니다. 그림이나 사진을 누르면 크게 볼 수 있습니다. [촬영 정보](docs/SCREENSHOTS.md).

### LIVE — 지금 하는 작업을 한눈에

![LIVE 화면: 현재 작업의 토큰과 남은 사용량](docs/screenshots/live.png)

지금 하는 작업의 토큰과 **5시간·주간 남은 사용량**을 봅니다. 숫자는 새 사용 기록을 받으면 바뀝니다. 남은 비율은 쓸수록 줄어듭니다.

<table>
<tr>
<td width="50%">
<b>단축키 — 승인하고, 자주 쓰는 기능을 실행</b><br>
<a href="docs/screenshots/shortcuts.png"><img src="docs/screenshots/shortcuts.png" alt="승인·거절 버튼과 나만의 단축키" width="640"></a><br>
01은 승인, 02는 거절입니다. 나머지 8개 버튼은 원하는 기능으로 바꿀 수 있습니다.
</td>
<td width="50%">
<b>프로젝트들 — 작업 폴더 선택</b><br>
<a href="docs/screenshots/projects.png"><img src="docs/screenshots/projects.png" alt="확인할 작업 폴더를 고르는 화면" width="640"></a><br>
폴더를 고르면 다른 탭에서도 그 폴더의 작업을 보여줍니다.
</td>
</tr>
<tr>
<td width="50%">
<b>대화 — 확인할 대화 선택</b><br>
<a href="docs/screenshots/conversations.png"><img src="docs/screenshots/conversations.png" alt="선택한 폴더의 대화 목록" width="640"></a><br>
한 대화를 계속 보거나, 가장 최근 대화가 자동으로 보이게 할 수 있습니다.
</td>
<td width="50%">
<b>알림 — 질문·답변·완료 확인</b><br>
<a href="docs/screenshots/notifications.png"><img src="docs/screenshots/notifications.png" alt="현재 질문과 답변, 승인할 내용을 읽는 화면" width="640"></a><br>
승인할 내용이 길면 요청 상세를 열고 안쪽을 스크롤해 끝까지 읽으세요.
</td>
</tr>
</table>

## 내 버튼 만들기 · 언어 바꾸기

- **단축키 → 빈 + 버튼 → 기능 선택**으로 저장합니다. 저장한 기능은 그 버튼을 다시 누를 때 실행됩니다.
- **추천 구성**을 누르면 빈 버튼을 한 번에 채웁니다. **키 편집**에서 바꾸거나 비울 수 있습니다. 01 승인·02 거절은 바꿀 수 없습니다.
- 오른쪽 위 **한국어 / EN**으로 언어를 바꿉니다. 버튼과 언어 설정은 다음에 켜도 유지됩니다.

| 버튼에 넣을 수 있는 기능 | 하는 일 |
| --- | --- |
| LIVE·프로젝트·대화·알림 | 해당 화면 열기 |
| 새로고침·최근 자동 | 새 정보 가져오기·최근 대화를 자동으로 보기 |
| 이전/다음 프로젝트·대화 | 보고 있는 작업이나 대화 바꾸기 |
| 에디터·터미널·폴더·웹페이지 | **Mac에서** 해당 프로젝트 열기 |

Mac의 에디터는 Zed·VS Code·VSCodium 중 하나가 필요합니다. 설치 때 고른 작업 폴더에만 열기 버튼을 쓸 수 있습니다. [다른 폴더나 웹페이지 추가하기](docs/INSTALL.md#host-shortcuts).

![빈 버튼에 넣을 기능을 고르는 한국어 편집창](docs/screenshots/key-editor.png)

## 다시 켜기·끄기

**Mac에서 통신 모듈을 먼저 켜고, Ally에서 리모컨을 켭니다.** 끌 때는 Ally 앱 창을 닫고, Mac의 통신 모듈 터미널에서 **Ctrl+C**를 누릅니다. 다시 켤 때마다 설치하거나 연결 파일을 옮길 필요는 없습니다.

[실행 명령·버튼 조작](docs/QUICKSTART_KO.md) · [연결이 안 될 때](docs/INSTALL.md#연결이나-알림이-안-될-때) · [업데이트](docs/INSTALL.md#업데이트)

## 참고

Mac의 작업 정보는 연결한 Ally로 보냅니다. Codex 로그인 정보는 Mac에 남으며, OrangeDeck이 작업 내용을 GitHub로 보내지는 않습니다. 연결 파일과 개인 설정은 공개하지 마세요.

[개발 구조](docs/ARCHITECTURE.md) · [변경 내역](docs/CHANGELOG.md) · [개발·게시 검사](docs/PUBLICATION.md)

OpenAI·ASUS·Elgato의 공식 제품이 아닌 개인 프로젝트입니다. [MIT 라이선스](LICENSE).
