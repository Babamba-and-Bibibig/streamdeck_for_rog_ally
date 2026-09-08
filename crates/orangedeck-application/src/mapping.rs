use orangedeck_domain as domain;
use orangedeck_protocol as dto;

pub fn snapshot_to_dto(state: &domain::DashboardState) -> dto::SnapshotDto {
    dto::SnapshotDto {
        host: host_to_dto(&state.host),
        projects: state.projects.iter().map(project_to_dto).collect(),
        selected_project_id: state
            .selected_project
            .as_ref()
            .map(|project_id| project_id.as_str().to_owned()),
        codex: codex_snapshot_to_dto(&state.codex),
        jobs: state.jobs.values().map(job_to_dto).collect(),
        git: state.git.values().map(git_to_dto).collect(),
        system: system_to_dto(&state.system),
        activity: state.activity.iter().cloned().collect(),
        notifications: Vec::new(),
    }
}

pub fn project_to_dto(project: &domain::Project) -> dto::ProjectDto {
    dto::ProjectDto {
        id: project.id.as_str().to_owned(),
        name: project.name.clone(),
        path: project.path.to_string_lossy().into_owned(),
        has_browser_url: project.browser_url.is_some(),
    }
}

pub fn host_to_dto(host: &domain::HostSnapshot) -> dto::HostDto {
    dto::HostDto {
        name: host.name.clone(),
        os: host.os.clone(),
        architecture: host.architecture.clone(),
        address: host.address.clone(),
        state: match host.connection {
            domain::ConnectionState::Connecting => dto::ConnectionStateDto::Connecting,
            domain::ConnectionState::Connected => dto::ConnectionStateDto::Connected,
            domain::ConnectionState::Disconnected => dto::ConnectionStateDto::Disconnected,
            domain::ConnectionState::Error => dto::ConnectionStateDto::Error,
        },
        tailscale: host.tailscale,
        latency_ms: host.latency_ms,
        last_seen: host.last_seen,
    }
}

pub fn job_to_dto(job: &domain::JobRecord) -> dto::JobDto {
    dto::JobDto {
        id: job.id,
        kind: match job.kind {
            domain::JobKind::CargoCheck => dto::JobKindDto::CargoCheck,
            domain::JobKind::CargoTest => dto::JobKindDto::CargoTest,
            domain::JobKind::CargoClippy => dto::JobKindDto::CargoClippy,
            domain::JobKind::CargoFmtCheck => dto::JobKindDto::CargoFmtCheck,
            domain::JobKind::CargoBuild => dto::JobKindDto::CargoBuild,
        },
        project_id: job.project_id.as_str().to_owned(),
        status: match job.status {
            domain::JobStatus::Queued => dto::JobStatusDto::Queued,
            domain::JobStatus::Running => dto::JobStatusDto::Running,
            domain::JobStatus::Succeeded => dto::JobStatusDto::Succeeded,
            domain::JobStatus::Failed => dto::JobStatusDto::Failed,
            domain::JobStatus::Cancelled => dto::JobStatusDto::Cancelled,
        },
        started_at: job.started_at,
        finished_at: job.finished_at,
        exit_code: job.exit_code,
        duration_ms: job.duration_ms,
        warning_count: job.warning_count,
        output_tail: job.output_tail.clone(),
    }
}

pub fn output_stream_to_dto(stream: domain::OutputStream) -> dto::OutputStreamDto {
    match stream {
        domain::OutputStream::Stdout => dto::OutputStreamDto::Stdout,
        domain::OutputStream::Stderr => dto::OutputStreamDto::Stderr,
    }
}

pub fn git_to_dto(git: &domain::GitSnapshot) -> dto::GitDto {
    dto::GitDto {
        project_id: git.project_id.as_str().to_owned(),
        branch: git.branch.clone(),
        clean: git.clean,
        counts: dto::GitCountsDto {
            modified: git.counts.modified,
            staged: git.counts.staged,
            untracked: git.counts.untracked,
            conflicted: git.counts.conflicted,
        },
        ahead: git.ahead,
        behind: git.behind,
        files: git
            .files
            .iter()
            .map(|file| dto::GitFileDto {
                path: file.path.clone(),
                index_status: file.index_status,
                worktree_status: file.worktree_status,
            })
            .collect(),
        recent_commits: git
            .recent_commits
            .iter()
            .map(|commit| dto::CommitDto {
                hash: commit.hash.clone(),
                author: commit.author.clone(),
                timestamp: commit.timestamp,
                subject: commit.subject.clone(),
            })
            .collect(),
        diff: dto::DiffSummaryDto {
            files_changed: git.diff.files_changed,
            insertions: git.diff.insertions,
            deletions: git.diff.deletions,
            summary: git.diff.summary.clone(),
        },
        updated_at: git.updated_at,
        error: git.error.clone(),
    }
}

