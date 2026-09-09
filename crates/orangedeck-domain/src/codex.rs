use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ProjectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexConnectionState {
    Connected,
    Connecting,
    Disconnected,
    Error,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexThreadStatus {
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
pub enum ThreadOwnership {
    OrangeDeck,
    ExternalReadOnly,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub model_context_window: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexThread {
    pub id: String,
    pub project_id: Option<ProjectId>,
    pub cwd: String,
    pub title: String,
    pub preview: String,
    pub status: CodexThreadStatus,
    pub ownership: ThreadOwnership,
    pub updated_at: i64,
    pub active_turn_id: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub observation: Option<ThreadObservation>,
    pub live_usage: Option<Box<LiveTokenUsage>>,
    #[serde(default)]
    pub activity: Option<Box<ThreadActivity>>,
}

/// Lifecycle and explicit user-input requests observed in this Mac's Codex records.
/// This is recent activity, not an OS process-liveness guarantee.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadActivity {
    pub turn_id: String,
    pub status: CodexThreadStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
    pub user_input: Option<String>,
    /// Latest explicit user message in this turn, bounded to 4000 characters.
    #[serde(default)]
    pub latest_user_prompt: Option<String>,
}

/// Bounded, read-only session-log observations, never account-wide totals.
/// `status` describes recorded lifecycle markers, not OS process liveness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveTokenUsage {
    pub turn_id: Option<String>,
    pub turn_tokens: Option<TokenUsage>,
    pub last_request: TokenUsage,
    pub recent_requests: Vec<i64>,
    pub status: CodexThreadStatus,
    pub updated_at: DateTime<Utc>,
    pub observed_at: DateTime<Utc>,
}

/// Read-only turn observations from history and local tool hooks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadObservation {
    #[serde(default)]
    pub turn_id: Option<String>,
    pub latest_user_prompt: Option<String>,
    pub latest_codex_reply: Option<String>,
    /// Only file changes recorded in this turn, never a project-wide Git diff.
    #[serde(default)]
    pub changes: Option<TurnChanges>,
    pub last_turn_status: CodexThreadStatus,
    pub model: Option<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnChanges {
    pub files: Vec<CodeChange>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeChange {
    pub path: String,
    pub previous_path: Option<String>,
    pub kind: CodeChangeKind,
    pub first_line: u32,
    pub truncated: bool,
}

impl CodeChange {
    /// The same exclusion policy applies to events, history, caches and navigation.
    pub fn is_listable(&self) -> bool {
        is_file_change_path(&self.path)
            && self
                .previous_path
                .as_deref()
                .is_none_or(is_file_change_path)
    }
}

pub fn is_file_change_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.chars().any(char::is_control)
        && std::path::Path::new(path).file_name().is_some()
        && std::path::Path::new(path)
            .components()
            .all(|part| match part {
                std::path::Component::Normal(name) => name
                    .to_str()
                    .is_some_and(|name| !excluded_change_component(name)),
                std::path::Component::RootDir | std::path::Component::CurDir => true,
                _ => false,
            })
}

pub fn excluded_change_component(name: &str) -> bool {
    let lowercase = name
        .bytes()
        .any(|byte| byte.is_ascii_uppercase())
        .then(|| name.to_ascii_lowercase());
    let name = lowercase.as_deref().unwrap_or(name);
    matches!(
        name,
        "." | ".."
            | ".git"
            | ".codex"
            | ".ssh"
            | ".aws"
            | ".gnupg"
            | ".config"
            | ".local"
            | "library"
            | ".netrc"
            | ".npmrc"
            | ".pypirc"
            | ".git-credentials"
            | ".docker"
            | ".kube"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".cache"
            | ".next"
            | ".tox"
            | "auth.json"
            | "credentials.json"
            | "hooks.json"
            | "file-changes.json"
            | "known_hosts"
            | "authorized_keys"
    ) || name.starts_with(".env")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || [".token", ".pem", ".key", ".p12", ".pfx", ".local.toml"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTokenUsage {
    pub date: String,
    pub tokens: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountUsage {
    pub lifetime_tokens: Option<i64>,
    #[serde(default)]
    pub peak_daily_tokens: Option<i64>,
    pub daily: Vec<DailyTokenUsage>,
    pub updated_at: Option<DateTime<Utc>>,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitWindow {
    pub used_percent: i32,
    pub resets_at: Option<i64>,
    pub window_duration_minutes: Option<i64>,
}

impl RateLimitWindow {
    pub fn remaining_percent(&self) -> i32 {
        100_i32.saturating_sub(self.used_percent).clamp(0, 100)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexLimits {
    #[serde(default)]
    pub limit_id: Option<String>,
    #[serde(default)]
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
    pub credits_balance: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub additional: Vec<LimitBucket>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitBucket {
    pub id: String,
    pub name: Option<String>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    CommandExecution,
    FileChange,
    Permissions,
    UserInput,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub kind: ApprovalKind,
    pub title: String,
    pub summary: String,
    pub details: Vec<String>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodexSnapshot {
    pub connection: CodexConnectionState,
    pub version: Option<String>,
    pub compatible: bool,
    pub message: Option<String>,
    pub threads: Vec<CodexThread>,
    pub pending_approvals: Vec<ApprovalRequest>,
    pub limits: Option<CodexLimits>,
    pub account_usage: Option<AccountUsage>,
    pub supported_features: BTreeSet<String>,
}

impl Default for CodexSnapshot {
    fn default() -> Self {
        Self {
            connection: CodexConnectionState::Disconnected,
            version: None,
            compatible: true,
            message: None,
            threads: Vec::new(),
            pending_approvals: Vec::new(),
            limits: None,
            account_usage: None,
            supported_features: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum CodexEvent {
    ConnectionChanged {
        state: CodexConnectionState,
        message: Option<String>,
    },
    ThreadsReplaced(Vec<CodexThread>),
    ThreadUpdated(CodexThread),
    TurnStarted {
        thread_id: String,
        turn_id: String,
    },
    TurnCompleted {
        thread_id: String,
        turn_id: String,
        status: CodexThreadStatus,
    },
    ApprovalRequested(ApprovalRequest),
    ApprovalResolved(Uuid),
    TokenUsageUpdated {
        thread_id: String,
        usage: TokenUsage,
    },
    LimitsUpdated(CodexLimits),
    AccountUsageUpdated(AccountUsage),
    Activity {
        thread_id: Option<String>,
        turn_id: Option<String>,
        kind: String,
        text: String,
    },
    Error(String),
}
