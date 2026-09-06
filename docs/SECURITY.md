# Security

## Trust Boundary

The Agent runs with the permissions of the logged-in Mac user. A party that has both
tailnet access and the OrangeDeck token can invoke the implemented allow-listed actions.
Protect the tailnet account, device access, and token accordingly.

OrangeDeck does not create a new OpenAI credential. The Mac's local Codex process uses
the Mac's existing Codex authentication. No Codex credential or API key is transferred
to the Ally.

## Enforced Controls

- Real Agent bind and UI URLs must be a literal Tailscale IPv4 in `100.64.0.0/10`.
- `0.0.0.0`, loopback, LAN IPs, public IPs, DNS names, URL credentials, and URL paths
  are rejected in real mode.
- A random 64-hex-character pre-shared token authenticates all state and command routes.
- Token comparison is constant-time. Token debug and malformed TOML errors withhold secret contents. Real pairing rejects the public demo token.
- HTTP bypasses system proxy settings and refuses redirects; Agent WebSocket input frames/messages are capped at 64 KiB.
- Token, pairing, and generated config files use mode `0600` on Unix.
- Projects are canonicalized and resolved only from the Agent-side allow-list.
- Cargo commands are fixed arrays: `check`, `test`, `clippy`, `fmt --check`, and `build`.
- Git commands are fixed read-only status, log, and diff invocations. Optional index
  locks, fsmonitor, external diff programs, and textconv execution are disabled.
- macOS actions are fixed `open` operations using local config values.
- Codex command/file/permission approvals are never automatic or session-wide.
- Existing Codex threads are observe-only unless their ID is in OrangeDeck's own registry.
- Prompts are bounded, are not logged by default, and can target only owned threads.
- API authentication runs before request-body reading/JSON parsing or WebSocket extraction. Command bodies are capped at 64 KiB and only one Cargo job can run at a time.
- Commands pressed while disconnected or connecting are rejected, never replayed later.
- `Ctrl-C` and `SIGTERM` trigger process-group cleanup for active Cargo and Codex children.
- Codex stderr content is suppressed rather than copied into OrangeDeck logs.

## Explicitly Absent

- No arbitrary remote shell endpoint.
- No remote path selection.
- No arbitrary environment-variable upload.
- No arbitrary AppleScript.
- No destructive Git operation, commit, or push.
- No direct network exposure of Codex App Server.
- No automatic approval.
- No edits to Codex config.toml, credentials or trust records. Explicit hook installation merges hooks.json with a private backup.
- No root service and no system-wide daemon.

## Tailscale

Being on the same home router is not used as a trust decision. Use the devices' `100.x`
Tailscale addresses. Tailscale supplies transport encryption and device identity; the
OrangeDeck token supplies application authentication.

The MVP intentionally uses HTTP inside the encrypted Tailscale tunnel and does not add
a second TLS layer. Do not expose port `45831` through router port forwarding, Tailscale
Funnel, public interfaces, or a reverse proxy. Restrict the Ally-to-Mac destination to
TCP port `45831` with a Tailscale grant when the tailnet contains untrusted devices.
Grants are the currently recommended access-control form; use the visual policy editor
so the user can review the exact Ally source and Mac destination before saving it.

Official references:

