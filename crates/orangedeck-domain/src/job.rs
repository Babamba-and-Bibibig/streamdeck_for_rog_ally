use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::ProjectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    CargoCheck,
    CargoTest,
    CargoClippy,
    CargoFmtCheck,
    CargoBuild,
}

impl JobKind {
    pub const ALL: [Self; 5] = [
        Self::CargoCheck,
        Self::CargoTest,
        Self::CargoClippy,
        Self::CargoFmtCheck,
        Self::CargoBuild,
    ];

    pub const fn cargo_args(self) -> &'static [&'static str] {
        match self {
            Self::CargoCheck => &["check"],
            Self::CargoTest => &["test"],
            Self::CargoClippy => &["clippy"],
            Self::CargoFmtCheck => &["fmt", "--check"],
            Self::CargoBuild => &["build"],
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::CargoCheck => "Cargo Check",
            Self::CargoTest => "Cargo Test",
            Self::CargoClippy => "Clippy",
            Self::CargoFmtCheck => "Format Check",
            Self::CargoBuild => "Cargo Build",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: Uuid,
    pub kind: JobKind,
    pub project_id: ProjectId,
    pub status: JobStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub warning_count: u32,
    pub output_tail: Vec<String>,
}

impl JobRecord {
    pub fn queued(id: Uuid, kind: JobKind, project_id: ProjectId) -> Self {
        Self {
            id,
            kind,
            project_id,
            status: JobStatus::Queued,
            started_at: None,
            finished_at: None,
            exit_code: None,
            duration_ms: None,
            warning_count: 0,
            output_tail: Vec::new(),
        }
    }

    pub fn start(&mut self, at: DateTime<Utc>) -> Result<(), JobTransitionError> {
        if self.status != JobStatus::Queued {
            return Err(JobTransitionError {
                from: self.status,
                to: JobStatus::Running,
            });
        }
        self.status = JobStatus::Running;
        self.started_at = Some(at);
        Ok(())
    }

    pub fn finish(
        &mut self,
        status: JobStatus,
        at: DateTime<Utc>,
        exit_code: Option<i32>,
        duration_ms: u64,
    ) -> Result<(), JobTransitionError> {
        if self.status != JobStatus::Running || !status.is_terminal() {
            return Err(JobTransitionError {
                from: self.status,
                to: status,
            });
        }
        self.status = status;
        self.finished_at = Some(at);
        self.exit_code = exit_code;
        self.duration_ms = Some(duration_ms);
        Ok(())
    }

    pub fn push_output(&mut self, line: String) {
        const MAX_TAIL_LINES: usize = 250;
        if line.to_ascii_lowercase().contains("warning:") {
            self.warning_count = self.warning_count.saturating_add(1);
        }
        self.output_tail.push(line);
        if self.output_tail.len() > MAX_TAIL_LINES {
            let excess = self.output_tail.len() - MAX_TAIL_LINES;
            self.output_tail.drain(..excess);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub enum JobEvent {
    Started(JobRecord),
    Output {
        job_id: Uuid,
        stream: OutputStream,
        sequence: u64,
        line: String,
    },
    Completed(JobRecord),
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid job transition from {from:?} to {to:?}")]
pub struct JobTransitionError {
    pub from: JobStatus,
    pub to: JobStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_state_machine_accepts_only_valid_transitions() {
        let mut job = JobRecord::queued(
            Uuid::nil(),
            JobKind::CargoTest,
            ProjectId::new("orange").unwrap(),
        );
        let now = Utc::now();

        assert!(job.start(now).is_ok());
        assert!(job.start(now).is_err());
        assert!(job.finish(JobStatus::Queued, now, None, 1).is_err());
        assert!(job.finish(JobStatus::Succeeded, now, Some(0), 42).is_ok());
        assert!(job.finish(JobStatus::Failed, now, Some(1), 43).is_err());
    }
}
