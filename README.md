# OrangeDeck

**A native companion screen for your Codex work.** Keep working on your Mac; follow the current conversation, token readings, remaining quotas and approval requests on a Linux desktop or handheld.

[한국어](README.ko.md) · [Installation](docs/INSTALL.md) · [Controls](docs/CONTROLS.md) · [Security](docs/SECURITY.md)

OrangeDeck is an independent community project, not an official OpenAI, ASUS or Elgato product. Version **0.1.19** is an early release. The interface currently contains Korean labels; both installation prompts and project documentation are available in English and Korean.

## What it does

| View | What you can do |
| --- | --- |
| **LIVE** | Follow the latest conversation in a selected project; read current-turn/model-request tokens, remaining five-hour/weekly quotas and server-provided account statistics. |
| **Shortcuts · 단축키** | Use a 5 × 2 deck with **01 Approve / 02 Reject**. A new approval for the selected current turn opens this view once. Eight keys are unassigned. |
| **Projects · 프로젝트들** | Browse folders discovered from Codex conversation history and change the shared project selection. |
| **Conversations · 대화** | Pin a conversation or follow recent activity within the current project. |
| **Notifications · 알림** | See the current question, response, completion and explicit approval details. Touch and controller actions decide the displayed request once. |

- Native Rust UI with touch, mouse and controller input, HTTP/WebSocket updates and reconnect protection.
- Large token readings, visible stale/disconnected states, and remaining quota bars: **100% − official used percentage**. Unused quota is 100% remaining.
- Persistent alerts for the selected current turn. Approval keys stay gray when unavailable and pulse only when actionable.
- Private communication over Tailscale, with a separate randomly generated OrangeDeck pairing token.
- Local simulation mode for development without a Codex account or remote machine.

Token readings update when model requests are recorded, not with every generated token. Account fields vary by account and server response; OrangeDeck does not invent missing periods, convert percentages into token counts, or guarantee the server's aggregation scope.

## Supported environments

| Component | Environment |
| --- | --- |
| Agent | macOS with Codex CLI; Linux Agent is also available. macOS host-opening actions use macOS applications. |
| Display | Linux with Wayland/X11, OpenGL and a desktop session; a Linux ROG Ally is one example. |
| Connection | Both devices signed in to the same trusted Tailscale network, with TCP 45831 allowed. |
| Build | Python 3.9+ for setup, Rust **1.95.0**, and native development libraries. Packaging requires Python 3.11+. |

Windows, stock Windows ROG Ally installations, mobile clients and a macOS display installer are **not currently supported**. New macOS installs and each Linux distribution still need testing on real devices. The stronger alerts and natural Mac approval round trip have not been fully validated in this release; local simulation tests do not establish real-device success.

## Install

Use **Agent** on the Mac Studio, Mac or Linux PC where you already use Codex, and **UI** on the Linux ROG Ally or Linux desktop you want to use as a display. Windows support is deferred.

```text
Your Mac / Linux main PC          Your Linux ROG Ally / desktop
Codex + OrangeDeck Agent   ← Tailscale →   OrangeDeck UI
```

On **both devices**, open [this repository](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally), choose **Code → Download ZIP**, and extract the ZIP completely. Git and a GitHub SSH key are unnecessary for downloading a public ZIP. Keep the source folder for future builds; installed programs and settings live separately.

To run commands below, open Terminal, type `cd ` (including the space), drag the extracted folder into Terminal, and press Enter. The folder must contain `install.sh` and `Cargo.toml`. On Linux you can also use your file manager's **Open Terminal Here**.

