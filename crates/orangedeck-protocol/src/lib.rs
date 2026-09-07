//! Versioned network contract between OrangeDeck UI and Connector.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientRequest {
    pub protocol_version: u16,
    pub request_id: Uuid,
    pub command: ClientCommand,
}

impl ClientRequest {
    pub fn new(command: ClientCommand) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            command,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientCommand {
    SelectProject {
        project_id: String,
    },
    RefreshState,
    RunCargoCheck {
        project_id: String,
    },
    RunCargoTest {
        project_id: String,
    },
    RunCargoClippy {
        project_id: String,
    },
    RunCargoFmt {
        project_id: String,
    },
    RunCargoBuild {
        project_id: String,
    },
    CancelJob {
        job_id: Uuid,
    },
    RefreshGit {
        project_id: String,
    },
    OpenEditor {
        project_id: String,
    },
    OpenTerminal {
        project_id: String,
    },
    OpenBrowser {
        project_id: String,
    },
    OpenProject {
        project_id: String,
    },
    CodexRefreshThreads,
    /// Observe history only. Never resume, subscribe to, or control an external thread.
    CodexReadThread {
        thread_id: String,
    },
    CodexStartThread {
        project_id: String,
    },
    CodexSendPrompt {
        thread_id: String,
        prompt: String,
    },
    CodexInterrupt {
        thread_id: String,
        turn_id: String,
    },
    CodexApprovalResponse {
        approval_id: Uuid,
        decision: ApprovalDecisionDto,
    },
    DemoScenario {
        scenario: DemoScenario,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionDto {
    Approve,
    Reject,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DemoScenario {
    Approval,
    CargoFailure,
    Reconnect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandResponse {
    pub protocol_version: u16,
    pub request_id: Uuid,
    pub accepted: bool,
    pub message: String,
    pub job_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerEnvelope {
    pub protocol_version: u16,
    pub event_id: Uuid,
    pub emitted_at: DateTime<Utc>,
    pub event: ServerEvent,
}

impl ServerEnvelope {
    pub fn new(event: ServerEvent) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            event_id: Uuid::new_v4(),
            emitted_at: Utc::now(),
            event,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ServerEvent {
    Snapshot(SnapshotDto),
    ConnectionChanged(ConnectionChangedDto),
    JobStarted(JobDto),
    JobOutput(JobOutputDto),
    JobCompleted(JobDto),
    GitUpdated(GitDto),
    CodexConnectionChanged(CodexConnectionDto),
    CodexThreadsReplaced { threads: Vec<CodexThreadDto> },
    CodexThreadUpdated(CodexThreadDto),
    CodexTurnUpdated(CodexTurnUpdateDto),
    CodexApprovalRequested(ApprovalDto),
    CodexApprovalResolved { approval_id: Uuid },
    CodexUsageUpdated(CodexUsageUpdateDto),
    CodexLimitsUpdated(CodexLimitsDto),
    CodexAccountUsageUpdated(AccountUsageDto),
    CodexActivity(CodexActivityDto),
    CodexError { message: String },
    SystemUpdated(SystemDto),
    Notification(NotificationDto),
    CompatibilityError { expected: u16, received: u16 },
    Heartbeat { server_time: DateTime<Utc> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDto {
    pub host: HostDto,
    pub projects: Vec<ProjectDto>,
    pub selected_project_id: Option<String>,
    pub codex: CodexSnapshotDto,
    pub jobs: Vec<JobDto>,
    pub git: Vec<GitDto>,
    pub system: SystemDto,
    pub activity: Vec<String>,
    #[serde(default)]
    pub notifications: Vec<NotificationDto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub has_browser_url: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostDto {
    pub name: String,
    pub os: String,
    pub architecture: String,
    pub address: Option<String>,
    pub state: ConnectionStateDto,
    pub tailscale: bool,
    pub latency_ms: Option<u64>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStateDto {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionChangedDto {
    pub state: ConnectionStateDto,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKindDto {
    CargoCheck,
    CargoTest,
    CargoClippy,
    CargoFmtCheck,
    CargoBuild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatusDto {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobDto {
    pub id: Uuid,
    pub kind: JobKindDto,
    pub project_id: String,
    pub status: JobStatusDto,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub warning_count: u32,
    pub output_tail: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStreamDto {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobOutputDto {
    pub job_id: Uuid,
    pub stream: OutputStreamDto,
    pub sequence: u64,
    pub line: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCountsDto {
    pub modified: u32,
    pub staged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummaryDto {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFileDto {
    pub path: String,
    pub index_status: char,
    pub worktree_status: char,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitDto {
    pub hash: String,
    pub author: String,
    pub timestamp: i64,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitDto {
    pub project_id: String,
    pub branch: String,
    pub clean: bool,
    pub counts: GitCountsDto,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFileDto>,
    pub recent_commits: Vec<CommitDto>,
    pub diff: DiffSummaryDto,
    pub updated_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexConnectionStateDto {
    Connected,
    Connecting,
    Disconnected,
    Error,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexThreadStatusDto {
    NotLoaded,
    Idle,
    Working,
    WaitingApproval,
    Completed,
    Error,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadOwnershipDto {
    OrangeDeck,
    ExternalReadOnly,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsageDto {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub model_context_window: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodexThreadDto {
    pub id: String,
    pub project_id: Option<String>,
    pub cwd: String,
    pub title: String,
    pub preview: String,
    pub status: CodexThreadStatusDto,
    pub ownership: ThreadOwnershipDto,
    pub updated_at: i64,
    pub active_turn_id: Option<String>,
    pub token_usage: Option<TokenUsageDto>,
    #[serde(default)]
    pub observation: Option<ThreadObservationDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_usage: Option<LiveTokenUsageDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<ThreadActivityDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadActivityDto {
    pub turn_id: String,
    pub status: CodexThreadStatusDto,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub user_input: Option<String>,
    #[serde(default)]
    pub latest_user_prompt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveTokenUsageDto {
    pub turn_id: Option<String>,
    pub turn_tokens: Option<TokenUsageDto>,
    pub last_request: TokenUsageDto,
    pub recent_requests: Vec<i64>,
    pub status: CodexThreadStatusDto,
    pub updated_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadObservationDto {
    #[serde(default)]
    pub turn_id: Option<String>,
    pub latest_user_prompt: Option<String>,
    // Keep the v1 wire key readable by previously paired clients.
    #[serde(rename = "latest_agent_message", alias = "latest_codex_reply")]
    pub latest_codex_reply: Option<String>,
    pub last_turn_status: CodexThreadStatusDto,
    pub model: Option<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTokenUsageDto {
    pub date: String,
    pub tokens: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountUsageDto {
    pub lifetime_tokens: Option<i64>,
    #[serde(default)]
    pub peak_daily_tokens: Option<i64>,
    pub daily: Vec<DailyTokenUsageDto>,
    pub updated_at: Option<DateTime<Utc>>,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitWindowDto {
    pub used_percent: i32,
    pub remaining_percent: i32,
    pub resets_at: Option<i64>,
    pub window_duration_minutes: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLimitsDto {
    #[serde(default)]
    pub limit_id: Option<String>,
    #[serde(default)]
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub primary: Option<RateLimitWindowDto>,
    pub secondary: Option<RateLimitWindowDto>,
    pub credits_balance: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub additional: Vec<LimitBucketDto>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitBucketDto {
    pub id: String,
    pub name: Option<String>,
    pub primary: Option<RateLimitWindowDto>,
    pub secondary: Option<RateLimitWindowDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKindDto {
    CommandExecution,
    FileChange,
    Permissions,
    UserInput,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDto {
    pub id: Uuid,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub kind: ApprovalKindDto,
    pub title: String,
    pub summary: String,
    pub details: Vec<String>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodexConnectionDto {
    pub state: CodexConnectionStateDto,
    pub version: Option<String>,
    pub compatible: bool,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodexSnapshotDto {
    pub connection: CodexConnectionDto,
    pub threads: Vec<CodexThreadDto>,
    pub pending_approvals: Vec<ApprovalDto>,
    pub limits: Option<CodexLimitsDto>,
    #[serde(default)]
    pub account_usage: Option<AccountUsageDto>,
    pub supported_features: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexTurnUpdateDto {
    pub thread_id: String,
    pub turn_id: String,
    pub status: CodexThreadStatusDto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexUsageUpdateDto {
    pub thread_id: String,
    pub usage: TokenUsageDto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexActivityDto {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemDto {
    pub cpu_percent: Option<f32>,
    pub memory_used_bytes: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub load_average: Option<f32>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLevelDto {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    pub level: NotificationLevelDto,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub name: String,
    pub version: String,
    pub protocol_version: u16,
    pub ready: bool,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub expected_protocol_version: Option<u16>,
    pub received_protocol_version: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_rename_preserves_the_v1_codex_reply_field() {
        let old = serde_json::json!({
            "latest_user_prompt": "question", "latest_agent_message": "complete answer",
            "last_turn_status": "completed", "model": null,
            "observed_at": "2026-01-01T00:00:00Z"
        });
        let observation: ThreadObservationDto = serde_json::from_value(old).unwrap();
        assert_eq!(
            observation.latest_codex_reply.as_deref(),
            Some("complete answer")
        );
        let encoded = serde_json::to_value(&observation).unwrap();
        assert_eq!(encoded["latest_agent_message"], "complete answer");
        assert!(encoded.get("latest_codex_reply").is_none());
    }

    #[test]
    fn old_connector_threads_and_snapshots_remain_readable() {
        let thread = serde_json::json!({"id":"old","project_id":null,"cwd":"/tmp/old","title":"old","preview":"first question","status":"not_loaded","ownership":"external_read_only","updated_at":10,"active_turn_id":null,"token_usage":null});
        let snapshot: CodexSnapshotDto = serde_json::from_value(serde_json::json!({
            "connection":{"state":"connected","version":"0.146.0","compatible":false,"message":null},
            "threads":[thread],"pending_approvals":[],"limits":null,"supported_features":["thread_list"]
        })).unwrap();
        assert!(snapshot.account_usage.is_none());
        assert!(snapshot.threads[0].observation.is_none());
        assert!(snapshot.threads[0].live_usage.is_none());
    }

    #[test]
    fn client_command_round_trips_with_version() {
        let request = ClientRequest::new(ClientCommand::RunCargoCheck {
            project_id: "orange".to_owned(),
        });
        let json = serde_json::to_string(&request).expect("request serializes");
        let decoded: ClientRequest = serde_json::from_str(&json).expect("request deserializes");

        assert_eq!(decoded, request);
        assert!(json.contains("\"protocol_version\":1"));
        assert!(json.contains("\"type\":\"run_cargo_check\""));
    }

    #[test]
    fn protocol_has_no_arbitrary_shell_variant() {
        let attempted = serde_json::from_value::<ClientCommand>(serde_json::json!({
            "type": "shell",
            "payload": { "command": "echo unsafe" }
        }));

        assert!(attempted.is_err());
    }

    #[test]
    fn server_event_round_trips() {
        let envelope = ServerEnvelope::new(ServerEvent::Heartbeat {
            server_time: Utc::now(),
        });
        let value = serde_json::to_value(&envelope).expect("event serializes");
        let decoded: ServerEnvelope = serde_json::from_value(value).expect("event deserializes");

        assert_eq!(decoded, envelope);
    }
}
