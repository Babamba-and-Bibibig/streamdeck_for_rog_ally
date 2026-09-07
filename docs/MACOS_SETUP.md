# macOS Connector setup

Start with [Installation](INSTALL.en.md) or the [한국어 README](../README.md#설치하기). This guide pairs a macOS work Mac with a ROG Ally running CachyOS Handheld in desktop mode. Steam Deck and SteamOS are untested.

In the fully extracted source directory, run `sh install.sh --role connector`, or open `Setup OrangeDeck.command`.

Provide your own project folder and display names. The installer discovers Cargo, Codex and Tailscale, builds the Connector, and generates configuration plus a private pairing bundle. Its default config is explicitly `~/.config/orangedeck/connector.toml` on macOS and Linux; this differs from the native macOS default chosen by the low-level Rust CLI. Use the generated launcher or pass `--config` explicitly.

After installation: **start Connector on the Mac → run enable-notifications.command → review/trust OrangeDeck in Codex `/hooks` → privately transfer the connection file → install the remote on the CachyOS Ally**. The hook command should run as the same user, with the same `CODEX_HOME`, as the Codex sessions you want to observe.

Starting in 0.1.19 the hook socket and owned-thread registry follow the actual `--config` directory. Version 0.1.23 renames the Mac program to Connector. Setup imports the previous configuration and token into the new filenames, preserves the Codex profile and keeps the old files. Start the updated Connector, rerun the notification launcher and review `/hooks` again. The generated `check-connector.command` diagnoses the saved configuration, Codex and Tailscale paths.

On macOS, Tailscale's CLI may be bundled inside `/Applications/Tailscale.app/Contents/MacOS/Tailscale`. The installer and Connector support this path and force CLI mode. See [Tailscale CLI](https://tailscale.com/docs/reference/tailscale-cli?tab=macos).

Leave the Connector terminal open while using the Ally remote. Press Ctrl+C in that terminal to stop it.
