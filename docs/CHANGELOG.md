# Changelog

## 0.1.23 — Connector naming and clearer setup

- The Mac component is now **OrangeDeck Connector** (한국어: **통신 모듈**). Product labels, executable/package names, launchers, example configs, build scripts and CI use the same name. Codex performs the AI work; OrangeDeck connects the devices and provides the remote controls.
- Setup imports older Mac settings into `connector.toml` and `connector.token` without rotating credentials, changing projects or removing the original files. Saved Codex profiles, hooks and owned-thread records stay in place. Existing UI settings and connection files remain readable. Run the notification installer and review `/hooks` after updating the Mac executable.
- Korean-first and English READMEs show the macOS Connector and CachyOS Handheld ROG Ally remote, a three-step installation diagram, a direct ZIP link and the exact files/commands to use. Steam Deck/SteamOS remain outside the current installation guide.

## 0.1.21

- Fix incomplete approval details: retain the full command, permissions and every supplied file change for review, including argument-array commands. Only the deck preview is shortened. Existing explicit, single-request decision rules are unchanged.
- Remove the repository's Security policy documents and their README navigation links. GitHub's built-in Security menu is controlled by GitHub; runtime authentication and automated checks remain enabled.
- Add regression coverage for long Unicode commands, trailing arguments, more than twelve file changes and large permission sets. Real Mac approval delivery still requires device verification.

## 0.1.20

- Make the default GitHub README Korean, with prominent one-click Korean/English links and matching native screenshots in each language.
- Add an immediate Korean/English toggle across the five tabs, a dark coral/violet/mint theme, original vector icons and refreshed square keys.
- Connect eight editable keys to 14 built-in actions: navigation, refresh, follow latest, project/conversation switching and registered host editor/terminal/folder/website launchers. Quick setup fills empty keys without overwriting existing choices.
- Keep 01/02 as fixed approval decisions. The action editor saves or cancels without executing an action or deciding a background request.
- Persist language/key preferences privately and atomically beside the UI config. Preserve malformed files, refuse links and display save failures; exclude preferences from publication.
- Include Korean font detection and guided installation for supported Linux package managers, including the Fedora font path.
- Detect supported installed editors and terminals on the host. Demo UI/Agent accept `--language ko|en` for matching synthetic content; real conversations remain unchanged.
- Update installation, controls and Korean quick-start instructions. Keep domain preferences, application command resolution and infrastructure persistence separate.
- Automated checks and native demo captures cover UI language, layout, key assignment, request guards and private persistence. Steam Deck/SteamOS installation is outside the current setup guide.

## 0.1.19

- Fix new/custom macOS Agent setup by selecting hook and owned-thread paths beside the active configuration. After updating an older Agent, rerun notification installation and Codex `/hooks` review.
- Preserve the configured Codex executable and profile on updates, validate existing configuration before replacing binaries, and generate diagnostic launchers for both roles.
- Accept quoted/escaped paths from Terminal drag-and-drop; add bounded account/network prerequisite probes and clearer installation progress.
- Use the selected Tailscale executable consistently in Agent/UI diagnostics, including the macOS app-bundled CLI.
- Default Git exclusion to private, explicitly allowing reviewed source/docs. Exclude local SSH credentials, generated launchers/metadata and private agent instructions; verify behavior with an actual temporary Git index.
- Add a checked inward-dependency policy for all six Rust crates, refresh architecture documentation, and provide repository-specific English/Korean installation, restart, diagnostic and update instructions. Windows support remains deferred.
- Native isolated tests cover fresh installation, pairing, preserved credentials/profile, generated hook installation and rejection of an invalid update before binary replacement. Real Mac notifications and approval delivery still require device testing.

## 0.1.18

- Authenticate API requests before reading/parsing request bodies or extracting WebSocket upgrades; reject unauthenticated slow uploads promptly.
- Preserve orphaned credentials when configuration is missing. Native setup checks all destinations, refuses collisions/symlinks/special files and requires explicit replacement of existing files.
- Check every historical filename even when blobs share identical content. Scan commit/tag metadata and sensitive filenames with redacted reports, reject shallow history and bound regular-file reads.
- Validate reused TAR files as well as ZIPs, requiring identical source content and permissions. Reject ambiguous paths, special files, unsafe permissions, extra metadata and oversized members.
- Add `python3 download-page/manage.py export` to prepare a separate GitHub source tree with a file checksum manifest; no Git initialization or upload. Existing archives and edited exports are preserved.
- Document exactly which files to publish and why local checks must precede a push.

A clean local export does not establish the state of an existing remote repository, its history or release assets.

## 0.1.17

- Guided per-user Mac/Linux Agent and Linux UI installation, private generated launchers and stable installed binaries. Existing pairing is preserved.
- English and Korean README, portable setup and updated usage/security documentation.
- Private machine settings and session handoffs excluded from public source; shared packaging policy and Git-index/history privacy checks.
- Direct HTTP connections bypass environment proxies; malformed TOML errors withhold source; public demo tokens are rejected in real pairing; bounded inbound WebSocket frames.
- Tailscale app-bundled CLI discovery on macOS and a pinned Rust toolchain.
- Public source packaging works without configuring the optional private download server. An earlier local 0.1.16 archive is preserved; use 0.1.17 for this corrected entrypoint.

Local validation: Rust workspace tests, strict Clippy, fmt, release workspace build and Apple Silicon Agent `cargo check` passed. Twelve installer/privacy checks include native Agent initialization, UI pairing and credential-preserving updates in temporary folders; eight package/server checks passed. Network/account commands in the installer integration check were simulated. RustSec reported no known advisories for 483 locked dependencies, and Gitleaks 8.30.1 found no secrets in the prepared public tree.

The GitHub workflows are provided but have not been run on the owner's repository in this workspace. Existing remote repository/history checks are separate from the local test results.

## 0.1.15

Approval keys show neutral unavailable states and consistent hover/press feedback.

## 0.1.14

Five-tab layout and 5 × 2 shortcut deck; current-turn approvals open the deck once. Eight keys remain unassigned.

## 0.1.9–0.1.13

Latest-conversation following, persistent completed request tokens, clearer remaining quota bars, account-statistic provenance and stronger current-turn alerts.

## Earlier releases

Native UI, authenticated Tailscale Agent, read-only conversation monitoring, explicit approval hooks and private reusable packaging tools.
