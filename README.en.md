<h3 align="center"><a href="README.md">🇰🇷 한국어</a>　|　🌐 English · Current page</h3>

# OrangeDeck

**Your Mac's Codex, at a glance on your Ally.**

Follow current work in **LIVE**. Check replies and changed files from multiple Codex conversations in **Agents**. Respond to approval requests and tap a changed file to open it in your Mac editor.

[⬇ Download ZIP](https://github.com/Babamba-and-Bibibig/streamdeck_for_rog_ally/archive/refs/heads/main.zip) · [Install](#installation) · [Controls](docs/CONTROLS.md)

## LIVE · See what's happening now

![LIVE: the current question, work status, token records and remaining quota](docs/screenshots/en-live.png)

**See the current question, work status, token usage and remaining quota together.**

- **Current work:** follow the question and its progress.
- **Usage records:** see input and output tokens as new records arrive.
- **Remaining quota:** check the five-hour and weekly percentages. They decrease as you use them.

Pin one conversation, or turn **Auto ON** to follow the most recent conversation in the selected project.

## Agents · Five conversations, replies and changed files

![Agents: five Codex conversations with paired response and changed-file keys](docs/screenshots/en-shortcuts.png)

**One column, one conversation. Replies above. Changed files below.**

A new reply or approval request makes both keys in that column pulse, with a notification sound. Tap an **upper + key** to connect a Codex conversation from your Mac terminal once.

<table>
<tr>
<td width="50%">
<b>① Upper key · Read and approve</b><br>
<a href="docs/screenshots/en-response.png"><img src="docs/screenshots/en-response.png" alt="Reading the selected question, reply and approval request" width="640"></a><br>
Read the question and reply. When an approval request arrives, review its details and choose <b>Approve / Reject</b>.
</td>
<td width="50%">
<b>② Lower key · Open changed files</b><br>
<a href="docs/screenshots/en-files.png"><img src="docs/screenshots/en-files.png" alt="Changed files and diffs with navigation to the Mac editor" width="640"></a><br>
Review changes on the Ally. Select a file to move your <b>Mac editor</b> to that file.
</td>
</tr>
</table>

**The connected Codex conversation's working folder is used automatically.** No extra folder registration is needed to open its files. Zed, VS Code, Cursor and VSCodium are supported.

The list shows **file names and changed text recorded for this question**, using Mac file events to fetch only changed files. The dialog distinguishes Codex diffs from the last captured **Current file contents**. Incomplete records show **Check file records**; a confirmed empty list shows **No file changes**. [File checks, retry and editor settings](docs/INSTALL.en.md#host-shortcuts).

Tap outside a dialog or **Close** to dismiss it without making a decision. Language, conversation assignments and sound settings are saved for the next launch.

<sub>Screenshots show 0.1.28 with simulated data. Click to enlarge. <a href="docs/SCREENSHOTS.md">Screenshot details</a></sub>

## How the two devices connect

![Codex and Connector on the Mac send activity to LIVE and Agents on the Ally; approvals and file-open requests return to the Mac.](docs/diagrams/device-roles-en.svg)

**Codex does the AI work on the Mac. OrangeDeck connects the devices.** You do not need to install Codex on the Ally.

| Device | Install |
| --- | --- |
| **macOS work Mac** · developed around Mac Studio | Codex CLI + OrangeDeck **Connector** |
| **ROG Ally running CachyOS Handheld** · desktop mode | OrangeDeck **remote UI** |

This guide covers that configuration. Steam Deck and SteamOS installation and operation are unverified; other Linux distributions and Windows are outside this guide.

## Installation

**Set up the Mac → transfer the connection file → set up the Ally.** Current version: **0.1.31**.

Install [Tailscale](https://tailscale.com/download) on both devices and sign in with the same account. Install [Codex CLI](https://learn.chatgpt.com/docs/cli) on the Mac and sign in. Both devices need Python 3.9 or later. [Check prerequisites](docs/INSTALL.en.md#prerequisites).

On both devices, use **Download ZIP** above and extract it. Git and SSH keys are not needed.

### 1. Install Connector on the Mac

**These three files run on the Mac.** In the extracted folder, double-click them in order.

| Order | File | Purpose and window lifetime |
| --- | --- | --- |
| **①** | **Setup OrangeDeck.command** | Installs Connector and asks for your work folder and editor. After setup finishes, press Enter when prompted and close it. |
| **②** | **Start OrangeDeck Connector.command** | Connects the Mac and Ally. **Keep this window open while using OrangeDeck.** |
| **③** | **Enable Codex Notifications.command** | Connects completion alerts and approval requests. After configuration finishes, press Enter and close it. |

After step ③, open **`/hooks` in your usual Mac Codex session → review and trust OrangeDeck**. The only OrangeDeck window you need to keep open is **② Connector**.

Choose your Mac work folder and editor when Setup asks. If a developer-tool installer appears, finish it and run Setup again. [Mac setup help](docs/INSTALL.en.md#mac-command-file-will-not-open).

### 2. Transfer the Mac's connection file to the Ally

In Mac **Finder → Go → Go to Folder…**, open `~/.config/orangedeck`.

Copy **`orangedeck-pairing.toml`** to **your Ally's Downloads folder** using USB or another private transfer. This **personal connection file** contains your Mac's address and secret connection credential. Do not post it on GitHub or in a chat.

### 3. Install the remote UI on the Ally

In the Ally's **desktop mode**, open the extracted folder, right-click an empty area and select **Open Terminal Here**. Use this command on the Ally:

```sh
sh install.sh --role ui
```

When asked for a connection file, drag **`orangedeck-pairing.toml`** into the terminal and press Enter. You do not need to type an IP address or credential.

After installation, start the app:

```sh
sh "$HOME/.config/orangedeck/start-ui.sh"
```

When it shows **CONNECTED**, open **Agents → an upper + key** and assign a conversation. Use Codex on the Mac to check new LIVE records, replies and changed files on the Ally.

## Next time, just start the apps

| Device | Start | Stop |
| --- | --- | --- |
| **Mac** | Double-click **Start OrangeDeck Connector.command** | Press Ctrl+C in the Connector terminal |
| **Ally** | `sh "$HOME/.config/orangedeck/start-ui.sh"` | Close the OrangeDeck window |

Keep Codex and Tailscale running. If you launch the app from a terminal, keep that terminal open too. Setup, Enable and installation are not daily steps.

<a id="my-settings"></a>

**Updates keep your settings.** First setup creates each device's settings and your personal connection file. Updating the same devices reuses your work folder, editor and connection. Private settings live under `~/.config/orangedeck/` on each device and are not included in downloads. [Update steps](docs/INSTALL.en.md#update) · [Why setup does not ask again](docs/INSTALL.en.md#saved-settings).

[Controls and everyday use](docs/CONTROLS.md) · [Setup and connection help](docs/INSTALL.en.md) · [한국어 안내](README.md) · [Changelog](docs/CHANGELOG.md)
