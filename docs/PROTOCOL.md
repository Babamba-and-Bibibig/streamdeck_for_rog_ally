# OrangeDeck Protocol

## Transport

- Default Connector port: `45831`.
- Real mode: direct Tailscale IPv4 endpoint in `100.64.0.0/10` only.
- Demo mode: `127.0.0.1` only.
- Request transport: HTTP/1.1 JSON.
- Event transport: WebSocket JSON text frames.
- Authentication: `Authorization: Bearer <pre-shared-token>` on every `/api/v1/*` route.

HTTP is carried inside Tailscale's encrypted tunnel. Codex App Server remains local
stdio and is not part of this network protocol.

## Routes

- `GET /readyz`: public process readiness only; returns no host or project data.
- `GET /api/v1/health`: authenticated version, protocol, readiness, and `real`/`mock` mode.
- `GET /api/v1/snapshot`: authenticated complete dashboard state.
- `POST /api/v1/command`: authenticated typed `ClientRequest`.
- `GET /api/v1/events`: authenticated WebSocket event stream.

The WebSocket sends a complete snapshot first, then incremental events and heartbeats.
If a receiver lags, the Connector sends another complete snapshot.

## Versioning

Every request and event includes `protocol_version`. MVP is version `1`. A mismatched
HTTP command returns `426 Upgrade Required`; a mismatched UI event is not applied.
The UI also verifies Connector mode so a mock endpoint cannot appear as a real Mac.

## Client Commands

- Project: select, refresh state.
- Cargo: check, test, clippy, format check, build, cancel job.
- Git: refresh read-only state.
- Host: open Zed, Terminal, browser URL, or project with fixed operations.
- Codex: refresh threads, read a listed thread without resume/control, create owned thread, send prompt, interrupt owned turn, resolve
  one approval.
- Demo-only: approval, failed test, and reconnect scenarios.

There is no shell command, raw executable, raw argument list or AppleScript field. The changed-file command carries a path only to identify an already recorded and authorized edit, not for general file access.

## Server Events

- Snapshot and connection changes.
- Job started, output line, and completion.
- Git updates.
- Codex connection, thread, turn, approval, usage, limits, activity, and errors.
- System updates, user notifications, compatibility errors, and heartbeat.

Protocol types are defined in `crates/orangedeck-protocol/src/lib.rs` and round-trip
through serde tests.

## Additive monitor fields (0.1.3)

Protocol remains 1. CodexThreadDto.observation and CodexSnapshotDto.account_usage
default to None when absent, so UI 0.1.3 accepts Mac Connector 0.1.1 snapshots.
CodexAccountUsageUpdated carries nullable cumulative tokens, dated daily buckets,
last successful read time, and an optional unavailable reason. It is not a usage-limit
percentage or a billable-cost estimate. Thread observation carries the latest prompt,
same-turn reply, persisted turn status, optional model, and read time.

Clients gate CodexReadThread on supported_features containing read_only_monitor.
The real Connector verifies the ID against its listed threads. It never calls thread/resume
or turn/start for this command. Older clients are not guaranteed to understand new
event variants; update the UI before the Connector. On this Ally the UI is already updated.

## Additive session-log usage (0.1.5)

Protocol remains 1. CodexThreadDto.live_usage is optional and omitted when unavailable.
It travels through existing thread replacement/update and snapshot events, not a new RPC
or event type. The Connector advertises session_log_usage for this reader capability; that
flag alone does not mean a supported, active log was found.

LiveTokenUsageDto carries turn_id, optional turn_tokens (baseline delta), last_request,
up to 16 recent_requests totals, recorded status, updated_at (actual count event) and
observed_at (file read). It contains no account/session lifetime total, raw path, prompt,
tool output, or credentials. Old token_usage is not a substitute for this telemetry.
Clients must expire values using their original timestamps and honor disconnection.

## Additions in 0.1.6 (protocol 1 compatible)

SnapshotDto.notifications defaults to an empty list when absent. NotificationDto adds optional id, thread_id and created_at fields. The Connector retains at most 100 notifications in memory; stable IDs let the UI deduplicate reconnect history. Old 0.1.1 connectors remain readable.

The existing codex_approval_response command addresses a live request UUID. Requests can originate in OrangeDeck's own stdio app-server or in a same-user opt-in Codex hook. Only hook-origin requests can control external sessions. No new shell or external-thread control command is added. Features completion_notifications and codex_hooks indicate adapter support; codex_hooks_active indicates a hook has been received during this Connector run, not universal coverage of all Mac sessions.

## Additions in 0.1.9 (protocol 1 compatible)

`ThreadActivityDto.latest_user_prompt` optionally carries the latest explicit `event_msg/user_message.message` in the observed turn, bounded to 4000 Unicode characters. It is cleared at a new turn, retained at completion, and never populated from tool output. The UI uses a shared current-turn prompt selector for LIVE, conversations and notifications, falling back to matching `thread/read` observations. A stored thread title/first preview is not presented as the latest question. Reads remain bounded; unread questions are explicitly labelled.

