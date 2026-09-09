use std::path::Path;

use chrono::Utc;
use orangedeck_domain::{
    CommitSummary, DiffSummary, GitCounts, GitFileStatus, GitSnapshot, Project,
};
use thiserror::Error;
use tokio::process::Command;

#[derive(Clone, Debug, Default)]
pub struct GitInspector;

impl GitInspector {
    pub async fn inspect(&self, project: &Project) -> Result<GitSnapshot, GitError> {
        let status = run_git(
            &project.path,
            &[
                "-c",
                "core.quotepath=false",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
            ],
        )
        .await?;
        let mut snapshot = parse_porcelain_v2(&status, project.id.clone());

        let log = run_git(
            &project.path,
            &["log", "-5", "--pretty=format:%h%x1f%an%x1f%ct%x1f%s"],
        )
        .await
        .unwrap_or_default();
        snapshot.recent_commits = parse_log(&log);

        let diff = run_git(
            &project.path,
            &["diff", "--no-ext-diff", "--no-textconv", "--shortstat"],
        )
        .await
        .unwrap_or_default();
        let staged = run_git(
            &project.path,
            &[
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-textconv",
                "--shortstat",
            ],
        )
        .await
        .unwrap_or_default();
        snapshot.diff =
            merge_diff_summaries(&parse_diff_summary(&diff), &parse_diff_summary(&staged));
        Ok(snapshot)
    }
}

async fn run_git(cwd: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .output()
        .await
        .map_err(GitError::Spawn)?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(GitError::Command(if message.is_empty() {
            format!("git exited with {}", output.status)
        } else {
            message
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn parse_porcelain_v2(input: &str, project_id: orangedeck_domain::ProjectId) -> GitSnapshot {
    let mut branch = "DETACHED".to_owned();
    let mut ahead = 0;
    let mut behind = 0;
    let mut counts = GitCounts::default();
    let mut files = Vec::new();

    let nul_delimited = input.contains('\0');
    let records = if nul_delimited {
        input.split('\0').collect::<Vec<_>>()
    } else {
        input.lines().collect::<Vec<_>>()
    };
    let mut record_index = 0;
    while record_index < records.len() {
        let line = records[record_index];
        record_index += 1;
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.head ") {
            value.clone_into(&mut branch);
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.ab ") {
            for component in value.split_whitespace() {
                if let Some(number) = component.strip_prefix('+') {
                    ahead = number.parse().unwrap_or(0);
                } else if let Some(number) = component.strip_prefix('-') {
                    behind = number.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("? ") {
            counts.untracked += 1;
            push_file(
                &mut files,
                GitFileStatus {
                    path: path.to_owned(),
                    index_status: '?',
                    worktree_status: '?',
                },
            );
            continue;
        }
        if line.starts_with("! ") {
            continue;
        }

        let record_type = line.split_once(' ').map(|(kind, _)| kind);
        let Some(record_type @ ("1" | "2" | "u")) = record_type else {
            continue;
        };
        let fields = tracked_fields(line, record_type);
        let mut status = fields[1].chars();
        let index_status = status.next().unwrap_or('.');
        let worktree_status = status.next().unwrap_or('.');
        if record_type == "u" {
            counts.conflicted += 1;
        }
        if index_status != '.' {
            counts.staged += 1;
        }
        if worktree_status != '.' {
            counts.modified += 1;
        }
        let path = fields.last().copied().unwrap_or_default();
        let path = if nul_delimited {
            if record_type == "2" {
                record_index = record_index.saturating_add(1);
            }
            path
        } else {
            path.rsplit_once('\t').map_or(path, |(_, renamed)| renamed)
        };
        push_file(
            &mut files,
            GitFileStatus {
                path: path.to_owned(),
                index_status,
                worktree_status,
            },
        );
    }

    GitSnapshot {
        project_id,
        branch,
        clean: files.is_empty(),
        counts,
        ahead,
        behind,
        files,
        recent_commits: Vec::new(),
        diff: DiffSummary::default(),
        updated_at: Utc::now(),
        error: None,
    }
}

fn push_file(files: &mut Vec<GitFileStatus>, file: GitFileStatus) {
    const MAX_FILES: usize = 250;
    if files.len() < MAX_FILES {
        files.push(file);
    }
}

fn tracked_fields<'a>(line: &'a str, record_type: &str) -> Vec<&'a str> {
    let maximum_fields = match record_type {
        "1" => 9,
        "2" => 10,
        "u" => 11,
        _ => 2,
    };
    line.splitn(maximum_fields, ' ').collect()
}

pub fn parse_log(input: &str) -> Vec<CommitSummary> {
    input
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\x1f');
            Some(CommitSummary {
                hash: parts.next()?.to_owned(),
                author: parts.next()?.to_owned(),
                timestamp: parts.next()?.parse().ok()?,
                subject: parts.next()?.to_owned(),
            })
        })
        .collect()
}

pub fn parse_diff_summary(input: &str) -> DiffSummary {
    let mut summary = DiffSummary {
        summary: input.split_whitespace().collect::<Vec<_>>().join(" "),
        ..DiffSummary::default()
    };
    for segment in input.split(',') {
        let segment = segment.trim();
        let count = segment
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        if segment.contains("file changed") || segment.contains("files changed") {
            summary.files_changed = summary.files_changed.saturating_add(count);
        } else if segment.contains("insertion") {
            summary.insertions = summary.insertions.saturating_add(count);
        } else if segment.contains("deletion") {
            summary.deletions = summary.deletions.saturating_add(count);
        }
    }
    summary
}

fn merge_diff_summaries(working: &DiffSummary, staged: &DiffSummary) -> DiffSummary {
    let summary = [working.summary.as_str(), staged.summary.as_str()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    DiffSummary {
        files_changed: working.files_changed.saturating_add(staged.files_changed),
        insertions: working.insertions.saturating_add(staged.insertions),
        deletions: working.deletions.saturating_add(staged.deletions),
        summary,
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("cannot start git: {0}")]
    Spawn(std::io::Error),
    #[error("git command failed: {0}")]
    Command(String),
}
