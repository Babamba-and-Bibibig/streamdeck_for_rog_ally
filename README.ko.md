<h3 align="center">🇰🇷 한국어 · 현재 페이지　|　<a href="README.en.md">🌐 English</a></h3>

<h1 align="center">OrangeDeck</h1>
<h2 align="center">Mac의 Codex 작업을<br>Ally에서 한눈에.</h2>

Mac에서 Codex를 돌리고, 손에 든 Ally에서 진행 상황을 확인하세요. **작업 상태를 보고, 응답과 승인 요청을 읽고, 수정된 파일을 Mac 편집기로 여는 리모컨**입니다.

[⬇ ZIP 다운로드](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally/archive/refs/heads/main.zip) · [설치하기](#설치하기) · [간단 사용법](docs/QUICKSTART_KO.md)

## LIVE · 지금 하는 일과 남은 사용량

### 질문 · 작업 상태 · 토큰 기록을 한 화면에서

![LIVE: 현재 질의, 작업 상태, 토큰 사용 기록과 남은 사용량](docs/screenshots/live.png)

지금 진행 중인 질문과 작업 상태, 입력·출력 토큰 기록을 확인합니다. **5시간·주간 한도는 남은 비율**로 보여주므로 얼마나 더 쓸 수 있는지 알아보기 쉽습니다.

대화를 하나 골라 계속 보거나 **자동 ON**으로 선택한 프로젝트의 최근 대화를 따라갈 수 있습니다.

## 에이전트들 · 여러 대화의 응답과 파일을 한곳에

### 한 열에 한 대화. 위는 응답, 아래는 수정 파일.

![에이전트들: 다섯 Codex 대화에 연결한 응답 버튼과 수정 파일 버튼](docs/screenshots/shortcuts.png)

**최대 다섯 개의 Codex 대화**를 연결할 수 있습니다. **위쪽 + 버튼**에서 Mac의 대화를 한 번 고르면 다음 실행에도 기억합니다.

새 응답이나 승인 요청이 오면 **그 열의 두 버튼이 함께 깜빡이고 알림음**이 납니다. 어느 대화를 확인할지 바로 알아볼 수 있습니다. 알림음은 켜고 끌 수 있습니다.

### ① 위 버튼 · 응답을 읽고, 필요한 요청만 승인

질문과 Codex의 응답을 읽습니다. 실제 승인 요청이 있으면 상세 내용을 확인하고 **승인 / 거부**를 직접 누릅니다. 결정은 **지금 표시된 요청 하나**에만 전달됩니다.

![질문·응답과 승인 요청을 읽고 직접 결정하는 창](docs/screenshots/response.png)

창 바깥이나 **닫기**를 누르면 창만 닫힙니다. 승인·거절은 보내지 않습니다.

### ② 아래 버튼 · 파일 목록부터 보고, 원하는 파일만 열기

**아래 버튼 → 파일명·전체 경로 확인 → 파일 선택 → Mac 편집기**

![이번 질의에서 수정된 파일의 이름·전체 경로·변경 줄을 보여주는 목록](docs/screenshots/files.png)

아래 버튼은 **이번 질의의 수정 파일 목록만** 엽니다. **목록에서 파일을 누를 때** Mac 편집기가 열리며, 그 파일의 기록된 변경 줄로 이동합니다. 코드 내용은 Mac 편집기에서 확인하세요. 변경 줄 정보가 없으면 첫 줄로 엽니다.

**작업 폴더는 연결한 Codex 대화에서 자동으로 가져옵니다.** 경로를 다시 등록할 필요가 없습니다. Zed·VS Code·Cursor·VSCodium을 지원합니다. 파일 변경 이벤트로 바뀐 경로만 수집하며 프로젝트 전체를 다시 훑지 않습니다.

수정이 없다고 확인되면 **파일 수정 없음**으로 표시합니다. [파일 목록·재시도·편집기 설정](docs/INSTALL.md#host-shortcuts).

모든 화면은 **모의 데이터**입니다. 사진을 누르면 크게 볼 수 있습니다. [촬영 정보](docs/SCREENSHOTS.md).

## 두 기기는 이렇게 연결됩니다

![Mac의 Codex와 통신 모듈을 Ally의 LIVE·에이전트들 화면에 연결하고, 승인과 파일 열기 요청을 Mac으로 보냅니다.](docs/diagrams/device-roles-ko.svg)

**AI 작업은 Mac의 Codex가 하고, OrangeDeck은 두 기기를 연결합니다.** Ally에 Codex를 설치할 필요는 없습니다.

| 기기 | 설치할 것 |
| --- | --- |
| **macOS 작업용 Mac** · Mac Studio를 기준으로 개발 | Codex CLI + OrangeDeck **통신 모듈(Connector)** |
| **CachyOS Handheld의 ROG Ally** · 데스크톱 모드 | OrangeDeck **리모컨(UI)** |

이 설명서는 위 구성을 대상으로 합니다. Steam Deck·SteamOS는 설치·동작을 확인하지 않았으며, 다른 Linux 배포판과 Windows는 안내 대상에 포함하지 않습니다.

## 설치하기

**Mac 설치 → 연결 파일 옮기기 → Ally 설치**, 세 단계입니다. 현재 버전은 **0.1.32**입니다.

먼저 두 기기에 [Tailscale](https://tailscale.com/download)을 설치하고 같은 계정으로 로그인하세요. Mac에는 [Codex CLI](https://learn.chatgpt.com/docs/cli)를 설치하고 로그인해 둡니다. 두 기기에 Python 3.9 이상이 필요합니다. [준비물 확인](docs/INSTALL.md#준비물).

두 기기에서 위의 **ZIP 다운로드**를 누르고 압축을 풉니다. Git이나 SSH 키는 필요하지 않습니다.

### 1. Mac에 통신 모듈 설치

**아래 세 파일은 Mac에서 실행합니다.** 압축을 푼 폴더에서 순서대로 두 번 누르세요.

1. **Setup OrangeDeck.command** — 설치하고 내 작업 폴더·편집기를 고릅니다. 설치 완료 후 안내에 따라 Enter를 누르고 창을 닫습니다.
2. **Start OrangeDeck Connector.command** — Mac과 Ally를 연결합니다. **OrangeDeck을 사용하는 동안 이 창은 켜 둡니다.**
3. **Enable Codex Notifications.command** — 완료 알림·승인 요청을 연결합니다. 설정 완료 후 Enter를 누르고 창을 닫습니다.

마지막으로 **평소 사용하는 Mac Codex**에서 **`/hooks` → OrangeDeck 확인·신뢰**를 진행하세요. Mac에서 계속 켜둘 OrangeDeck 창은 **2번 통신 모듈 하나**입니다.

필요한 개발 도구 설치가 뜨면 설치를 마친 뒤 Setup을 다시 실행하세요. [Mac 설치 도움말](docs/INSTALL.md#mac-설치-파일이-안-열릴-때).

### 2. Mac의 연결 파일을 Ally로 옮기기

Mac Finder에서 **이동 → 폴더로 이동…** 메뉴를 고르고, `~/.config/orangedeck`을 엽니다.

그 안의 `orangedeck-pairing.toml` 파일을 USB 등으로 **내 Ally의 다운로드 폴더**에 옮기세요. 내 Mac의 주소와 비밀 연결 인증값이 담긴 **개인 연결 파일**입니다. GitHub나 채팅에 올리지 마세요.

### 3. Ally에 리모컨 설치

Ally의 **데스크톱 모드**에서 압축을 푼 폴더를 열고, 빈 곳을 오른쪽 클릭해 **여기서 터미널 열기**를 누릅니다. Mac용 `.command` 파일 대신 아래 명령을 실행하세요.

```sh
sh install.sh --role ui
```

연결 파일을 물으면 옮겨 둔 `orangedeck-pairing.toml` 파일을 터미널에 끌어 놓고 Enter를 누릅니다. IP 주소나 인증값을 직접 적을 필요가 없습니다.

설치가 끝나면 앱을 켭니다.

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

**연결됨**이 뜨면 **에이전트들 → 위쪽 + 버튼**에서 대화를 연결하세요. Mac에서 Codex 작업을 하면서 LIVE의 새 기록과 에이전트들의 응답·파일을 확인하면 됩니다.

## 다음부터는 켜기만 하세요

| 기기 | 켜기 | 끄기 |
| --- | --- | --- |
| **Mac** | **Start OrangeDeck Connector.command** 두 번 누르기 | 통신 모듈 터미널에서 Ctrl+C |
| **Ally** | `sh "$HOME/.config/orangedeck/start-ui.sh"` | OrangeDeck 창 닫기 |

Codex와 두 기기의 Tailscale은 켜 두세요. 앱을 터미널에서 실행했다면 사용하는 동안 그 터미널도 켜 둡니다. Setup·Enable·설치는 매번 다시 하지 않습니다.

<a id="my-settings"></a>

### 업데이트해도 내 설정은 그대로

처음 설치할 때 각 기기의 설정과 개인 연결 파일을 준비합니다. 같은 기기에 업데이트할 때는 저장된 **작업 폴더·편집기·연결·언어·대화 배치·알림음**을 그대로 씁니다. 개인 설정은 각 기기의 `~/.config/orangedeck/`에 있고 다운로드에 포함되지 않습니다. [업데이트 순서](docs/INSTALL.md#업데이트) · [설정을 다시 묻지 않는 이유](docs/INSTALL.md#saved-settings).

[간단 사용법·패드 조작](docs/QUICKSTART_KO.md) · [설치·연결 문제 해결](docs/INSTALL.md) · [English guide](README.en.md) · [변경 기록](docs/CHANGELOG.md)
