# OrangeDeck Protocol

## Transport

- Default Agent port: `45831`.
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
If a receiver lags, the Agent sends another complete snapshot.

## Versioning

Every request and event includes `protocol_version`. MVP is version `1`. A mismatched
HTTP command returns `426 Upgrade Required`; a mismatched UI event is not applied.
The UI also verifies Agent mode so a mock endpoint cannot appear as a real Mac.

## Client Commands

- Project: select, refresh state.
- Cargo: check, test, clippy, format check, build, cancel job.
- Git: refresh read-only state.
- Host: open Zed, Terminal, browser URL, or project with fixed operations.
- Codex: refresh threads, read a listed thread without resume/control, create owned thread, send prompt, interrupt owned turn, resolve
  one approval.
- Demo-only: approval, failed test, and reconnect scenarios.

There is no shell command, raw executable, raw argument list, filesystem path, or
AppleScript field in the wire contract.

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
default to None when absent, so UI 0.1.3 accepts Mac Agent 0.1.1 snapshots.
CodexAccountUsageUpdated carries nullable cumulative tokens, dated daily buckets,
last successful read time, and an optional unavailable reason. It is not a usage-limit
percentage or a billable-cost estimate. Thread observation carries the latest prompt,
same-turn reply, persisted turn status, optional model, and read time.

Clients gate CodexReadThread on supported_features containing read_only_monitor.
The real Agent verifies the ID against its listed threads. It never calls thread/resume
or turn/start for this command. Older clients are not guaranteed to understand new
event variants; update the UI before the Agent. On this Ally the UI is already updated.

## Additive session-log usage (0.1.5)

Protocol remains 1. CodexThreadDto.live_usage is optional and omitted when unavailable.
It travels through existing thread replacement/update and snapshot events, not a new RPC
or event type. The Agent advertises session_log_usage for this reader capability; that
flag alone does not mean a supported, active log was found.

LiveTokenUsageDto carries turn_id, optional turn_tokens (baseline delta), last_request,
up to 16 recent_requests totals, recorded status, updated_at (actual count event) and
observed_at (file read). It contains no account/session lifetime total, raw path, prompt,
tool output, or credentials. Old token_usage is not a substitute for this telemetry.
Clients must expire values using their original timestamps and honor disconnection.

## Additions in 0.1.6 (protocol 1 compatible)

SnapshotDto.notifications defaults to an empty list when absent. NotificationDto adds optional id, thread_id and created_at fields. The Agent retains at most 100 notifications in memory; stable IDs let the UI deduplicate reconnect history. Old 0.1.1 agents remain readable.

The existing codex_approval_response command addresses a live request UUID. Requests can originate in OrangeDeck's own stdio app-server or in a same-user opt-in Codex hook. Only hook-origin requests can control external sessions. No new shell or external-thread control command is added. Features completion_notifications and codex_hooks indicate adapter support; codex_hooks_active indicates a hook has been received during this Agent run, not universal coverage of all Mac sessions.

## Additions in 0.1.9 (protocol 1 compatible)

`ThreadActivityDto.latest_user_prompt` optionally carries the latest explicit `event_msg/user_message.message` in the observed turn, bounded to 4000 Unicode characters. It is cleared at a new turn, retained at completion, and never populated from tool output. The UI uses a shared current-turn prompt selector for LIVE, conversations and notifications, falling back to matching `thread/read` observations. A stored thread title/first preview is not presented as the latest question. Reads remain bounded; unread questions are explicitly labelled.

`CodexLimitsDto.limit_id` and `limit_name` preserve the primary bucket's scope. The multi-bucket `codex` entry takes precedence over the legacy single-bucket view when both exist. Only returned windows become metric cards; model-specific five-hour quotas remain separately labelled. Missing fields remain compatible with 0.1.8 and earlier Agents.

## Additions in 0.1.8 (protocol 1 compatible)

Optional `CodexThreadDto.activity` carries a recorded turn ID, lifecycle state, original activity/start times, observation time and explicit decision-question text. It works before token counts exist. `ThreadObservationDto.turn_id`, `ApprovalDto.turn_id` and `NotificationDto.turn_id` identify a single query, preventing earlier replies/notifications from attaching to a new turn. Missing fields retain compatibility with old Agents; full current-query behavior requires the new Agent.

`CodexLimitsDto.additional` retains additional named quota buckets, and `AccountUsageDto.peak_daily_tokens` retains the official daily peak when supplied. Account statistics poll every 60 seconds; quotas every 15 seconds. Original timestamps and unavailable values are preserved.

The project catalog pages through non-archived `thread/list` results, including app-server and agent sources. Duplicate IDs are removed. Repeated cursors or an unfinished catalog at 10,000 records fail the refresh instead of publishing a partial catalog. Token/activity reads remain bounded to 100 recent logs plus a few explicitly watched older conversations; histories are never resumed. Logs export explicit `request_user_input`/`request_user_input_async` question text but no tool answers, arbitrary output or private reasoning.

Project focus is a local UI selection keyed by the Mac's recorded absolute folder path. Discovered folders do not enter the Agent command allow-list. All four pages share that selection; the conversation page contains only that project's threads. No new remote action is introduced.
