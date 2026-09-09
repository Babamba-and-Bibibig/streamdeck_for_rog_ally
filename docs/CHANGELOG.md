# Changelog

## 0.1.34 — Smaller source downloads

- Exclude development tests and test scripts from the public source and installation downloads. Preserve the app's build and installation files, illustrated guides and all user features.

## 0.1.33 — Language controls, English README and clear use terms

- Show full **한국어 / English** names in larger, high-contrast buttons at the top right of every tab. Switch immediately and remember the choice without changing conversation assignments.
- Translate remaining OrangeDeck status messages, desktop notification titles and hook approval labels in the selected UI language, including messages from existing Connectors. Preserve original questions, replies, paths and complete approval arguments.
- Make the default README English, with matching Korean/English editions and large language links. Preserve the existing Mac Connector compatibility.
- Apply limited-use terms to OrangeDeck's own code, while preserving separate third-party rights. Include library/font notices with the source and installed app.
- Keep illustrated setup and everyday-use guides in the download; exclude release-server tools and developer-only documents.

## 0.1.32 — Choose a file before opening the Mac editor

- The lower conversation key opens only a full-width list of file names, full paths and recorded changed lines. Remove the in-dialog diff and content preview.
- Open the Mac editor only after an explicit file selection. Loading, refreshing, moving focus and reopening a dialog do not launch it; pending requests retain only explicit selections.
- Keep deleted files visible without an open action. Preserve the existing recorded-line navigation and first-line fallback when no changed line is available.
- This Ally UI change works with the existing 0.1.31 Mac Connector.

## 0.1.31 — Preserve the latest file preview and harden event capture

- Show the last captured file contents alongside an existing Codex diff. Later deletion or recreation updates whether that file can be opened.
- Invalidate overlapping active captures before rejecting an over-capacity tool, mark duplicate tool starts as ambiguous, and discard results arriving after Stop/Interrupt.
- Apply credential-path exclusions regardless of ASCII case, including common CLI credential stores. Stop an unexpectedly finished macOS event loop without spinning.
- Keep event-only collection, bounded previews, explicit approvals and existing private settings. Add regressions for mixed edits, capacity rejection, credential paths, interruption and the file dialog.

## 0.1.30 — Receive changed paths from macOS file events

- Replace whole-folder before/after scans with FSEvents. A small edit no longer disappears because an unrelated folder exceeds the old 8,192-entry limit.
- Start observation before the tool runs and flush pending events when it finishes. Read only changed paths, including newly created files in subfolders; retain file, text and event-loss limits.
- Preserve Codex diffs. Shell edits without previous contents show an explicitly labeled current preview and open at the first line in the configured Mac editor.
- Keep exact conversation/turn/tool matching, private-path exclusions, explicit approvals and the existing hook installation flow.

## 0.1.29 — Capture files when Codex tools run

- Install and receive `PreToolUse`/`PostToolUse` hooks so local file edits and shell-generated files reach the existing Agents file dialog, even when persisted Codex history omits them.
- Compare bounded before/after file states, correlate exact conversation/turn/tool IDs, and retain changed text across history refresh and Connector restart. Unchanged user edits are excluded; ambiguous or incomplete captures remain visible as incomplete.
- Update an already-open file dialog when that same turn's changes arrive, keeping the selected file stable without repeatedly opening the editor.
- Keep reads inside the conversation folder, reject symlinks, skip common credentials/build artifacts, and bound time, memory and retained diffs. No new remote command or automatic approval is added.
- After updating, rerun the Mac notification installer and review/trust the new hooks in Codex `/hooks`. Captures apply to subsequent work.

## 0.1.28 — Focused file-review fixes and current guides

- Use complete turn-item arrays directly instead of fetching them twice. Fetch omitted items before deciding whether a turn contains edits.
- Show recovered deletions as file changes without attempting to open a deleted file or claiming there were no edits. Explain when file records or change details may be incomplete.
- Rework the Korean and English READMEs around LIVE and Agents with fourteen current demo captures and updated device diagrams. Remove the obsolete Notifications screenshots and align the everyday-use guide with the four tabs.

## 0.1.27 — File history recovery and a single Agents view

- Remove the separate Notifications tab. Keep response and approval dialogs in Agents; Y and the former notification entry points now open Agents.
- Read full, paginated turn items and retain a legacy read fallback. Recover omitted edits from successful `apply_patch` call/output pairs in the same conversation and turn's trusted local log.
- Distinguish incomplete tool/file records from confirmed no edits. Make loading and unavailable keys open a visible recovery dialog with retry; open recovered files only for the displayed turn while its dialog stays open.
- Keep cached file lists viewable after disconnection, and allow recorded-file navigation while Codex itself is disconnected but the Mac Connector is reachable. Preserve automatic conversation cwd selection and exact file/turn validation.

## 0.1.26 — Automatic Codex working folder

- Open recorded edits directly from the selected Codex conversation’s working folder, including new files and files in subfolders. Remove the separate folder-registration step and editor registry requirement.
- Keep exact conversation/turn/file checks, canonical path containment and the general command allow-list. Accept the 0.1.25 registration command as a compatible file open without saving grants.
- Retain prominent errors and retry controls. Explain the required Mac Connector update when an older version returns a registration error.

## 0.1.25 — Agents and recoverable Mac editor navigation

- Rename the conversation tab to **에이전트들 / Agents**.
- Let users review and register a conversation’s Mac folder from the file dialog, then open its recorded changed file. Save editor-only registrations separately from the general command allow-list.
- Accept real subfolders of registered projects and resolve relative files from the conversation cwd; keep exact turn/file checks and canonical path containment.
- Show file-opening errors and registration/retry actions above the file list. Complete disconnected file requests by navigation ID so they cannot remain stuck waiting.
- Explicitly show when no approval request has been received for the displayed question; preserve deliberate decisions for actual requests.

## 0.1.24 — Five paired Codex conversations

- Replace the individual action keys with five stable conversation columns: response/approval above, this turn's changed files below. Both keys pulse for new notices; a saved sound toggle controls a local chime.
- Show the question and reply in a dialog. Explicit approvals/rejections close it after delivery; outside tap and Close dismiss without deciding. Guard against changed requests, hidden controls, duplicate presses and stale replies.
- Open recorded file changes at their changed line in the configured Mac editor, with an Ally file list and diffs. Serialize navigation so the latest selection follows the prior handoff. No-edit questions show a disabled **파일 수정 없음 / No file changes** key; loading and missing records are distinct.
- Read only completed Codex file-edit records from that turn, with bounded Unicode-safe diffs. Require a registered project and matching thread/turn/file, reject outside paths and symlinks escaping the project, and pass literal editor arguments without a shell.
- Add setup/editor choices for Zed, VS Code, Cursor and VSCodium. Preserve language and existing private settings; assign the new conversation slots once after updating both devices.
- Disable incremental caches for development and tests to limit build artifact accumulation.

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