pub fn codex_thread_to_dto(thread: &domain::CodexThread) -> dto::CodexThreadDto {
    dto::CodexThreadDto {
        id: thread.id.clone(),
        project_id: thread
            .project_id
            .as_ref()
            .map(|project_id| project_id.as_str().to_owned()),
        cwd: thread.cwd.clone(),
        title: thread.title.clone(),
        preview: thread.preview.clone(),
        status: codex_status_to_dto(thread.status),
        ownership: match thread.ownership {
            domain::ThreadOwnership::OrangeDeck => dto::ThreadOwnershipDto::OrangeDeck,
            domain::ThreadOwnership::ExternalReadOnly => dto::ThreadOwnershipDto::ExternalReadOnly,
        },
        updated_at: thread.updated_at,
        active_turn_id: thread.active_turn_id.clone(),
        token_usage: thread.token_usage.as_ref().map(token_usage_to_dto),
        live_usage: thread
            .live_usage
            .as_ref()
            .map(|value| dto::LiveTokenUsageDto {
                turn_id: value.turn_id.clone(),
                turn_tokens: value.turn_tokens.as_ref().map(token_usage_to_dto),
                last_request: token_usage_to_dto(&value.last_request),
                recent_requests: value.recent_requests.clone(),
                status: codex_status_to_dto(value.status),
                updated_at: value.updated_at,
                observed_at: value.observed_at,
            }),
        observation: thread
            .observation
            .as_ref()
            .map(|value| dto::ThreadObservationDto {
                turn_id: value.turn_id.clone(),
                latest_user_prompt: value.latest_user_prompt.clone(),
                latest_codex_reply: value.latest_codex_reply.clone(),
                changes: value.changes.as_ref().map(turn_changes_to_dto),
                last_turn_status: codex_status_to_dto(value.last_turn_status),
                model: value.model.clone(),
                observed_at: value.observed_at,
            }),
        activity: thread
            .activity
            .as_ref()
            .map(|value| dto::ThreadActivityDto {
                turn_id: value.turn_id.clone(),
                status: codex_status_to_dto(value.status),
                started_at: value.started_at,
                updated_at: value.updated_at,
                observed_at: value.observed_at,
                user_input: value.user_input.clone(),
                latest_user_prompt: value.latest_user_prompt.clone(),
            }),
    }
}

pub fn turn_changes_to_dto(changes: &domain::TurnChanges) -> dto::TurnChangesDto {
    dto::TurnChangesDto {
        truncated: changes.truncated,
        files: changes
            .files
            .iter()
            .map(|file| dto::CodeChangeDto {
                path: file.path.clone(),
                previous_path: file.previous_path.clone(),
                kind: match file.kind {
                    domain::CodeChangeKind::Added => dto::CodeChangeKindDto::Added,
                    domain::CodeChangeKind::Modified => dto::CodeChangeKindDto::Modified,
                    domain::CodeChangeKind::Deleted => dto::CodeChangeKindDto::Deleted,
                    domain::CodeChangeKind::Renamed => dto::CodeChangeKindDto::Renamed,
                },
                first_line: file.first_line,
                diff: file.diff.clone(),
                content: file.content.clone(),
                truncated: file.truncated,
            })
            .collect(),
    }
}

pub fn codex_status_to_dto(status: domain::CodexThreadStatus) -> dto::CodexThreadStatusDto {
    match status {
        domain::CodexThreadStatus::NotLoaded => dto::CodexThreadStatusDto::NotLoaded,
        domain::CodexThreadStatus::Idle => dto::CodexThreadStatusDto::Idle,
        domain::CodexThreadStatus::Working => dto::CodexThreadStatusDto::Working,
        domain::CodexThreadStatus::WaitingApproval => dto::CodexThreadStatusDto::WaitingApproval,
        domain::CodexThreadStatus::Completed => dto::CodexThreadStatusDto::Completed,
        domain::CodexThreadStatus::Error => dto::CodexThreadStatusDto::Error,
        domain::CodexThreadStatus::Unknown => dto::CodexThreadStatusDto::Unknown,
    }
}