Before setup, install [Tailscale](https://tailscale.com/download) on both devices, sign in to the same tailnet, and install/sign in to [Codex CLI](https://learn.chatgpt.com/docs/cli) on the Agent machine. Account login, operating-system consent and Codex hook trust require your own interaction.

Run `python3 --version` in Terminal; setup needs **Python 3.9+**. If missing, use the [macOS Python installer](https://www.python.org/downloads/macos/) or your Linux distribution's `python3` package. On the Agent, `codex login status` should succeed. Rust and native build dependencies are offered during setup. The first source build can take several minutes or longer; keep the terminal open. Run setup as your normal account, without `sudo`.

### 1. On the Mac / Agent machine

Double-click **Setup OrangeDeck.command** on macOS, or run:

```sh
sh install.sh --role agent
```

When asked, enter:

| Prompt | Your answer |
| --- | --- |
| Project folder | An existing, trusted project folder on this main PC. Dragging the folder into Terminal also works. |
| Host label | A name to show in OrangeDeck, such as `MY MAC`. Enter accepts the default. |
| Project label | A display name for the folder. Enter accepts its folder name. |
| Install missing tools? | Review the displayed installation and enter `y` to proceed, or install them yourself. On macOS finish Apple's developer-tools dialog and rerun setup when asked. |

Setup discovers Codex and your Tailscale address, builds the app, generates private configuration and pairing credentials, and creates launchers. It asks for a Codex executable path only if discovery fails. Existing configuration and credentials are reused on updates.

Follow the printed steps in this order:

1. Start the Agent with the generated **start-agent.command** and keep its terminal open.
2. In another terminal, run the generated **enable-notifications.command** (or **Enable Codex Notifications.command** in the source folder).
3. In your usual Codex session, open **`/hooks`**, inspect the OrangeDeck definitions and trust them. [Codex hook review](https://learn.chatgpt.com/docs/hooks).
4. Transfer the generated **orangedeck-pairing.toml** privately to the display device using Tailscale Taildrop or an offline method. This file contains a secret: never upload it to GitHub or paste it into chat.

With the default installation, these are the exact commands. Run the first one and **leave its terminal open**:

```sh
sh "$HOME/.config/orangedeck/start-agent.command"
```

In a **second terminal**:

```sh
sh "$HOME/.config/orangedeck/enable-notifications.command"
```

Then review `/hooks` in your usual Codex. To find the pairing file on a Mac, use **Finder → Go → Go to Folder…**, enter `~/.config/orangedeck`, and select `orangedeck-pairing.toml`. Send it to your own Linux device with [Tailscale Taildrop](https://tailscale.com/docs/features/taildrop), or copy it with a USB drive. On Linux, show hidden folders in the file manager to reach `.config/orangedeck`. Keep the received file somewhere you can select, such as Downloads.

### 2. On the Linux display

In the extracted source folder, run:

```sh
sh install.sh --role ui
```

At the pairing prompt, paste or drag the **received file's full path** and press Enter. Setup builds and installs the UI, imports the private connection settings and prints the **start-ui.sh** launcher and desktop shortcut location. No IP address or token needs to be typed. With the Agent still running, start the display:

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

Check **CONNECTED**, select a project/conversation, and verify a new token reading and completion notification from your normal Codex work. When a natural approval request appears, read it and explicitly decide it to check delivery. A connected screen alone does not verify hooks.

The commands above use the default directory `~/.config/orangedeck` on both machines. If you set `XDG_CONFIG_HOME` or `--config-dir`, use the **exact paths printed by setup**. A custom Codex profile is selected by running Agent setup with your usual `CODEX_HOME`; subsequent updates preserve it. Read [advanced settings and removal](docs/INSTALL.md).

## Everyday use

Start the generated Agent and UI launchers. Select a project and conversation. **자동 ON** means follow recent activity; selecting a specific conversation pins it. Switch back with **최근 자동**.

On the shortcut deck, directions/stick move across keys; physical **A** activates the selected key and **B** rejects the displayed actionable request. Read request details before deciding. Enter/Escape/Space do not approve requests. Closing an alert does not approve a request behind it. Answer ordinary Codex questions on the Agent machine.

Close the UI window to stop the display. Stop the Agent with **Ctrl+C** in its own terminal when finished. Restart with the same launchers; ordinary restarts do not need setup or hook installation again.

## Troubleshooting and updates

| Problem | What to do |
| --- | --- |
| Mac `.command` does not open | Open Terminal in the extracted source folder and run `sh install.sh --role agent`. |
| Installation reports a missing tool | Follow the specific prompt, finish installation, and rerun the same setup command. |
| UI cannot connect | Check Tailscale on both devices and keep Agent running. On the display run `sh "$HOME/.config/orangedeck/check-ui.sh"`. |
| Agent fails to start | In another Agent-side terminal run `sh "$HOME/.config/orangedeck/check-agent.command"`; close an old Agent with Ctrl+C if it already occupies the port. |
| Connected, but no approval/notification | Run the notification launcher, review `/hooks`, and check that Agent and Codex use the same account/profile. Only operations needing approval create approval requests. |

To update, download and extract a new source version, then rerun the same `sh install.sh --role agent` or `--role ui` on the corresponding device. Existing settings, pairing and Codex profile are preserved. Close the old OrangeDeck process and launch it again to use the new binary.

**Updating an Agent from 0.1.18 or earlier to 0.1.19:** after restarting Agent, run `enable-notifications.command` again and review `/hooks` again. Hook sockets now follow the selected Agent configuration, which fixes new/custom macOS installations. Use the printed launcher paths if customized.

## Privacy and security

The Agent can send project paths, conversation titles, selected question/response text, approval tool arguments and usage statistics to your paired display. Treat screenshots, diagnostic output and pairing files as private. Codex authentication stays on its host; OrangeDeck has no analytics service and does not upload your work to GitHub.

Authenticated API clients can invoke implemented allow-listed operations, including fixed Cargo commands in registered projects and actions for OrangeDeck-owned Codex threads, even though these are not current UI tabs. Cargo build scripts and explicitly approved tools can run code with your account's permissions. Pair only trusted devices and register trusted repositories. There is no arbitrary shell endpoint or automatic approval. [Full trust boundary](docs/SECURITY.md).

Public packages exclude machine settings, credentials, runtime data and private session handoffs. Before committing or publishing:

```sh
python3 scripts/check_public.py
python3 scripts/check_public.py --tracked --history
```

The second command requires a full Git checkout and checks ignored-but-tracked files, historical paths/content and commit/tag metadata. For a separate upload candidate, run `python3 download-page/manage.py export`; it creates only reviewed source under `dist/github/OrangeDeck-vVERSION/` and a file checksum list. Run local checks **before pushing**; GitHub Actions runs after files reach GitHub. A clean heuristic scan is not proof that no secret exists. Removing a file from the latest tree does not erase Git history or previously published archives. [Publication and incident handling](docs/PUBLICATION.md).

## Development

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
python3 scripts/check_architecture.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 download-page/test_manage.py
cargo build --release --workspace --locked
./scripts/run-demo.zsh
```

The demo uses loopback and labels its data **SIMULATED MAC / LOCAL MOCK**. It requires zsh and curl. A real UI rejects a mock Agent. The adapter's recorded schema baseline is Codex CLI 0.153.2; other versions receive a compatibility warning and runtime checks rather than a promise of support.

[Architecture](docs/ARCHITECTURE.md) · [Protocol](docs/PROTOCOL.md) · [Changelog](docs/CHANGELOG.md) · [Optional private download server](download-page/README.md)

Licensed under the [MIT License](LICENSE).
