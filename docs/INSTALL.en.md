<h3><a href="INSTALL.md">🇰🇷 한국어</a>　|　English · current page</h3>

# Setup help

For a first installation, follow [the three README steps](../README.en.md#install). Use this page when setup gets stuck or you want to change a setting.

This guide targets **macOS on the work Mac, developed around a Mac Studio, and CachyOS Handheld in desktop mode on a ROG Ally remote**. Steam Deck, SteamOS and other Linux distributions have not been validated for this guide. Windows is not supported.

## Which files run on which device?

**On the Mac only**, double-click the three files in the extracted folder in order: **1 → 2 → 3**.

| Mac file | Purpose | Can I close its window? |
| --- | --- | --- |
| **1. Setup OrangeDeck.command** | Installs or updates Connector and prepares your settings. | After setup completes, press Enter when prompted and close it. |
| **2. Start OrangeDeck Connector.command** | Runs the communication module that sends Codex status and receives Ally button actions. | **Keep it open while using OrangeDeck.** |
| **3. Enable Codex Notifications.command** | Connects Codex completion alerts and approval requests. | After configuration completes, press Enter when prompted and close it. |

After step 3, enter **`/hooks` in your usual Mac Codex session**, then **review and trust OrangeDeck**. **Only window 2 stays open for OrangeDeck on the Mac. For everyday use, run Start only.** Keep Codex and Tailscale running too.

**The Linux Ally running CachyOS Handheld uses its own commands.** To install or update, run `sh install.sh --role ui` in the extracted folder. For everyday use, run `sh "$HOME/.config/orangedeck/start-ui.sh"` to start the remote. [First Ally setup](../README.en.md#3-install-the-remote-on-the-ally).

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
sh install.sh --role connector
```

Then continue from [step 2: Start Connector](../README.en.md#1-install-connector-on-the-mac). There is no need to disable macOS security globally.

## What to enter

**These choices are for a first installation on your own devices.** An update [reuses saved settings](#saved-settings), so it does not ask the same questions again.

| Prompt | Your input |
| --- | --- |
| Project folder on the Mac | The real folder you work on with Codex. Dragging a folder into the terminal is supported. |
| Device and project names | Labels to show on the Ally. Enter accepts the suggested value. |
| Mac editor | Choose `zed`, `vs_code`, `cursor` or `vscodium`. Enter uses automatic detection. |
| Codex executable | Asked only if discovery fails. Find it with `command -v codex` in your normal Mac terminal. |
| Connection file on the Ally | The transferred `orangedeck-pairing.toml`. Drag the file into the terminal. |
| Install required tools | Review the prompt and enter `y` to proceed. |

Setup reads this Mac's Tailscale address and generates **a new connection credential**. Instead of typing either value, transfer the resulting `orangedeck-pairing.toml` to your Ally. It contains the Mac address and secret credential, so do not upload it to GitHub or chat.

<a id="saved-settings"></a>

## Setup did not ask for my settings or editor again

**This is normal on a device where OrangeDeck is already installed.** Updates reuse that device's saved project folders, editor and connection settings. Personal settings default to **`~/.config/orangedeck/`** on each device. Extracting a new ZIP into a different folder does not remove them. An older Mac configuration without an editor choice uses automatic detection. To change it, see [editor settings](#host-shortcuts).

**Personal settings and connection credentials are created during each user's installation and are not included in the GitHub download.** Register the connection file from your Mac on your Ally to connect them. Connecting requires **both Tailscale access to that Mac and its correct connection credential**.

## Start again

**Everyday launches do not require Setup, Enable or reinstallation.** On the Mac, double-click Start or run the command below, then leave the Connector terminal open:

```sh
sh "$HOME/.config/orangedeck/start-connector.command"
```

On the Ally, run the following. When launching this way, leave that terminal open while using the app.

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

Close the Ally app window to stop the remote. Use **Ctrl+C** in the Mac Connector terminal to stop Connector. If you chose a custom settings directory, use the paths printed by setup.

## Connection or alert problems

1. Check that **Tailscale is running on both devices**, signed in with the same account.
2. Check that **the Mac Connector terminal is still running**.
3. For missing alerts, start Connector, run **Enable Codex Notifications.command**, then review/trust OrangeDeck in **`/hooks`** in your usual Mac Codex session. Set up the connection under the same user account as Codex.
4. Check the connection on the Ally:

```sh
sh "$HOME/.config/orangedeck/check-ui.sh"
```

For Mac Connector problems:

```sh
sh "$HOME/.config/orangedeck/check-connector.command"
```

“Address in use” usually means an Connector is already running. Stop that Connector in its own terminal with Ctrl+C, then restart it. If your organization or custom Tailscale policy restricts connections, check whether TCP 45831 is allowed between the devices.

Approval keys are active only when Codex has an actual approval request. Answer ordinary questions on the Mac. CONNECTED does not confirm that notifications were set up: do new work on the Mac and check the Ally's token reading, completion alert and explicit approval delivery.

## Update

**Get updates from GitHub.** Download the [new ZIP](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally/archive/refs/heads/main.zip) on both devices and extract it into a **new folder**. Then follow the steps for each device. The app does not install new versions automatically.

**On the Mac:**

1. Double-click **Setup OrangeDeck.command** in the new folder. After setup completes, press Enter when prompted and close its window.
2. Press Ctrl+C in the **old OrangeDeck Connector terminal**. Run **Start OrangeDeck Connector.command** from the new folder and leave that terminal open.
3. Run **Enable Codex Notifications.command** from the new folder. After configuration completes, close its window and **review and trust OrangeDeck in `/hooks` in your usual Mac Codex session**.

**On the Ally:**

1. Run `sh install.sh --role ui` in the new folder to update.
2. Close the old OrangeDeck app window and restart it with `sh "$HOME/.config/orangedeck/start-ui.sh"`.

**Personal settings, pairing and language are preserved. No new connection-file transfer is needed.** It is normal not to be asked for your project or editor again. [Why settings are reused](#saved-settings). The individual action keys from 0.1.23 and earlier are replaced by five conversation pairs; assign conversations with the upper + keys once. Everyday launches do not repeat installation or notification configuration.

**When updating from 0.1.21 or earlier:** the Mac program and settings files now use the Connector name. Setup imports existing projects and credentials into the new filenames and keeps the old files. Complete the Enable → `/hooks` review and trust steps above.

For an older source-folder installation, run Setup on the Mac once during this update. Then use `Start OrangeDeck Connector.command` from the new folder. The Ally's existing launcher is `./scripts/run-ally.zsh`, run from the project folder.

<a id="host-shortcuts"></a>

## Mac editor and project folders

**The lower row opens changed code in your Mac editor.** Choose the editor during first setup. Press Enter for automatic detection in this order: Zed → VS Code → Cursor → VSCodium.

To change it, edit the Mac’s private **`~/.config/orangedeck/connector.toml`**. Add or update `editor` **above the first `[[projects]]` block**:

```toml
editor = "zed"
```

Choose `auto`, `zed`, `vs_code`, `cursor` or `vscodium`. Install that app in `/Applications` or `~/Applications` on the Mac, then restart Connector after saving. Existing installations default to automatic detection; rerunning setup preserves your private settings.

Only changed files **inside registered project folders** can be opened. To add a project, append this block to the same file. Use an unused `id` and the exact working folder shown for the Codex conversation:

```toml
[[projects]]
id = "another-project"
name = "My Website"
path = "/Users/YOU/Code/my-website"
```

Save and restart Connector. No new connection-file transfer is needed. Appearing in Codex history does not authorize opening files from an unregistered project.

## Where settings are stored

The default directory on both devices is **`~/.config/orangedeck`**. Its contents are private.

| File | Purpose |
| --- | --- |
| Mac `connector.toml` | Project folders, editor and Codex executable settings |
| Ally `config.toml` | Mac connection settings |
| `orangedeck-pairing.toml` | Private connection file transferred from Mac to Ally |
| `connector.token` / `ui.token` | Secret connection credentials |
| Ally `ui-preferences.toml` | Automatically saved language and keys |
| `start-connector.command` / `start-ui.sh` | Launchers |
| `enable-notifications.command` | Mac notification setup |
| `check-connector.command` / `check-ui.sh` | Diagnostic launchers |

Do not put personal settings in the public example files. Notification installation preserves existing Codex settings and backs up `hooks.json` before adding OrangeDeck entries.

## Optional installer settings

Most users can keep the defaults. **`connector` means the Mac; `ui` means the Ally.**

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

After a Mac address change, run `orangedeck-connector pairing --config /path/to/connector.toml --output /path/to/private-pairing.toml` on the Mac, transfer the result privately, then run `orangedeck-ui pair --config /path/to/config.toml --bundle /path/to/private-pairing.toml --force` on the Ally. Replace `/path/to/...` with your actual paths.

If a credential was exposed, stop Connector first. `orangedeck-connector init --force` recreates both the credential and the initial Mac project configuration. Preserve/review existing projects before doing so, then pair again with the new connection file. See [publication incident handling](PUBLICATION.md).

## Uninstall

Stop the Ally app and Mac Connector. Disable OrangeDeck in Codex `/hooks`, then remove the dedicated OrangeDeck installation directory and any shortcut you copied. Preserve other applications' settings, Codex conversations and project folders. Codex, Tailscale and Rust have their own separate installations.
