# Installation / 설치 상세

[English overview](../README.md#install) · [한국어 안내](../README.ko.md#설치하기)

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

For Debian/Ubuntu UI builds the package set includes `build-essential`, `pkg-config`, `libudev-dev`, `libxkbcommon-dev`, `libwayland-dev`, `libx11-dev`, `libxi-dev`, `libgl1-mesa-dev`, `libdbus-1-dev` and `libnotify-bin`. The installer contains corresponding Fedora/Arch package lists. Runtime needs a desktop session with graphics drivers; controller/notification support also depends on the desktop and available input device permissions.

Immutable handheld distributions may restrict package installation. Setup does not disable read-only system protection or change controller permissions. Use that distribution's supported development environment. This repository does not promise successful installation on every Linux image.

Official build reference: [Rust installation and native linker requirements](https://doc.rust-lang.org/book/ch01-01-installation.html).

## Update / 업데이트

Download a new source version, extract it into a separate folder, and rerun the installer for the same role and config directory. It builds the new source and atomically replaces the installed executable. Existing tokens, projects and pairing are preserved. The previous process keeps running until you close it; the next launch uses the updated binary.

The newly built executable validates an existing configuration and credential before replacing the installed binary. Agent updates check the Codex executable configured in that file and preserve the previous Codex profile. `orangedeck-agent check-config --config ...` and `orangedeck-ui check-config --config ...` perform that local validation without starting services or contacting accounts.

**0.1.19 Agent migration:** restart the Agent, rerun the generated notification installer, then review/trust `/hooks` again. The socket and owned-thread registry now live beside the selected Agent configuration, fixing fresh/custom macOS setup. If your previous registry lived elsewhere, its conversations remain read-only under the new registry; registries are not automatically merged across configurations.

**0.1.19로 Agent 업데이트:** Agent 재실행 → 알림 설치 실행기 재실행 → `/hooks` 재검토·신뢰를 한 번 진행하세요. 새 알림 소켓은 실제 Agent 설정 폴더를 사용합니다. 기존 소유 대화 기록이 다른 폴더에 있으면 자동 병합하지 않으며 해당 대화는 읽기 전용으로 표시합니다.

If using the older source-folder installation, `scripts/run-ally.zsh`, `Start OrangeDeck Agent.command` and `Enable Codex Notifications.command` remain available. The guided installer can adopt the existing `~/.config/orangedeck` configuration. Review `/hooks` again when the hook definition or executable path changes. Never assume build success proves real Mac notifications or approval delivery.

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
