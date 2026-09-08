<h3 align="center">🇰🇷 한국어 · 현재 페이지　|　<a href="README.en.md">🌐 English</a></h3>

# OrangeDeck

**Mac의 Codex를, Ally에서 한눈에.**

지금 작업은 **LIVE**, 여러 Codex의 응답과 수정 파일은 **에이전트들**에서 확인하세요. 승인 요청에 답하고, 수정 파일을 눌러 Mac 편집기로 바로 이동할 수 있습니다.

[⬇ ZIP 다운로드](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally/archive/refs/heads/main.zip) · [설치하기](#설치하기) · [간단 사용법](docs/QUICKSTART_KO.md)

## LIVE · 지금 무엇을 하고 있는지

![LIVE: 현재 질의, 작업 상태, 토큰 사용 기록과 남은 사용량](docs/screenshots/live.png)

**현재 질문과 작업 상태, 사용한 토큰, 남은 사용량을 한 화면에서 봅니다.**

- **현재 작업:** 지금 진행 중인 질의와 작업 상태를 확인합니다.
- **사용 기록:** 입력·출력 토큰을 보고, 새 기록이 오면 갱신됩니다.
- **남은 한도:** 5시간·주간 남은 비율을 확인합니다. 쓸수록 줄어듭니다.

대화를 하나 골라 계속 보거나 **자동 ON**으로 선택한 프로젝트의 최근 대화를 따라갈 수 있습니다.

## 에이전트들 · 다섯 대화의 응답과 수정 파일

![에이전트들: 다섯 Codex 대화에 연결한 응답 버튼과 수정 파일 버튼](docs/screenshots/shortcuts.png)

**한 열이 한 대화입니다. 위는 응답, 바로 아래는 수정 파일.**

새 응답이나 승인 요청이 오면 같은 열의 두 버튼이 함께 깜빡이고 알림음이 납니다. **위쪽 + 버튼**에서 Mac 터미널의 Codex 대화를 한 번 연결하면 됩니다.

<table>
<tr>
<td width="50%">
<b>① 위 버튼 · 응답 읽고 승인하기</b><br>
<a href="docs/screenshots/response.png"><img src="docs/screenshots/response.png" alt="해당 질의의 응답과 승인 요청을 읽는 창" width="640"></a><br>
질문과 응답을 읽습니다. 실제 승인 요청이 있으면 내용을 확인한 뒤 <b>승인 / 거부</b>를 누릅니다.
</td>
<td width="50%">
<b>② 아래 버튼 · 수정 파일 열기</b><br>
<a href="docs/screenshots/files.png"><img src="docs/screenshots/files.png" alt="수정 파일 목록과 변경 내용, Mac 편집기로 파일 열기" width="640"></a><br>
Ally에서 변경 내용을 보고, 파일을 누르면 <b>Mac 편집기</b>가 그 파일로 이동합니다.
</td>
</tr>
</table>

**폴더는 연결한 Codex 대화의 작업 폴더를 자동으로 사용합니다.** 파일을 열기 위해 경로를 다시 등록할 필요가 없습니다. Zed·VS Code·Cursor·VSCodium을 지원합니다.

파일 목록은 **이번 질의에 기록된 편집**을 보여줍니다. 기록이 부족하면 **파일 정보 확인**, 편집이 없다고 확인되면 **파일 수정 없음**으로 표시합니다. [파일 확인·재시도와 편집기 설정](docs/INSTALL.md#host-shortcuts).

창 바깥이나 **닫기**를 누르면 창만 닫힙니다. 승인·거절은 보내지 않습니다. 언어·대화 연결·알림음 설정은 다음 실행에도 유지됩니다.

<sub>화면은 0.1.28의 모의 데이터입니다. 사진을 누르면 크게 볼 수 있습니다. <a href="docs/SCREENSHOTS.md">촬영 정보</a></sub>

## 두 기기는 이렇게 연결됩니다

![Mac의 Codex와 통신 모듈을 Ally의 LIVE·에이전트들 화면에 연결하고, 승인과 파일 열기 요청을 Mac으로 보냅니다.](docs/diagrams/device-roles-ko.svg)

**AI 작업은 Mac의 Codex가 하고, OrangeDeck은 두 기기를 연결합니다.** Ally에 Codex를 설치할 필요는 없습니다.

| 기기 | 설치할 것 |
| --- | --- |
| **macOS 작업용 Mac** · Mac Studio를 기준으로 개발 | Codex CLI + OrangeDeck **통신 모듈(Connector)** |
| **CachyOS Handheld의 ROG Ally** · 데스크톱 모드 | OrangeDeck **리모컨(UI)** |

이 설명서는 위 구성을 대상으로 합니다. Steam Deck·SteamOS는 설치·동작을 확인하지 않았으며, 다른 Linux 배포판과 Windows는 안내 대상에 포함하지 않습니다.

## 설치하기

**Mac 설치 → 연결 파일 옮기기 → Ally 설치**, 세 단계입니다. 현재 버전은 **0.1.28**입니다.

먼저 두 기기에 [Tailscale](https://tailscale.com/download)을 설치하고 같은 계정으로 로그인하세요. Mac에는 [Codex CLI](https://learn.chatgpt.com/docs/cli)를 설치하고 로그인해 둡니다. 두 기기에 Python 3.9 이상이 필요합니다. [준비물 확인](docs/INSTALL.md#준비물).

두 기기에서 위의 **ZIP 다운로드**를 누르고 압축을 풉니다. Git이나 SSH 키는 필요하지 않습니다.

### 1. Mac에 통신 모듈 설치

**아래 세 파일은 Mac에서 실행합니다.** 압축을 푼 폴더에서 순서대로 두 번 누르세요.

| 순서 | 실행할 파일 | 하는 일 · 창을 닫는 시점 |
| --- | --- | --- |
| **①** | **Setup OrangeDeck.command** | 설치하고 작업 폴더·편집기를 고릅니다. 설치 완료 후 안내에 따라 Enter를 누르고 닫습니다. |
| **②** | **Start OrangeDeck Connector.command** | Mac과 Ally를 연결합니다. **앱을 사용하는 동안 켜 둡니다.** |
| **③** | **Enable Codex Notifications.command** | 완료 알림·승인 요청을 연결합니다. 설정 완료 후 Enter를 누르고 닫습니다. |

③이 끝나면 **평소 사용하는 Mac Codex**에서 **`/hooks` → OrangeDeck 확인·신뢰**를 진행하세요. Mac에서 계속 켜둘 OrangeDeck 창은 **② 통신 모듈 하나**입니다.

Setup이 물으면 Mac의 작업 폴더와 편집기를 고릅니다. 필요한 개발 도구 설치가 뜨면 설치를 마친 뒤 Setup을 다시 실행하세요. [Mac 설치 도움말](docs/INSTALL.md#mac-설치-파일이-안-열릴-때).

### 2. Mac의 연결 파일을 Ally로 옮기기

Mac **Finder → 이동 → 폴더로 이동…**에서 `~/.config/orangedeck`을 엽니다.

그 안의 **`orangedeck-pairing.toml`**을 USB 등으로 **내 Ally의 다운로드 폴더**에 옮기세요. 내 Mac의 주소와 비밀 연결 인증값이 담긴 **개인 연결 파일**입니다. GitHub나 채팅에 올리지 마세요.

### 3. Ally에 리모컨 설치

Ally의 **데스크톱 모드**에서 압축을 푼 폴더를 열고, 빈 곳을 오른쪽 클릭해 **여기서 터미널 열기**를 누릅니다. Mac용 `.command` 파일 대신 아래 명령을 실행하세요.

```sh
sh install.sh --role ui
```

연결 파일을 물으면 옮겨 둔 **`orangedeck-pairing.toml`**을 터미널에 끌어 놓고 Enter를 누릅니다. IP 주소나 인증값을 직접 적을 필요가 없습니다.

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

**업데이트해도 내 설정은 유지됩니다.** 처음 설치할 때 각 기기의 설정과 개인 연결 파일을 준비합니다. 같은 기기에 업데이트할 때는 저장된 작업 폴더·편집기·연결을 그대로 씁니다. 개인 설정은 각 기기의 `~/.config/orangedeck/`에 있고 다운로드에 포함되지 않습니다. [업데이트 순서](docs/INSTALL.md#업데이트) · [설정을 다시 묻지 않는 이유](docs/INSTALL.md#saved-settings).

[간단 사용법·패드 조작](docs/QUICKSTART_KO.md) · [설치·연결 문제 해결](docs/INSTALL.md) · [English guide](README.en.md) · [변경 기록](docs/CHANGELOG.md)
