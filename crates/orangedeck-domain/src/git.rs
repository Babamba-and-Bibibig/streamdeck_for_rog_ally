use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ProjectId;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCounts {
    pub modified: u32,
    pub staged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub path: String,
    pub index_status: char,
    pub worktree_status: char,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitSummary {
    pub hash: String,
    pub author: String,
    pub timestamp: i64,
    pub subject: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSnapshot {
    pub project_id: ProjectId,
    pub branch: String,
    pub clean: bool,
    pub counts: GitCounts,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFileStatus>,
    pub recent_commits: Vec<CommitSummary>,
    pub diff: DiffSummary,
    pub updated_at: DateTime<Utc>,
    pub error: Option<String>,
}