pub fn approval_to_dto(approval: &domain::ApprovalRequest) -> dto::ApprovalDto {
    dto::ApprovalDto {
        id: approval.id,
        thread_id: approval.thread_id.clone(),
        turn_id: approval.turn_id.clone(),
        kind: match approval.kind {
            domain::ApprovalKind::CommandExecution => dto::ApprovalKindDto::CommandExecution,
            domain::ApprovalKind::FileChange => dto::ApprovalKindDto::FileChange,
            domain::ApprovalKind::Permissions => dto::ApprovalKindDto::Permissions,
            domain::ApprovalKind::UserInput => dto::ApprovalKindDto::UserInput,
            domain::ApprovalKind::Other => dto::ApprovalKindDto::Other,
        },
        title: approval.title.clone(),
        summary: approval.summary.clone(),
        details: approval.details.clone(),
        requested_at: approval.requested_at,
    }
}

pub fn token_usage_to_dto(usage: &domain::TokenUsage) -> dto::TokenUsageDto {
    dto::TokenUsageDto {
        input_tokens: usage.input_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_output_tokens: usage.reasoning_output_tokens,
        total_tokens: usage.total_tokens,
        model_context_window: usage.model_context_window,
    }
}

pub fn limits_to_dto(limits: &domain::CodexLimits) -> dto::CodexLimitsDto {
    let window = |value: &domain::RateLimitWindow| dto::RateLimitWindowDto {
        used_percent: value.used_percent,
        remaining_percent: value.remaining_percent(),
        resets_at: value.resets_at,
        window_duration_minutes: value.window_duration_minutes,
    };
    dto::CodexLimitsDto {
        limit_id: limits.limit_id.clone(),
        limit_name: limits.limit_name.clone(),
        plan_type: limits.plan_type.clone(),
        primary: limits.primary.as_ref().map(window),
        secondary: limits.secondary.as_ref().map(window),
        credits_balance: limits.credits_balance.clone(),
        updated_at: limits.updated_at,
        additional: limits
            .additional
            .iter()
            .map(|bucket| dto::LimitBucketDto {
                id: bucket.id.clone(),
                name: bucket.name.clone(),
                primary: bucket.primary.as_ref().map(window),
                secondary: bucket.secondary.as_ref().map(window),
            })
            .collect(),
    }
}

pub fn codex_connection_to_dto(codex: &domain::CodexSnapshot) -> dto::CodexConnectionDto {
    dto::CodexConnectionDto {
        state: match codex.connection {
            domain::CodexConnectionState::Connected => dto::CodexConnectionStateDto::Connected,
            domain::CodexConnectionState::Connecting => dto::CodexConnectionStateDto::Connecting,
            domain::CodexConnectionState::Disconnected => {
                dto::CodexConnectionStateDto::Disconnected
            }
            domain::CodexConnectionState::Error => dto::CodexConnectionStateDto::Error,
            domain::CodexConnectionState::Unsupported => dto::CodexConnectionStateDto::Unsupported,
        },
        version: codex.version.clone(),
        compatible: codex.compatible,
        message: codex.message.clone(),
    }
}

pub fn account_usage_to_dto(usage: &domain::AccountUsage) -> dto::AccountUsageDto {
    dto::AccountUsageDto {
        lifetime_tokens: usage.lifetime_tokens,
        peak_daily_tokens: usage.peak_daily_tokens,
        daily: usage
            .daily
            .iter()
            .map(|day| dto::DailyTokenUsageDto {
                date: day.date.clone(),
                tokens: day.tokens,
            })
            .collect(),
        updated_at: usage.updated_at,
        unavailable_reason: usage.unavailable_reason.clone(),
    }
}

pub fn codex_snapshot_to_dto(codex: &domain::CodexSnapshot) -> dto::CodexSnapshotDto {
    dto::CodexSnapshotDto {
        connection: codex_connection_to_dto(codex),
        threads: codex.threads.iter().map(codex_thread_to_dto).collect(),
        pending_approvals: codex
            .pending_approvals
            .iter()
            .map(approval_to_dto)
            .collect(),
        limits: codex.limits.as_ref().map(limits_to_dto),
        account_usage: codex.account_usage.as_ref().map(account_usage_to_dto),
        supported_features: codex.supported_features.iter().cloned().collect(),
    }
}

pub fn system_to_dto(system: &domain::SystemSnapshot) -> dto::SystemDto {
    dto::SystemDto {
        cpu_percent: system.cpu_percent,
        memory_used_bytes: system.memory_used_bytes,
        memory_total_bytes: system.memory_total_bytes,
        load_average: system.load_average,
        uptime_seconds: system.uptime_seconds,
    }
}
