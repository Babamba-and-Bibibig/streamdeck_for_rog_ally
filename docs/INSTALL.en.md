<h3><a href="INSTALL.md">🇰🇷 한국어</a>　|　English · current page</h3>

# Setup help

For a first installation, follow [the three README steps](../README.en.md#install). Use this page when setup gets stuck or you want to change a setting.

This guide targets **macOS on the work Mac, developed around a Mac Studio, and CachyOS Handheld in desktop mode on a ROG Ally remote**. Steam Deck, SteamOS and other Linux distributions have not been validated for this guide. Windows is not supported.

## Prerequisites

| Where? | What you need |
| --- | --- |
| **Mac** | Installed and signed-in Codex CLI; Python 3.9+ |
| **Ally** | CachyOS Handheld desktop mode; Python 3.9+ |
| **Both** | Tailscale installed and signed in with the same account; the OrangeDeck ZIP fully extracted |

Check Python in each device's terminal:

```sh
python3 --version
```

Use version 3.9 or later. On a Mac, use the [official Python installer](https://www.python.org/downloads/macos/) if needed. On the CachyOS Ally, install the [python package](https://archlinux.org/packages/core/x86_64/python/) with:

```sh
sudo pacman -S --needed python
```

Check Codex sign-in on the Mac:

```sh
codex login status
```

[Tailscale downloads](https://tailscale.com/download) · [Codex CLI guide](https://learn.chatgpt.com/docs/cli)

OrangeDeck setup asks before installing Rust and other required development tools. It builds the app from source, so the first installation takes time. If macOS opens a developer-tools installation dialog, finish it and rerun Setup. **Run OrangeDeck setup as your normal user, without `sudo`.** Setup does not install, upgrade or sign into Codex for you.

## Mac command file will not open

1. Open **Terminal**.
2. Type `cd `, including the space after `cd`.
3. Drag the extracted OrangeDeck folder into Terminal and press Enter. Choose the folder containing `install.sh` and `Cargo.toml`.
4. Run:

```sh
sh install.sh --role agent
```

Then continue from [Mac setup step 2](../README.en.md#1-install-agent-on-the-mac). There is no need to disable macOS security globally.

## What to enter

| Prompt | Your input |
| --- | --- |
| Project folder on the Mac | The real folder you work on with Codex. Dragging a folder into the terminal is supported. |
| Device and project names | Labels to show on the Ally. Enter accepts the suggested value. |
| Codex executable | Asked only if discovery fails. Find it with `command -v codex` in your normal Mac terminal. |
| Connection file on the Ally | The transferred `orangedeck-pairing.toml`. Drag the file into the terminal. |
| Install required tools | Review the prompt and enter `y` to proceed. |

You do not need to invent an IP address or credential. Transfer the connection file created on the Mac. Keep that file private.

## Start again

On the Mac, start Agent first and leave this terminal open:

```sh
sh "$HOME/.config/orangedeck/start-agent.command"
```

On the Ally:

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

Close the Ally app window to stop the remote. Use **Ctrl+C** in the Mac Agent terminal to stop Agent. If you chose a custom settings directory, use the paths printed by setup.

## Connection or alert problems

1. Check that **Tailscale is running on both devices**, signed in with the same account.
2. Check that **the Mac Agent terminal is still running**.
3. For missing alerts, start Agent, run **Enable Codex Notifications.command**, then review/trust OrangeDeck in **`/hooks`** in your usual Mac Codex session. Set up the connection under the same user account as Codex.
4. Check the connection on the Ally:

```sh
sh "$HOME/.config/orangedeck/check-ui.sh"
```

For Mac Agent problems:

```sh
sh "$HOME/.config/orangedeck/check-agent.command"
```

“Address in use” usually means an Agent is already running. Stop that Agent in its own terminal with Ctrl+C, then restart it. If your organization or custom Tailscale policy restricts connections, check whether TCP 45831 is allowed between the devices.

Approval keys are active only when Codex has an actual approval request. Answer ordinary questions on the Mac. CONNECTED does not confirm that notifications were set up: do new work on the Mac and check the Ally's token reading, completion alert and explicit approval delivery. A fresh latest-version Mac installation followed by these real-device checks has not yet been completed.

## Update

1. Download the new ZIP on both devices and extract it into a **new folder**.
2. **Mac:** run **Setup OrangeDeck.command** from the new folder. **Ally:** run `sh install.sh --role ui` from the new folder.
3. Close the old OrangeDeck processes and restart them. Personal settings, pairing, language and key assignments are preserved.

**If Mac Agent is 0.1.18 or earlier:** start the updated Agent, run **Enable Codex Notifications.command**, then review/trust OrangeDeck in Codex **`/hooks`** again. The notification socket location changed in 0.1.19.

Older source-folder installations can continue using the Mac's `Start OrangeDeck Agent.command` and the Ally's `./scripts/run-ally.zsh`. Run the Ally command in the existing project folder. Review `/hooks` again when the executable location changes.

## Host shortcuts

**Editor, terminal, folder and webpage keys on the Ally open things on the Mac.** The project folder chosen during setup is registered already. A folder appearing in Codex history does not by itself allow open actions.

To add a folder, append a block to **the Mac's private** `~/.config/orangedeck/agent.toml`. Choose an unused `id` and replace `path` with the real project folder. Omit `browser_url` if you do not need a webpage.

```toml
[[projects]]
id = "another-project"
name = "My Website"
path = "/Users/YOU/Code/my-website"
browser_url = "http://127.0.0.1:3000"
```

Web addresses must use HTTP/HTTPS. The example refers to a development server already running on the Mac. Save the file, restart Agent, then select the folder on the Ally. No new connection-file transfer is needed.

Editor detection checks `/Applications` and `~/Applications` on the Mac in this order: **Zed, VS Code, VSCodium**. The terminal key opens macOS **Terminal**. Install a supported editor if none is found.

## Where settings are stored

The default directory on both devices is **`~/.config/orangedeck`**. Its contents are private.

| File | Purpose |
| --- | --- |
| Mac `agent.toml` | Project folders and Codex executable settings |
| Ally `config.toml` | Mac connection settings |
| `orangedeck-pairing.toml` | Private connection file transferred from Mac to Ally |
| `agent.token` / `ui.token` | Secret connection credentials |
| Ally `ui-preferences.toml` | Automatically saved language and keys |
| `start-agent.command` / `start-ui.sh` | Launchers |
| `enable-notifications.command` | Mac notification setup |
| `check-agent.command` / `check-ui.sh` | Diagnostic launchers |

Do not put personal settings in the public example files. Notification installation preserves existing Codex settings and backs up `hooks.json` before adding OrangeDeck entries.

## Optional installer settings

Most users can keep the defaults. **`agent` means the Mac; `ui` means the Ally.**

| Option | Purpose |
| --- | --- |
| `--project-path`, `--project-name`, `--host-name` | Initial project folder and display labels |
| `--codex-binary` | Explicit Mac Codex executable location |
| `--pairing` | Connection-file location for a first Ally pairing |
| `--config-dir` | Custom private installation directory |
| `--port` | Change the default connection port, 45831 |
| `--install-rust`, `--install-deps` | Permit the displayed tool installation in advance |
| `--non-interactive` | Use supplied settings without asking; stop if required values are missing |
| `--start` / `--check` | Start after setup / inspect prerequisites without installing |

Updates preserve existing project and connection settings; first-install options do not overwrite them. With a custom `XDG_CONFIG_HOME` or `--config-dir`, use the printed launchers. If you use a custom Codex profile (`CODEX_HOME`), install from your usual terminal environment. The installer saves that location and preserves it on update.

## Lost connection files or changed settings

Setup stops if credentials exist without a configuration. Do not delete credentials or add `--force` simply to suppress the error. Restore your configuration or choose a new settings directory.

After a Mac address change, run `orangedeck-agent pairing --config /path/to/agent.toml --output /path/to/private-pairing.toml` on the Mac, transfer the result privately, then run `orangedeck-ui pair --config /path/to/config.toml --bundle /path/to/private-pairing.toml --force` on the Ally. Replace `/path/to/...` with your actual paths.

If a credential was exposed, stop Agent first. `orangedeck-agent init --force` recreates both the credential and the initial Mac project configuration. Preserve/review existing projects before doing so, then pair again with the new connection file. See [publication incident handling](PUBLICATION.md).

## Uninstall

Stop the Ally app and Mac Agent. Disable OrangeDeck in Codex `/hooks`, then remove the dedicated OrangeDeck installation directory and any shortcut you copied. Preserve other applications' settings, Codex conversations and project folders. Codex, Tailscale and Rust have their own separate installations.
