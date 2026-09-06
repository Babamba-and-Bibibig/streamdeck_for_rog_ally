# macOS Agent setup

Start with [Installation](INSTALL.md) or the [한국어 README](../README.ko.md#설치하기).

In the fully extracted source directory, run `sh install.sh --role agent`, or open `Setup OrangeDeck.command`.

Provide your own project folder and display names. The installer discovers Cargo, Codex and Tailscale, builds the Agent, and generates configuration plus a private pairing bundle. Its default config is explicitly `~/.config/orangedeck/agent.toml` on macOS and Linux; this differs from the native macOS default chosen by the low-level Rust CLI. Use the generated launcher or pass `--config` explicitly.

After installation: **start Agent → run enable-notifications.command → review/trust OrangeDeck in Codex `/hooks` → privately transfer the pairing file → install the Linux UI**. The hook command should run as the same user, with the same `CODEX_HOME`, as the Codex sessions you want to observe.

Starting in 0.1.19 the hook socket and owned-thread registry follow the actual `--config` directory. When updating an older Agent, restart it, rerun the notification launcher and review `/hooks` again. The generated `check-agent.command` diagnoses the saved configuration, Codex and Tailscale paths. Updates preserve your selected Codex profile.

On macOS, Tailscale's CLI may be bundled inside `/Applications/Tailscale.app/Contents/MacOS/Tailscale`. The installer and Agent support this path and force CLI mode. See [Tailscale CLI](https://tailscale.com/docs/reference/tailscale-cli?tab=macos).

No login service, firewall changes, system-wide permissions, Codex upgrade or credential edits are installed automatically. Complete a real-device notification, token and explicit approval check yourself; simulation and cross-compilation are not Mac runtime verification.
