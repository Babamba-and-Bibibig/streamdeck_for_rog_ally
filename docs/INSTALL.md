# Installation / 설치 상세

[English overview](../README.en.md#install) · [한국어 안내](../README.ko.md#설치하기)

## Before running setup / 준비

Use a normal user account, not `sudo sh install.sh`. Agent and UI installation support macOS/Linux Agent and Linux desktop UI. On macOS install Apple's Command Line Tools; the guided installer can open `xcode-select --install`, but you must finish the Apple dialog and rerun. Python 3.9+ must already be available. On Linux install `python3` with your distribution package manager if missing.

Both machines need Tailscale installed, signed in and able to reach each other. The Agent machine also needs an installed, signed-in Codex CLI. Setup checks `codex login status` without printing captured account output. It does not install or upgrade Codex, log into either account or modify tailnet policy. These integrations need the user's own account and operating-system consent.

필수 계정 로그인과 운영체제 확인을 마친 뒤 실행하세요. 설치기는 두 계정에 대신 로그인하지 않습니다. Codex는 평소 사용하던 사용자와 프로필을 사용합니다.

## What setup changes / 설치 범위

`sh install.sh` asks which role to install on Linux; macOS defaults to Agent. It offers missing native build dependencies and the official Rust installer before running them. You may decline and install them yourself. Downloads/builds take time and use disk space; the source build is not a precompiled application.

Setup builds the selected component with `cargo +1.95.0 build --locked --release`. The generated app, token and launchers are private per-user files. Existing configuration is reused and its token is preserved. Re-running setup updates the installed binary. Stop the existing Agent yourself before launching the updated Agent; setup does not kill existing processes.

If a config is missing but a token or pairing file remains, setup stops before installing instead of overwriting credentials. Restore/review the original configuration or choose a new private directory. Native `init`/`pair` also check every destination and require an explicit `--force` to replace existing credentials; symlinks, special files and colliding destinations are refused even with that flag.

설정 파일 없이 토큰·페어링 파일만 남아 있으면 설치를 중단하고 보존합니다. 기존 설정을 복원·확인하거나 새 개인 설정 폴더를 선택하세요. 오류를 없애려고 토큰을 지우거나 `--force`를 무작정 붙이지 마세요.

| Default path under `~/.config/orangedeck` | Purpose |
| --- | --- |
| `bin/orangedeck-agent` or `bin/orangedeck-ui` | Installed native executable |
| `agent.toml` / `config.toml` | Agent / UI configuration |
| `ui-preferences.toml` | UI language and eight custom keys; private, auto-saved, preserved on update |
| `agent.token` / `ui.token` | Secret application credential, mode 0600 |
| `orangedeck-pairing.toml` | Secret bundle to transfer privately to the UI |
| `start-agent.command` / `start-ui.sh` | Start the component using the exact configured paths |
| `enable-notifications.command` | Explicit hook installer for the selected Codex executable |
| `check-agent.command` / `check-ui.sh` | Agent diagnostics / read-only display connection verification |
| `hooks/codex.sock` / `owned-codex-threads.json` | Runtime state scoped to the selected Agent configuration |
| `OrangeDeck.desktop` | UI shortcut generated with your path |
| `install-agent.json` / `install-ui.json` | Private installation metadata; no token contents |

Use the paths printed by setup if `XDG_CONFIG_HOME` or `--config-dir` is customized. Copy the generated UI shortcut to your desktop/application menu if desired; desktop environments may ask you to trust that local launcher. Source-root wrappers use the default/XDG installation; custom directories use their printed launchers directly.

Generated launchers preserve the executable search path, selected Tailscale binary and selected `CODEX_HOME` (default `~/.codex`). Updates recover the previous profile even when a new shell does not set that variable. An explicitly supplied `CODEX_HOME` selects a different profile. Other shell environment overrides are not recorded. Configuration, shell profiles and credentials belonging to Codex remain untouched. Installing notifications separately merges `hooks.json` with a private backup; **start Agent → enable notifications → `/hooks` review/trust**.

## Settings / 입력값

| Option | Description |
| --- | --- |
| `--role agent` / `--role ui` | Component to install |
| `--project-path '/path/to/project'` | Existing trusted project folder on the Agent; required for a new unattended Agent install |
| `--project-name 'My Project'` | Project display label |
| `--host-name 'CODEX HOST'` | Host display label; does not rename the computer |
| `--port 45831` | Agent's Tailscale TCP port, 1–65535 |
| `--codex-binary '/path/to/codex'` | Override Codex discovery; useful for nonstandard installations |
| `--pairing '/path/to/orangedeck-pairing.toml'` | Private bundle from Agent, for a new UI pairing |
| `--config-dir '/path/to/private/config'` | Override the per-user installation directory |
| `--install-rust` | Explicit permission to download/install rustup when missing |
| `--install-deps` | Explicit permission to run the displayed OS package installation |
| `--non-interactive` | Require provided/default settings and stop instead of prompting |
| `--start` | Start the configured app in this terminal after setup |
| `--check` | Show platform and prerequisite availability without installing or contacting Codex |

Example settings contain no shared credential:

```sh
sh install.sh --role agent --project-path "$HOME/Code/my-project" --host-name 'My Codex Mac'
sh install.sh --role ui --pairing "$HOME/Downloads/orangedeck-pairing.toml"
```

The UI installer restricts the received bundle to private permissions, then the Rust importer validates its protocol, URL and token. Keep only the pairing copies you need; never commit or publicly share them. An existing UI configuration plus `--pairing` is deliberately refused to avoid silently changing its trust relationship.

## Native packages / 빌드 패키지

The installer recognizes `apt-get`, `dnf` and `pacman`. It shows the exact package command and asks before using `sudo`, unless `--install-deps` was explicitly given. Other distributions require manual package installation.

For Debian/Ubuntu UI builds the package set includes `build-essential`, `pkg-config`, `libudev-dev`, `libxkbcommon-dev`, `libwayland-dev`, `libx11-dev`, `libxi-dev`, `libgl1-mesa-dev`, `libdbus-1-dev` `libnotify-bin` and `fonts-noto-cjk`. The installer contains corresponding Fedora/Arch package lists. Runtime needs a desktop session with graphics drivers; controller/notification support also depends on the desktop and available input device permissions.

The installer also checks for the Noto Sans CJK font used by Korean labels. Package names and paths are documented by [Ubuntu](https://packages.ubuntu.com/noble/all/fonts-noto-cjk/filelist), [Fedora](https://packages.fedoraproject.org/pkgs/google-noto-sans-cjk-fonts/google-noto-sans-cjk-fonts/) and [Arch](https://archlinux.org/packages/extra/any/noto-fonts-cjk/files/). If Korean appears as squares, install that distribution's package (`fonts-noto-cjk`, `google-noto-sans-cjk-fonts` or `noto-fonts-cjk`) and restart the UI. The English toggle remains available.

**한국어가 네모로 보이면:** 설치기를 다시 실행해 글꼴 설치 안내를 따르고 UI를 다시 켜세요. Debian/Ubuntu는 `fonts-noto-cjk`, Fedora는 `google-noto-sans-cjk-fonts`, Arch는 `noto-fonts-cjk`를 사용합니다.

Immutable handheld distributions may restrict package installation. Setup does not disable read-only system protection or change controller permissions. Use that distribution's supported development environment. This repository does not promise successful installation on every Linux image.

Official build reference: [Rust installation and native linker requirements](https://doc.rust-lang.org/book/ch01-01-installation.html).

## Update / 업데이트

Download a new source version, extract it into a separate folder, and rerun the installer for the same role and config directory. It builds the new source and atomically replaces the installed executable. Existing tokens, projects, pairing and UI preferences are preserved. The previous process keeps running until you close it; the next launch uses the updated binary.

The newly built executable validates an existing configuration and credential before replacing the installed binary. Agent updates check the Codex executable configured in that file and preserve the previous Codex profile. `orangedeck-agent check-config --config ...` and `orangedeck-ui check-config --config ...` perform that local validation without starting services or contacting accounts.

**0.1.19 Agent migration:** restart the Agent, rerun the generated notification installer, then review/trust `/hooks` again. The socket and owned-thread registry now live beside the selected Agent configuration, fixing fresh/custom macOS setup. If your previous registry lived elsewhere, its conversations remain read-only under the new registry; registries are not automatically merged across configurations.

**0.1.19로 Agent 업데이트:** Agent 재실행 → 알림 설치 실행기 재실행 → `/hooks` 재검토·신뢰를 한 번 진행하세요. 새 알림 소켓은 실제 Agent 설정 폴더를 사용합니다. 기존 소유 대화 기록이 다른 폴더에 있으면 자동 병합하지 않으며 해당 대화는 읽기 전용으로 표시합니다.

If using the older source-folder installation, `scripts/run-ally.zsh`, `Start OrangeDeck Agent.command` and `Enable Codex Notifications.command` remain available. The guided installer can adopt the existing `~/.config/orangedeck` configuration. Review `/hooks` again when the hook definition or executable path changes. Never assume build success proves real Mac notifications or approval delivery.

<a id="host-shortcuts"></a>

## 메인 PC 열기 기능 · Host shortcuts

**단축키의 에디터·터미널·폴더·웹페이지 열기**는 메인 PC의 Agent에 등록된 프로젝트를 대상으로 합니다. Codex 기록에서 발견한 폴더를 보기만 해서는 실행 권한이 추가되지 않습니다. 설치할 때 입력한 프로젝트는 이미 등록되어 있습니다.

추가하려면 메인 PC에서 **개인 설정** `~/.config/orangedeck/agent.toml`을 열어 아래 형식의 `[[projects]]` 블록을 덧붙입니다. `id`는 기존 항목과 겹치지 않게, `path`는 **그 메인 PC의 실제 프로젝트 폴더**로 바꾸세요. 기존 프로젝트의 웹 주소만 추가할 때는 그 프로젝트 블록 안에 `browser_url` 한 줄을 넣습니다. 공개 `config/agent.example.toml`에는 개인정보를 쓰지 마세요.

```toml
[[projects]]
id = "another-project"
name = "My Website"
path = "/Users/YOU/Code/my-website"
browser_url = "http://127.0.0.1:3000"
```

`browser_url`은 선택 항목이며 HTTP/HTTPS 주소만 지원합니다. 위 주소는 **메인 PC 자체**의 개발 서버 예시입니다. 그 서버를 OrangeDeck이 대신 시작하지는 않습니다. 저장 후 Agent 터미널에서 Ctrl+C → 같은 Agent 실행기로 다시 켜고, UI에서 해당 프로젝트를 선택하세요. 페어링을 다시 할 필요는 없습니다.

에디터는 Mac의 `/Applications` 또는 `~/Applications`에서 **Zed → Visual Studio Code → VSCodium** 순으로 찾습니다. Linux에서는 Agent 실행기의 PATH에 있는 `zed`/`zeditor` → `code` → `codium`을 찾습니다. 터미널은 Mac Terminal, Linux Konsole → GNOME Terminal → Xfce Terminal 순입니다. 설치된 앱이 없으면 오류를 표시합니다. 앱 실행 요청은 메인 PC에 전달되므로 메인 PC의 로그인된 데스크톱에서 결과를 확인하세요.

**English:** host open actions require an Agent-registered project. Add a unique project block to your private `agent.toml`, using a real path on that host; add an optional HTTP/HTTPS `browser_url` inside the same block. Restart Agent after saving. No re-pairing is needed. The sample loopback URL refers to a server already running on the host. Editor detection supports Zed, VS Code and VSCodium; terminal detection supports macOS Terminal or Linux Konsole, GNOME Terminal and Xfce Terminal. Missing applications report an error. Linux commands must be on the launcher's PATH. Commands run on the host desktop, not the handheld.

## More projects, re-pairing and token rotation

Read-only project discovery comes from Codex history and does not expand the action allow-list. To register additional trusted projects for implemented API actions, edit your **private** `agent.toml` using `config/agent.example.toml` as a structural reference, then restart the Agent. Do not edit the public example with your real paths.

After an Agent address change, use `orangedeck-agent pairing --config /path/to/agent.toml --output /path/to/private-pairing.toml`, transfer privately and explicitly import with `orangedeck-ui pair --config /path/to/config.toml --bundle /path/to/private-pairing.toml --force`.

If a token was exposed, first stop the Agent. Regenerate it using `orangedeck-agent init --force` with the intended project/settings, then transfer the new bundle and explicitly re-pair. `init --force` replaces the Agent configuration and its initial project list: review/preserve additional project settings before doing so. Old connected clients must be disconnected by stopping the Agent. See [security](SECURITY.md).

## Remove / 제거

Stop the UI and its Agent terminal first. In Codex `/hooks`, disable the OrangeDeck hooks before deleting their executable. Remove only OrangeDeck's entries if you choose to edit `hooks.json`; preserve other hooks and backups. Then remove the dedicated OrangeDeck installation folder and any desktop shortcut you copied. Codex, Tailscale, Rust and project data belong to their own installations and are not automatically removed. Do not delete shared parent config directories.

## Troubleshooting / 문제 해결

- **Command file blocked:** open Terminal in the source folder and run `sh install.sh --role agent`. Review downloaded code and macOS security prompts; do not globally disable Gatekeeper.
- **Codex not found/logged out:** pass `--codex-binary`, and sign in using your normal Codex installation.
- **Tailscale disconnected:** sign in on both devices and check tailnet policy. Setup does not weaken network policy.
- **Bind/port already in use:** close only the old Agent's terminal with Ctrl+C, then start the new launcher.
- **No approvals:** start Agent, install notifications, review `/hooks`, and use the same Codex user/profile. No hook is generated for operations that do not need approval.
- **UI cannot connect:** run the generated `check-ui.sh`. For Agent diagnostics run `check-agent.command` on the host. These use the saved executable paths and configuration. Native `verify` is read-only unless you explicitly add action flags.
- **No real-device test yet:** verify a new token reading, completion notification and a natural explicit approval on your own machines. A demo screen is not evidence of those events.
