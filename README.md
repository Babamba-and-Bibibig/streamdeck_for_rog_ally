<h3 align="center">🇰🇷 한국어 · 현재 페이지　|　<a href="README.en.md">🌐 English</a></h3>

# OrangeDeck

**Codex 작업을 옆 화면에서 확인하는 네이티브 보조 앱입니다.** Mac에서 평소처럼 Codex를 사용하면서 Linux 데스크톱이나 휴대용 기기에서 현재 대화, 토큰, 남은 한도와 승인 요청을 확인합니다.

[화면 둘러보기](#다섯-탭-둘러보기) · [바로 설치하기](#설치하기) · [간단 사용법](docs/QUICKSTART_KO.md)

![OrangeDeck LIVE 실제 구동 화면: 현재 질의 토큰, 남은 한도와 프로젝트 작업 현황](docs/screenshots/live.png)

*작업은 메인 PC에서, 진행 상황은 손안의 보조 화면에서. 아래는 개인정보가 없는 모의 프로젝트·대화로 실행한 실제 네이티브 앱 화면입니다.*

## 다섯 탭 둘러보기

위 **LIVE** 화면에서 현재 질의의 토큰과 남은 한도를 한눈에 확인합니다. 아래 네 탭도 같은 프로젝트·대화 선택을 공유합니다. 이미지를 누르면 원본 크기로 볼 수 있습니다.

<table>
<tr>
<td width="50%">
<b>02 · 단축키</b><br>
<a href="docs/screenshots/shortcuts.png"><img src="docs/screenshots/shortcuts.png" alt="모의 승인 요청에 승인·거절 키가 활성화된 단축키 화면" width="640"></a><br>
터치하기 편한 5 × 2 키. 표시된 요청 한 건을 승인·거절합니다. 나머지 8개 키에는 14가지 기능 중 원하는 것을 연결합니다.
</td>
<td width="50%">
<b>03 · 프로젝트들</b><br>
<a href="docs/screenshots/projects.png"><img src="docs/screenshots/projects.png" alt="모의 프로젝트 폴더와 대화 수, 최근 활동을 보여주는 프로젝트들 화면" width="640"></a><br>
프로젝트를 찾고 모든 탭의 작업 범위를 함께 전환합니다.
</td>
</tr>
<tr>
<td width="50%">
<b>04 · 대화</b><br>
<a href="docs/screenshots/conversations.png"><img src="docs/screenshots/conversations.png" alt="선택한 모의 대화를 고정하고 같은 프로젝트의 대화 목록을 확인하는 화면" width="640"></a><br>
확인할 대화를 고정하거나 가장 최근 활동을 자동으로 따라갑니다.
</td>
<td width="50%">
<b>05 · 알림</b><br>
<a href="docs/screenshots/notifications.png"><img src="docs/screenshots/notifications.png" alt="모의 현재 질문·응답과 명령 승인 요청 상세를 보여주는 알림 화면" width="640"></a><br>
현재 질문·응답과 승인 요청의 내용을 읽고 직접 결정합니다.
</td>
</tr>
</table>

**ROG Ally나 Steam Deck을 보조 화면으로 쓰고 싶다면**, 먼저 Linux 데스크톱 환경이 필요합니다. 위 이미지는 Linux 앱을 로컬 모의 데이터로 촬영한 것이며 SteamOS 설치는 아직 검증하지 않았습니다. [지원 환경](#지원-환경)과 [설치 방법](#설치하기)을 확인하세요. [촬영 정보](docs/SCREENSHOTS.md). 단축키 사진은 **추천 구성**을 적용한 예시입니다.

OpenAI·ASUS·Elgato의 공식 제품이 아닌 독립 커뮤니티 프로젝트입니다. **0.1.21 초기 버전**입니다. 화면 오른쪽 위 **한국어 / EN**을 누르면 모든 탭의 안내 언어가 즉시 바뀝니다. 한국어가 기본이며 언어와 키 배치는 자동 저장됩니다.

## 기능과 특징

| 화면 | 기능 |
| --- | --- |
| **LIVE** | 선택한 프로젝트의 최근 대화를 따라가며 현재 질의/모델 요청 토큰, 5시간·주간 남은 한도, 서버 제공 계정 통계를 확인합니다. |
| **단축키** | 정사각형 5×2 키 중 **01 승인 / 02 거절**을 사용합니다. 현재 질의에 새 승인 요청이 오면 한 번 자동 이동합니다. 나머지 8개 키에는 14가지 기능 중 원하는 것을 연결합니다. |
| **프로젝트들** | Codex 대화 기록에서 발견한 폴더를 보고 모든 탭의 프로젝트를 함께 전환합니다. |
| **대화** | 특정 대화를 고정하거나 같은 프로젝트의 최근 활동을 자동으로 따라갑니다. |
| **알림** | 현재 질문·응답·완료와 승인 요청 상세를 확인하고, 화면에 표시된 요청 한 건에 직접 응답합니다. |

- 코랄·보라·민트 포인트의 어두운 테마, 직접 그린 아이콘과 손가락으로 누르기 편한 정사각형 키.
- **+ → 기능 선택**만으로 만드는 나만의 덱. **추천 구성**으로 빈 키를 한 번에 채우고, **키 편집**으로 변경·삭제합니다.
- Rust로 만든 네이티브 화면, 터치·마우스·컨트롤러 입력, HTTP/WebSocket 갱신과 재연결 보호.
- 큰 토큰 숫자, 수신 지연·연결 끊김 구분, **100% − 공식 사용률**로 계산한 남은 한도. 미사용은 100%입니다.
- 현재 질의의 알림은 확인 전까지 유지됩니다. 실행할 수 없는 승인 키는 회색, 승인 대기일 때만 점멸합니다.
- Tailscale의 암호화 연결과 별도의 무작위 페어링 토큰을 사용합니다.
- Codex 계정·원격 기기 없이 화면을 개발할 수 있는 로컬 모의 모드가 있습니다.

토큰은 완료된 모델 요청이 기록될 때 갱신되며 매 토큰 실시간 증가가 아닙니다. 계정별 서버 응답에 없는 기간·수치를 만들거나 비율을 토큰 개수로 환산하지 않습니다. 계정 통계의 전체 집계 범위도 보장하지 않습니다.

## 지원 환경

| 구성 | 조건 |
| --- | --- |
| Agent | Codex CLI가 설치된 macOS. Linux Agent도 사용할 수 있습니다. Mac 앱 열기 동작은 macOS 앱을 사용합니다. |
| 화면 앱 | Wayland/X11·OpenGL 데스크톱을 사용하는 Linux. Linux를 설치한 ROG Ally도 해당합니다. |
| 연결 | 두 기기가 신뢰하는 같은 Tailscale 네트워크에 로그인되어 있고 TCP 45831 통신이 허용되어야 합니다. |
| 빌드 | 설치기는 Python 3.9 이상, Rust 1.95.0과 운영체제 개발 라이브러리를 사용합니다. 배포 도구는 Python 3.11 이상이 필요합니다. |

**Windows 기본 상태의 ROG Ally, Windows·모바일 앱, macOS 화면 앱 설치는 현재 지원하지 않습니다.** 각 배포판과 새 Mac에서 설치·동작 검증이 필요합니다. 새 알림의 실사용과 실제 Mac의 자연 발생 승인 왕복은 아직 완전히 검증되지 않았으며 모의 검사 통과와 구별합니다.

## 설치하기

Codex를 평소 사용하던 **Mac Studio·Mac·Linux 메인 PC에는 Agent**, 보조 화면으로 쓸 **Linux ROG Ally·Linux 데스크톱에는 UI**를 설치합니다. Windows 지원은 추후 진행합니다.

```text
내 Mac / Linux 메인 PC                  내 Linux ROG Ally / 데스크톱
Codex + OrangeDeck Agent   ← Tailscale →   OrangeDeck UI
```

**두 기기 모두** [이 GitHub 저장소](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally)에서 **Code → Download ZIP**을 누르고 압축을 완전히 풉니다. 공개 ZIP 다운로드에는 Git 설치나 GitHub SSH 키가 필요하지 않습니다. 빌드용 소스 폴더는 보관하고, 앱과 개인 설정은 별도 사용자 설정 폴더에 설치합니다.

아래 명령을 실행할 때는 터미널에 `cd `를 입력하고 **한 칸 띄운 뒤 압축을 푼 폴더를 끌어 놓고 Enter**를 누르세요. `install.sh`와 `Cargo.toml`이 들어 있는 폴더여야 합니다. Linux는 파일 관리자의 **여기서 터미널 열기**도 사용할 수 있습니다.

먼저 두 기기에 [Tailscale](https://tailscale.com/download)을 설치하고 같은 네트워크에 로그인하세요. Agent 기기에는 [Codex CLI](https://learn.chatgpt.com/docs/cli)를 설치하고 로그인합니다. 계정 로그인·운영체제 동의·Codex 훅 신뢰는 본인이 직접 확인해야 합니다.

터미널에서 `python3 --version`을 실행해 **Python 3.9 이상**인지 확인하세요. 없다면 [Mac용 Python 설치 파일](https://www.python.org/downloads/macos/) 또는 Linux 배포판의 `python3` 패키지로 설치합니다. Agent 기기의 `codex login status`도 성공해야 합니다. Rust와 필요한 빌드 도구는 설치 중 안내합니다. **첫 빌드는 몇 분 이상 걸릴 수 있으니 터미널을 켜 두세요.** 설치 명령 앞에 `sudo`를 붙이지 않습니다.

### 1. Mac / Agent 기기

Mac에서는 **Setup OrangeDeck.command**를 더블클릭하거나 아래 명령을 실행합니다.

```sh
sh install.sh --role agent
```

질문이 나오면 다음과 같이 입력합니다.

| 질문 | 입력할 값 |
| --- | --- |
| 프로젝트 폴더 | 이 메인 PC에 있는, 본인이 신뢰하는 실제 작업 폴더. 터미널에 폴더를 끌어 놓아도 됩니다. |
| 기기 표시 이름 | `MY MAC`처럼 화면에 표시할 이름. Enter를 누르면 기본값을 사용합니다. |
| 프로젝트 표시 이름 | 화면에 표시할 프로젝트 이름. Enter를 누르면 폴더 이름을 사용합니다. |
| 필요한 도구 설치 여부 | 표시된 설치 내용을 읽고 진행하려면 `y`를 입력합니다. Mac 개발 도구 설치 창이 뜨면 설치를 마친 뒤 같은 명령을 다시 실행합니다. |

Codex 실행 파일·Tailscale 주소는 자동으로 찾습니다. Codex를 찾지 못한 경우에만 실행 파일 경로를 추가로 묻습니다. 빌드·개인 설정·무작위 토큰·페어링 파일·실행기 생성은 자동입니다. 업데이트할 때는 기존 설정과 토큰을 유지합니다.

설치기가 출력하는 경로를 따라 다음 순서로 진행합니다.

1. 생성된 **start-agent.command**로 Agent를 실행하고 터미널을 켜 둡니다.
2. 다른 터미널에서 **enable-notifications.command** 또는 소스 폴더의 **Enable Codex Notifications.command**를 실행합니다.
3. 평소 사용하는 Mac Codex에서 **`/hooks`**를 열어 OrangeDeck 정의를 검토하고 신뢰합니다. [공식 훅 안내](https://learn.chatgpt.com/docs/hooks).
4. 생성된 **orangedeck-pairing.toml**을 Tailscale Taildrop이나 오프라인 방법으로 화면 기기에 전달합니다. **비밀 토큰이 들어 있으므로 GitHub·채팅에 올리면 안 됩니다.**

기본 설치라면 아래 명령을 그대로 복사하면 됩니다. 먼저 Agent를 켜고 **이 터미널은 열어 두세요**.

```sh
sh "$HOME/.config/orangedeck/start-agent.command"
```

**새 터미널을 하나 더 열어** 알림을 설치합니다.

```sh
sh "$HOME/.config/orangedeck/enable-notifications.command"
```

그다음 평소 Codex에서 `/hooks`를 검토·신뢰합니다. 페어링 파일은 Mac **Finder → 이동 → 폴더로 이동…**에 `~/.config/orangedeck`를 입력하면 찾을 수 있습니다. `orangedeck-pairing.toml`을 골라 [Tailscale Taildrop](https://tailscale.com/docs/features/taildrop) 또는 USB로 자신의 Linux 기기에 전달하세요. Linux 메인 PC에서는 파일 관리자의 숨김 폴더 표시를 켜고 `.config/orangedeck`로 이동합니다. 받은 파일은 Downloads처럼 찾기 쉬운 폴더에 두세요.

### 2. Linux 화면 기기

압축을 푼 소스 폴더에서 실행합니다.

```sh
sh install.sh --role ui
```

페어링 파일을 물으면 **전달받은 파일의 전체 경로**를 붙여넣거나 파일을 터미널에 끌어 놓고 Enter를 누릅니다. UI 빌드·설치·연결 설정 가져오기는 자동입니다. IP·토큰을 직접 입력할 필요는 없습니다. Agent를 켜 둔 상태에서 화면 앱을 실행하세요.

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

**연결됨**(영어: CONNECTED)이 표시되면 프로젝트·대화를 고르고, 평소 Codex 작업에서 새 토큰과 완료 알림을 확인합니다. 실제 승인 요청이 발생하면 내용을 읽고 직접 결정해 Codex에 반영되는지도 확인하세요. 연결 표시만으로 알림 설정까지 확인된 것은 아닙니다.

위 명령은 두 기기 모두 기본 설치 위치 `~/.config/orangedeck` 기준입니다. `XDG_CONFIG_HOME`·`--config-dir`을 바꿨다면 **설치기가 출력한 경로**를 사용하세요. 별도 Codex 프로필을 사용한다면 평소 `CODEX_HOME`을 지정한 상태에서 Agent를 설치하면 이후 업데이트에도 유지합니다. [설치 옵션·제거 방법](docs/INSTALL.md)을 참고하세요.

## 사용하기

생성된 Agent·UI 실행기를 켜고 프로젝트와 대화를 고릅니다. **자동 ON**은 같은 프로젝트의 최근 대화를 따라가는 상태입니다. 대화를 직접 선택하면 고정되며 **최근 자동**으로 돌아갈 수 있습니다.

단축키 탭에서는 방향키/스틱으로 키를 고르고, 물리 **A**는 선택한 키를 실행하며 **B**는 표시된 승인 요청을 거절합니다. 긴 요청은 **요청 상세**를 열고 안쪽을 스크롤해 끝까지 읽은 뒤 결정하세요. Agent 0.1.21부터 전달받은 명령·권한·파일 변경 내용을 상세 보기에서 생략하지 않습니다. Enter/Escape/Space로 승인을 전송하지 않습니다. 알림을 닫아도 그 뒤에 가려진 승인은 처리되지 않습니다. 일반 질문에는 Mac Codex에서 답합니다.

UI 창을 닫으면 화면 앱이 종료됩니다. Agent는 작업이 끝난 뒤 해당 터미널의 **Ctrl+C**로 끕니다. 다시 켤 때는 같은 실행기만 사용하며 설치·페어링·훅 설정을 반복할 필요가 없습니다. [전체 조작 안내](docs/QUICKSTART_KO.md).

## 내 덱 만들기 · 언어 바꾸기

1. 오른쪽 위 **한국어 / EN**을 눌러 언어를 고릅니다. 재시작 없이 모든 탭이 바뀌며 실제 질문·응답과 프로젝트 이름은 원문을 유지합니다.
2. **단축키 → 빈 + 키 → 원하는 기능**을 누르면 저장됩니다. 기능은 이후 해당 키를 누를 때 실행합니다.
3. 처음에는 **추천 구성**을 누르세요. 새로고침·최근 자동·대화·알림·이전/다음 대화·에디터·터미널을 빈 자리에 채웁니다. 이미 설정한 키는 유지합니다.
4. 바꾸려면 **키 편집 → 바꿀 키**, 없애려면 편집창의 **키 비우기**를 누릅니다. 끝나면 **편집 끝**을 누릅니다. 마우스 오른쪽 클릭으로도 키를 편집할 수 있습니다.

| 연결할 기능 | 하는 일 |
| --- | --- |
| LIVE / 프로젝트 / 대화 목록 / 알림 보기 | 원하는 탭으로 바로 이동 |
| 새로고침 / 최근 자동 | 메인 PC에서 대화 정보를 다시 읽기 / 같은 프로젝트의 최신 대화 추적 |
| 이전·다음 프로젝트 / 이전·다음 대화 | 현재 작업 범위 또는 고정할 대화 전환 |
| 에디터 / 터미널 / 폴더 열기 | **메인 PC에서** Agent에 등록한 프로젝트 열기 |
| 웹페이지 열기 | 메인 PC에서 해당 프로젝트의 `browser_url` 열기 |

**에디터 열기:** 메인 PC에 Zed, VS Code 또는 VSCodium 중 하나가 필요합니다. Mac은 `/Applications` 또는 사용자 `Applications`에서, Linux는 실행기 PATH에서 찾습니다. **터미널 열기:** Mac은 Terminal, Linux는 Konsole·GNOME Terminal·Xfce Terminal 중 설치된 앱을 사용합니다. 이 네 가지 열기 기능은 Codex 기록에서 보기만 가능한 미등록 폴더에는 실행되지 않습니다. 웹 주소와 추가 프로젝트 등록은 [Agent 설정 안내](docs/INSTALL.md#host-shortcuts)를 참고하세요.

![한국어 키 편집창: 목록에서 기능을 고르면 해당 키에 저장됩니다](docs/screenshots/key-editor.png)

01 승인·02 거절은 고정입니다. 편집창에서 **A는 기능 저장, B는 취소**이며 뒤의 승인 요청에는 응답하지 않습니다. 언어·8개 키 설정은 UI 설정 파일 옆의 **`ui-preferences.toml`**에 저장됩니다. 기본 위치는 `~/.config/orangedeck/ui-preferences.toml`이며 직접 편집할 필요가 없습니다. 페어링 토큰과 Codex 설정은 별개입니다. 데모의 언어·키 변경은 해당 실행에서만 유지됩니다.

## 문제 해결과 업데이트

| 증상 | 확인할 것 |
| --- | --- |
| Mac `.command`가 안 열림 | 압축을 푼 폴더에서 터미널을 열고 `sh install.sh --role agent`를 실행합니다. |
| 설치 중 필수 도구가 없다고 나옴 | 표시된 설치 안내를 완료한 뒤 같은 설치 명령을 다시 실행합니다. |
| UI 연결이 안 됨 | 두 기기의 Tailscale과 Agent 실행 상태를 확인하고, 화면 기기에서 `sh "$HOME/.config/orangedeck/check-ui.sh"`를 실행합니다. |
| Agent가 안 켜짐 | 메인 PC의 다른 터미널에서 `sh "$HOME/.config/orangedeck/check-agent.command"`로 진단합니다. 기존 Agent가 포트를 사용 중이면 그 Agent 터미널에서 Ctrl+C로 종료합니다. |
| 연결됐지만 알림·승인이 없음 | 알림 실행기 → `/hooks` 검토·신뢰와 동일한 Codex 사용자/프로필을 확인합니다. 승인이 필요한 동작에서만 승인 요청이 생깁니다. |

업데이트는 새 소스를 받아 **새 폴더에 압축을 푼 뒤**, 해당 기기에서 기존과 같은 `sh install.sh --role agent` 또는 `--role ui`를 실행하면 됩니다. 개인 설정·페어링·Codex 프로필·키 배치·언어는 유지합니다. 기존 OrangeDeck을 닫고 다시 실행해야 새 버전이 켜집니다.

**Agent 0.1.18 이하 → 0.1.19 이상 업데이트:** Agent를 다시 켠 뒤 `enable-notifications.command`를 한 번 더 실행하고 `/hooks`를 다시 검토·신뢰하세요. 새 Mac·사용자 지정 설치에서 발생하던 오류를 고치기 위해 알림 소켓이 Agent 설정 폴더를 따르도록 변경했습니다. 사용자 지정 경로는 설치기가 출력한 실행기를 사용합니다.

## 개인정보와 보안

Agent는 연결한 화면에 프로젝트 경로·대화 제목·선택한 질문/응답·승인 도구 인자·사용 통계를 보냅니다. 스크린샷과 진단 출력도 개인정보를 포함할 수 있습니다. Codex 인증 정보는 Agent 기기에 남습니다. OrangeDeck에는 별도 분석 서버가 없으며 작업 내용을 GitHub에 전송하지 않습니다.

인증된 API에는 현재 화면 탭에 없는 고정 Cargo 명령과 OrangeDeck 소유 대화 제어도 구현되어 있습니다. Cargo 빌드 스크립트와 직접 승인한 도구는 사용자 권한으로 코드를 실행할 수 있으므로 신뢰하는 기기·프로젝트만 연결하세요. 임의 셸 실행 API나 자동 승인은 없습니다.

공개 배포에서는 개인 설정·인증 파일·실행 기록·개인 인수인계를 제외합니다. 게시 전에 검사하세요.

```sh
python3 scripts/check_public.py
python3 scripts/check_public.py --tracked --history
```

두 번째 명령은 전체 기록이 있는 Git 저장소에서 추적 파일, 과거 파일명·내용, 커밋·태그 메타데이터까지 검사합니다. `python3 download-page/manage.py export`로 검사된 소스만 담은 `dist/github/OrangeDeck-v버전/` 폴더와 파일별 해시 목록을 만들 수 있습니다. **업로드 전에 로컬에서 검사하세요.** GitHub Actions는 파일이 GitHub에 도착한 뒤 실행됩니다. 검사 통과가 모든 비밀값 부재를 보장하지는 않습니다. 현재 파일을 지워도 과거 커밋·공개 ZIP에서 사라지지는 않습니다. [GitHub 게시와 노출 대응](docs/PUBLICATION.md).

## 개발과 라이선스

소스 폴더에서 아래 명령으로 검사·빌드하고 모의 화면을 실행할 수 있습니다.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
python3 scripts/check_architecture.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 download-page/test_manage.py
cargo build --release --workspace --locked
./scripts/run-demo.zsh
# 영어 UI와 영어 모의 대화로 실행하려면:
./scripts/run-demo.zsh en
```

[설계 구조](docs/ARCHITECTURE.md) · [변경 내역](docs/CHANGELOG.md). 모의 화면은 **SIMULATED MAC / LOCAL MOCK**으로 표시합니다. Codex CLI 스키마의 기록된 기준은 0.153.2이며 다른 버전은 경고와 실행 중 호환성 검사 대상으로 취급합니다.

[MIT 라이선스](LICENSE)로 배포합니다.
