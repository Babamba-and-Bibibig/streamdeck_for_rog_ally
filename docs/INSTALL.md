<h3>🇰🇷 한국어 · 현재 페이지　|　<a href="INSTALL.en.md">English</a></h3>

# 설치 도움말

처음 설치한다면 [README의 세 단계](../README.md#설치하기)를 따라 하세요. 이 문서는 설치가 막히거나 설정을 바꾸고 싶을 때 보시면 됩니다.

설치 대상은 **macOS를 쓰는 Mac(작업용 Agent) + ROG Ally의 CachyOS Handheld(리모컨)**입니다. Mac 쪽은 Mac Studio를 기준으로 개발했습니다. Steam Deck·SteamOS와 다른 Linux 배포판의 설치는 확인하지 않았습니다. Windows는 지원하지 않습니다.

## 준비물

| 어디에서? | 준비할 것 |
| --- | --- |
| **Mac** | Codex CLI 설치·로그인, Python 3.9 이상 |
| **Ally** | CachyOS Handheld의 데스크톱 모드, Python 3.9 이상 |
| **두 기기** | Tailscale 설치 후 같은 계정으로 로그인, OrangeDeck ZIP 다운로드·압축 풀기 |

Python은 각 기기의 터미널에서 확인합니다.

```sh
python3 --version
```

3.9 이상이면 됩니다. Mac에 없다면 [Python 공식 설치 파일](https://www.python.org/downloads/macos/)을 설치하세요. CachyOS Ally에서는 아래 명령으로 [python 패키지](https://archlinux.org/packages/core/x86_64/python/)를 설치할 수 있습니다.

```sh
sudo pacman -S --needed python
```

Mac의 Codex 로그인 상태는 아래 명령으로 확인합니다. 설치기가 Codex를 대신 설치하거나 로그인하지는 않습니다.

```sh
codex login status
```

[Tailscale 다운로드](https://tailscale.com/download) · [Codex CLI 안내](https://learn.chatgpt.com/docs/cli)

Rust와 나머지 개발 도구는 OrangeDeck 설치기가 설치 여부를 묻습니다. 첫 설치에는 소스를 빌드하는 시간이 필요합니다. Mac의 개발 도구 설치 창이 뜨면 설치를 마친 뒤 Setup을 다시 여세요. **OrangeDeck 설치 명령에는 `sudo`를 붙이지 마세요.**

## Mac 설치 파일이 안 열릴 때

1. **터미널** 앱을 엽니다.
2. `cd `를 입력합니다. `cd` 뒤에 빈칸을 하나 두세요.
3. 압축을 푼 OrangeDeck 폴더를 터미널로 끌어 놓고 Enter를 누릅니다. `install.sh`와 `Cargo.toml`이 들어 있는 폴더입니다.
4. 아래 명령을 실행합니다.

```sh
sh install.sh --role agent
```

설치가 끝나면 [README의 Mac 설치 2번](../README.md#1-mac에-agent-설치)부터 이어가세요. macOS의 보안 기능을 통째로 끌 필요는 없습니다.

## 설치 중 무엇을 입력하나요?

| 질문 | 입력할 것 |
| --- | --- |
| Mac의 작업 폴더 | Codex로 작업하는 실제 폴더. 폴더를 터미널에 끌어 놓아도 됩니다. |
| 기기 이름·프로젝트 이름 | Ally 화면에 보여줄 이름. Enter로 기본값을 써도 됩니다. |
| Codex 실행 파일 | 자동으로 찾지 못했을 때만 묻습니다. Mac 터미널의 `command -v codex`로 위치를 확인하세요. |
| Ally의 연결 파일 | Mac에서 옮겨 온 `orangedeck-pairing.toml`. 파일을 터미널에 끌어 놓으세요. |
| 필요한 도구 설치 | 안내를 읽고 설치하려면 `y`를 입력합니다. |

IP 주소나 인증값을 직접 만들 필요는 없습니다. Mac 설치기가 만든 연결 파일을 Ally에 주면 됩니다. 연결 파일은 개인 파일이므로 공개하지 마세요.

## 다시 켜기

Mac에서 먼저 실행하고 이 터미널을 켜 두세요.

```sh
sh "$HOME/.config/orangedeck/start-agent.command"
```

Ally에서는 아래 명령을 실행합니다.

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

끄려면 Ally 앱 창을 닫고 Mac의 Agent 터미널에서 **Ctrl+C**를 누릅니다. 설치 폴더를 직접 바꿨다면 설치가 끝날 때 나온 경로를 사용하세요. [자세한 사용법](QUICKSTART_KO.md).

## 연결이나 알림이 안 될 때

1. 두 기기에서 **Tailscale이 켜져 있는지**, 같은 계정인지 확인하세요.
2. Mac에서 **Agent 터미널이 켜져 있는지** 확인하세요.
3. 알림이 없다면 **Enable Codex Notifications.command → 평소 Mac Codex의 `/hooks` → OrangeDeck 확인·신뢰** 순서로 확인하세요. Agent를 먼저 켜야 합니다. 알림 연결도 평소 Codex와 같은 사용자 계정에서 설정하세요.
4. Ally에서 아래 연결 검사를 실행하세요.

```sh
sh "$HOME/.config/orangedeck/check-ui.sh"
```

Mac의 Agent가 안 켜지면 아래 명령으로 확인합니다.

```sh
sh "$HOME/.config/orangedeck/check-agent.command"
```

‘주소 사용 중’ 오류는 이미 켜진 Agent가 있을 때 나옵니다. **그 Agent의 터미널에서 Ctrl+C**를 누르고 다시 켜세요. 회사나 직접 관리하는 Tailscale에서 연결을 막았다면 두 기기 사이의 TCP 45831 허용 여부를 확인해야 합니다.

승인 버튼은 Codex가 실제 승인을 요청할 때만 켜집니다. 일반 질문에는 Mac에서 답하세요. 연결 표시가 떠도 알림 설치까지 끝났다는 뜻은 아닙니다. Mac에서 새 작업을 해 보고, Ally의 새 토큰·완료 알림과 실제 승인 처리를 확인하세요. 새 Mac에서 최신 버전으로 이 과정을 마치는 검사는 아직 완료하지 못했습니다.

## 업데이트

1. 두 기기에서 새 ZIP을 받고 **새 폴더**에 압축을 풉니다.
2. **Mac:** 새 폴더의 **Setup OrangeDeck.command**를 실행합니다. **Ally:** 새 폴더에서 `sh install.sh --role ui`를 실행합니다.
3. 켜져 있던 OrangeDeck을 닫고 다시 켭니다. 개인 설정·연결·언어·버튼 배치는 유지됩니다.

**Mac Agent가 0.1.18 이하라면:** 업데이트한 Agent 실행 → **Enable Codex Notifications.command** 실행 → Codex **`/hooks`**에서 다시 확인·신뢰가 필요합니다. 0.1.19부터 알림 연결 파일의 위치가 바뀌었습니다.

처음에 예전 소스 폴더 방식으로 설치했다면 Mac의 `Start OrangeDeck Agent.command`와 Ally의 `./scripts/run-ally.zsh`를 계속 쓸 수 있습니다. Ally 명령은 기존 프로젝트 폴더에서 실행하세요. 실행 파일의 경로가 바뀌면 `/hooks`도 다시 확인하세요.

<a id="host-shortcuts"></a>

## Mac에서 다른 폴더나 웹페이지 열기

Ally의 **에디터·터미널·폴더·웹페이지 열기 버튼은 Mac에서 실행됩니다.** 설치 때 선택한 폴더는 이미 등록되어 있습니다. 대화 목록에 보인다는 이유만으로 다른 폴더를 열 수 있는 것은 아닙니다.

다른 폴더도 쓰려면 **Mac의 개인 설정 파일** `~/.config/orangedeck/agent.toml`에 아래 내용을 추가하세요. `id`는 다른 항목과 겹치지 않게, `path`는 실제 작업 폴더로 바꿉니다. 웹페이지가 필요 없으면 `browser_url` 줄을 빼세요.

```toml
[[projects]]
id = "another-project"
name = "My Website"
path = "/Users/YOU/Code/my-website"
browser_url = "http://127.0.0.1:3000"
```

웹 주소는 HTTP/HTTPS만 됩니다. 위 주소는 Mac에서 이미 켜 둔 개발용 웹 서버의 예시입니다. 저장 후 Mac의 Agent를 껐다 켜고 Ally에서 그 폴더를 고르세요. 연결 파일을 다시 옮길 필요는 없습니다.

에디터는 Mac의 `/Applications` 또는 `~/Applications`에 있는 **Zed → VS Code → VSCodium** 순서로 찾습니다. 터미널은 Mac의 **Terminal**을 엽니다. 해당 앱이 없으면 설치해야 합니다.

## 설정 파일은 어디에 있나요?

기본 위치는 두 기기 모두 **`~/.config/orangedeck`**입니다. 이 안의 파일은 개인 설정입니다.

| 파일 | 용도 |
| --- | --- |
| Mac의 `agent.toml` | 작업 폴더·Codex 실행 파일 설정 |
| Ally의 `config.toml` | 연결할 Mac 설정 |
| `orangedeck-pairing.toml` | Mac에서 Ally로 옮기는 비밀 연결 파일 |
| `agent.token` / `ui.token` | 연결에 쓰는 비밀 인증값 |
| Ally의 `ui-preferences.toml` | 자동 저장되는 언어·버튼 배치 |
| `start-agent.command` / `start-ui.sh` | 다시 켤 때 쓰는 실행 파일 |
| `enable-notifications.command` | Mac의 알림 연결 설정 |
| `check-agent.command` / `check-ui.sh` | 문제가 생겼을 때 확인하는 실행 파일 |

GitHub에서 받은 예제 파일에 개인정보를 적지 마세요. 알림 설정은 기존 Codex 설정을 보존하고 `hooks.json`을 백업한 뒤 OrangeDeck 항목을 추가합니다.

## 필요한 경우에만 바꾸는 설치 옵션

보통은 옵션을 바꿀 필요가 없습니다. **`agent`는 Mac, `ui`는 Ally**를 뜻합니다.

| 옵션 | 뜻 |
| --- | --- |
| `--project-path`, `--project-name`, `--host-name` | 처음 설치할 작업 폴더·화면에 보일 이름 |
| `--codex-binary` | Mac의 Codex 실행 파일 위치를 직접 지정 |
| `--pairing` | 처음 연결할 Ally에 가져온 연결 파일 위치 지정 |
| `--config-dir` | 기본 설정 폴더 대신 다른 폴더 사용 |
| `--port` | 기본 연결 포트 45831 변경 |
| `--install-rust`, `--install-deps` | 안내에 나오는 도구 설치를 미리 허용 |
| `--non-interactive` | 질문하지 않고 지정한 값 사용. 값이 부족하면 중단 |
| `--start` / `--check` | 설치 후 바로 실행 / 설치하지 않고 준비물만 확인 |

이미 설치했다면 기존 프로젝트와 연결 설정을 유지합니다. 처음 설치할 때 쓰는 옵션으로 기존 설정이 덮어써지지는 않습니다. `XDG_CONFIG_HOME`이나 `--config-dir`을 바꿨다면 설치기가 알려준 실행 파일을 사용하세요. 다른 Codex 설정 폴더(`CODEX_HOME`)를 쓰는 사람은 평소 쓰는 터미널 환경에서 설치하세요. 설치기는 그 위치를 저장하고 업데이트할 때도 유지합니다.

## 연결 파일을 잃어버렸거나 설정을 바꿔야 할 때

설정 파일 없이 인증 파일만 남아 있으면 설치기가 멈춥니다. 오류를 없애려고 인증 파일을 지우거나 `--force`를 붙이지 마세요. 기존 설정을 복원하거나 새 설정 폴더를 선택해야 합니다.

Mac 주소가 바뀌었다면 Mac의 `orangedeck-agent pairing --config /path/to/agent.toml --output /path/to/private-pairing.toml`로 연결 파일을 다시 만들고, Ally에서 `orangedeck-ui pair --config /path/to/config.toml --bundle /path/to/private-pairing.toml --force`로 가져옵니다. `/path/to/...`는 자신의 실제 파일 위치로 바꿔야 합니다.

인증값이 공개됐다면 먼저 Agent를 끄세요. `orangedeck-agent init --force`는 인증값뿐 아니라 Mac의 초기 프로젝트 설정도 다시 만듭니다. 기존 프로젝트 설정을 확인·보관한 뒤 새 연결 파일로 다시 연결해야 합니다. [공개 파일 문제 대응](PUBLICATION.md)을 참고하세요.

## 제거

Ally 앱과 Mac Agent를 먼저 끕니다. Codex `/hooks`에서 OrangeDeck 연결을 해제하고 OrangeDeck 전용 설치 폴더와 만든 바로가기를 지웁니다. 다른 앱의 설정 폴더, Codex 대화, 프로젝트 폴더는 지우지 마세요. Codex·Tailscale·Rust는 각각 따로 설치한 프로그램입니다.
