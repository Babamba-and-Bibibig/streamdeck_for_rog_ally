# macOS Connector setup

Start with [Installation](INSTALL.en.md) or the [한국어 README](../README.ko.md#설치하기). **The tested setup is Mac Studio (macOS, main PC) + ASUS ROG Ally (CachyOS Handheld, remote in desktop mode).**

**The main PC running Codex and Connector must use macOS. Linux and Windows hosts are not supported.** Windows remotes are also unsupported; other Linux distributions, Steam Deck, SteamOS and other handhelds are unverified as remotes.

The three `.command` files in the downloaded folder are **macOS launcher scripts for the main PC**. Double-click them in this order during first setup:

| File | Purpose | Window after completion |
| --- | --- | --- |
| **1. Setup OrangeDeck.command** | Install or update Connector and prepare your settings. | Press Enter when prompted and close it after setup finishes. |
| **2. Start OrangeDeck Connector.command** | Run communication between the Mac and Ally. | **Keep it open while using OrangeDeck.** |
| **3. Enable Codex Notifications.command** | Connect Codex completion alerts and approval requests. | Press Enter when prompted and close it after configuration finishes. |

Then review and trust OrangeDeck in **`/hooks` in your usual Mac Codex session**. **For everyday use, run Start only.** Keep Codex and Tailscale running too. The Linux Ally uses `sh install.sh --role ui` for setup and its generated `start-ui.sh` to launch the remote; it does not run these Mac files.

If a Mac file will not open, run `sh install.sh --role connector` in the fully extracted source directory instead of Setup, then continue with Start and Enable. See [updates](INSTALL.en.md#update) for the exact restart order.

Provide your own project folder and display names. The installer discovers Cargo, Codex and Tailscale, builds Connector, and generates configuration plus a private connection file. The guided installation saves Mac settings in `~/.config/orangedeck/connector.toml`. Use its generated launcher, or the paths printed by setup if you chose a custom settings folder.

After the Mac setup above, privately transfer the connection file and install the remote on the CachyOS Ally. Run notification setup as the same user, with the same `CODEX_HOME`, as the Codex sessions you want to observe.

Starting in 0.1.19 the hook socket and owned-thread registry follow the actual `--config` directory. Version 0.1.23 renames the Mac program to Connector. Setup imports the previous configuration and token into the new filenames, preserves the Codex profile and keeps the old files. Start the updated Connector, rerun the notification launcher and review `/hooks` again. The generated `check-connector.command` diagnoses the saved configuration, Codex and Tailscale paths.

On macOS, Tailscale's CLI may be bundled inside `/Applications/Tailscale.app/Contents/MacOS/Tailscale`. The installer and Connector support this path and force CLI mode. See [Tailscale CLI](https://tailscale.com/docs/reference/tailscale-cli?tab=macos).

Leave the Connector terminal open while using the Ally remote. Press Ctrl+C in that terminal to stop it.