- [Tailscale grants](https://tailscale.com/docs/features/access-control/grants)
- [Tailscale visual policy editor](https://tailscale.com/kb/1587/visual-editor-reference)
- [Tailscale CLI and Taildrop](https://tailscale.com/kb/1080/cli)

## Pairing Token Handling

The generated `orangedeck-pairing.toml` contains the secret token. Transfer it with
Tailscale Taildrop or another private offline method, import it on the Ally, and remove
unneeded copies. Never paste it into chat, source control, issue reports, or logs.

To rotate the token, stop the Agent, run `init --force` with the same project settings,
transfer the new bundle, and pair the Ally again. The old Ally token then stops working.

## macOS Changes

Normal build, `doctor`, `init`, and `serve` do not change macOS System Settings. `init`
writes only the chosen OrangeDeck config directory. Host action buttons call the normal
user-level `open` command. The optional LaunchAgent template is not installed
automatically and must be reviewed and installed manually by the user.

## Residual Risk

Approved Codex operations and Cargo build scripts can execute code with the Mac user's
permissions. OrangeDeck reduces the remote command surface but cannot make an untrusted
registered repository safe. Register only projects you trust, inspect approval details,
and reject unexpected requests.

## Read-only session token adapter (0.1.5)

Only paths returned for listed Codex threads are considered, within the canonical local
Codex sessions directory. Filenames and session_meta IDs must match the listed UUID.
Regular files only; Unix opens use O_NOFOLLOW and O_NONBLOCK. Symlink escapes are rejected.
Reads are bounded (2 MiB per file, 100 listed files; 256 KiB header/line limits) and incremental.
The bounded session adapter extracts token counters, task boundaries, timestamps and the latest user question (up to 4000 characters) for listed conversations. Selected conversation reads can also send response text to the UI. Full raw logs, private reasoning and authentication files are not forwarded. Approval details can include tool arguments and paths; these are sensitive.
Session logs are never edited. There is no fs/read endpoint, watcher service, telemetry
exporter configuration, or third-party usage analytics connection.

## Explicitly authorized external approvals (0.1.6)

Existing Codex sessions accept explicit approval/decline only for requests arriving through the opt-in PermissionRequest hook. External thread start/resume/prompt/interrupt APIs remain forbidden.

The hook channel is a Unix socket in a private 0700 directory, mode 0600, with same-uid peer checks and 32 concurrent connections. Incoming JSON is bounded to 64 KiB; approvals exceeding 24 KiB of complete arguments stay in Codex. Prompt/transcript/reply fields from the hook payload are not forwarded. The separate conversation monitor may provide selected question/response text. The GUI can inspect the requested tool, cwd and arguments. No shell content is executed by the Agent. Only a fresh click/touch or physical A/B input resolves the specific request, once. Timeout (120 s), a closed hook client, no controller connection or an error never grants permission.

Hook installation is an explicit `codex-hooks --install` / Enable Codex Notifications.command action. It preserves unrelated hooks, saves a private backup, checks for concurrent changes, and leaves Codex hook trust to the user's /hooks review. config.toml and credentials are unchanged. No bypass-hook-trust or session-wide approval flag is set.

## Optional download server and public artifacts

The reusable `download-page/manage.py` serves a fixed page and explicitly published source ZIPs only to the configured tailnet addresses. Personal settings live in ignored `download-page/config.local.toml`. The manager verifies its process instance before stopping it. There is no upload or arbitrary file endpoint.

New packages use a source allow-list and common secret/private-data scanning. Both ZIP and TAR are validated, including reused releases, and must contain the same files, bytes and normalized permissions. Special files, unsafe paths, extra metadata, duplicates and oversized members are refused. The `export` command produces a separate GitHub source tree and file checksum manifest. Private handoffs, machine configuration, caches and runtime files are excluded. Historic packages are preserved locally and must be reviewed separately before any public upload. See [publication checks](PUBLICATION.md).

## Data on your paired display

Project paths, conversation titles, selected question/response text, approval arguments and usage statistics can identify you or reveal your work. Treat the paired display, screenshots and diagnostic output as private. Git author information and GitHub release assets need separate review. No source scan or dependency audit is a security guarantee.

## Installation permissions

The guided installer runs as the logged-in user. It creates OrangeDeck files and offers explicit native package/Rust installation; it never installs itself as root, signs into accounts, changes tailnet policy or enables a login daemon. Hook installation remains a separate explicit action after Agent startup, followed by user trust review in Codex. Generated launchers store paths and profile selection privately.

Missing configuration does not authorize overwriting an existing token. Guided setup stops for incomplete credential sets; native initialization/import checks all destinations before writing, including when explicit replacement is requested. File writes are atomic individually; this is not a multi-file transaction or a defense against compromise of the same local account.