`CodexLimitsDto.limit_id` and `limit_name` preserve the primary bucket's scope. The multi-bucket `codex` entry takes precedence over the legacy single-bucket view when both exist. Only returned windows become metric cards; model-specific five-hour quotas remain separately labelled. Missing fields remain compatible with 0.1.8 and earlier Connectors.

## Additions in 0.1.8 (protocol 1 compatible)

Optional `CodexThreadDto.activity` carries a recorded turn ID, lifecycle state, original activity/start times, observation time and explicit decision-question text. It works before token counts exist. `ThreadObservationDto.turn_id`, `ApprovalDto.turn_id` and `NotificationDto.turn_id` identify a single query, preventing earlier replies/notifications from attaching to a new turn. Missing fields retain compatibility with old Connectors; full current-query behavior requires the new Connector.

`CodexLimitsDto.additional` retains additional named quota buckets, and `AccountUsageDto.peak_daily_tokens` retains the official daily peak when supplied. Account statistics poll every 60 seconds; quotas every 15 seconds. Original timestamps and unavailable values are preserved.

The project catalog pages through non-archived `thread/list` results, including app-server and other Codex sessions. Duplicate IDs are removed. Repeated cursors or an unfinished catalog at 10,000 records fail the refresh instead of publishing a partial catalog. Token/activity reads remain bounded to 100 recent logs plus a few explicitly watched older conversations; histories are never resumed. Logs export explicit `request_user_input`/`request_user_input_async` question text but no tool answers, arbitrary output or private reasoning.

Project focus is a local UI selection keyed by the Mac's recorded absolute folder path. Discovered folders do not enter the Connector command allow-list. All four pages share that selection; the conversation page contains only that project's threads. No new remote action is introduced.

## Paired conversations and changed files (0.1.24, protocol 1)

`CodexWatchThreads` accepts up to five listed conversation IDs. `OpenCodexChange` carries a navigation UUID, conversation ID, turn ID and recorded file path. The Connector requires an exact current observation, takes the working folder from that known conversation automatically, rejects deleted/non-recorded targets, and resolves the canonical file inside that folder before calling a fixed editor adapter. No shell interpolation is used. Editor completions and approval completions are correlated independently.

`ThreadObservationDto.changes` defaults to absent for older Connectors. When present it contains up to 64 recorded files: kind, current/previous path, first changed line and diff. Diffs are bounded to 16 KiB per file and 64 KiB total, including repeated edits, with truncation flags. Sources include completed `fileChange` items and, since 0.1.29, local tool-hook observations matched by conversation, turn and tool-call IDs. The existing wire format is unchanged. Missing/partial history differs from a confirmed empty list. Replies are bounded to 32,000 Unicode characters; private reasoning is excluded.

The capabilities are `paired_conversations` and `turn_file_changes`. Update both devices for the new file keys. Older snapshots remain readable; missing data does not establish a file target or imply no edits. Conversation assignments and editor configuration remain private local settings.

## Automatic conversation folder (0.1.26, protocol 1)

The `conversation_editor_root` capability indicates automatic cwd selection for `OpenCodexChange`. The root comes from the Connector's known Codex conversation, not an arbitrary path supplied by the client. No folder-registration step or saved editor registry is required. Relative files resolve from that cwd; canonical files and symlinks must remain inside it. The general command project allow-list is unchanged.

For compatibility with 0.1.25 clients, `RegisterCodexProject` still accepts a navigation UUID, listed conversation ID, exact turn ID, `expected_cwd` and recorded file path. Current Connectors verify the additional cwd match and perform the same file open without saving a registration. New UIs send `OpenCodexChange` directly.

All file operations retain navigation correlation. Disconnected requests return a failure for their exact navigation UUID and are never replayed automatically. If an older Connector returns `editor_project_not_registered` or its earlier registered-project error, the UI shows an update instruction instead of requesting folder registration.

## Complete file-history reads and recovery (0.1.27, protocol 1)

Connector reads the latest user-bearing turn through full paginated history, with a legacy full-thread fallback for stores without pagination. Matching successful `apply_patch` calls from the same turn's trusted local session log can supplement empty app-server file records. The same recorded-file limits and automatic cwd checks apply to recovered changes. This extends the 0.1.24 collection path without changing the wire schema.

An absent `changes` value means unavailable history. An empty list with `truncated: true` means incomplete records, including tool execution without verifiable file records. Only a complete empty list can produce the inert **No file changes** key. Both unavailable and loading states can open the file dialog and send `CodexReadThread`; a fresh matching observation completes the request, while a changed turn, timeout or closed dialog prevents delayed editor navigation. The UI displays recovery and errors prominently.
