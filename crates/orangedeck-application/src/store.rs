use std::sync::Arc;

use orangedeck_domain::{CodexEvent, CodexThreadStatus, DashboardState, JobEvent};
use orangedeck_protocol::SnapshotDto;
use tokio::sync::RwLock;

use crate::snapshot_to_dto;

#[derive(Clone, Debug)]
pub struct SharedDashboard(Arc<RwLock<DashboardState>>);

impl SharedDashboard {
    pub fn new(state: DashboardState) -> Self {
        Self(Arc::new(RwLock::new(state)))
    }

    pub async fn snapshot(&self) -> SnapshotDto {
        let state = self.0.read().await;
        snapshot_to_dto(&state)
    }

    pub async fn read<R>(&self, reader: impl FnOnce(&DashboardState) -> R) -> R {
        let state = self.0.read().await;
        reader(&state)
    }

    pub async fn update<R>(&self, writer: impl FnOnce(&mut DashboardState) -> R) -> R {
        let mut state = self.0.write().await;
        writer(&mut state)
    }

    pub async fn apply_job_event(&self, event: &JobEvent) {
        let mut state = self.0.write().await;
        match event {
            JobEvent::Started(job) => {
                state.jobs.insert(job.id, job.clone());
                trim_job_history(&mut state);
                state.push_activity(format!("{}: {:?}", job.kind.label(), job.status));
            }
            JobEvent::Completed(job) => {
                let mut completed = job.clone();
                if let Some(existing) = state.jobs.get(&job.id) {
                    completed.output_tail.clone_from(&existing.output_tail);
                    completed.warning_count = existing.warning_count;
                }
                state.jobs.insert(job.id, completed);
                trim_job_history(&mut state);
                state.push_activity(format!("{}: {:?}", job.kind.label(), job.status));
            }
            JobEvent::Output { job_id, line, .. } => {
                if let Some(job) = state.jobs.get_mut(job_id) {
                    job.push_output(line.clone());
                }
            }
        }
    }

    pub async fn apply_codex_event(&self, event: &CodexEvent) {
        let mut state = self.0.write().await;
        match event {
            CodexEvent::ConnectionChanged {
                state: next,
                message,
            } => {
                state.codex.connection = *next;
                state.codex.message.clone_from(message);
                if matches!(
                    next,
                    orangedeck_domain::CodexConnectionState::Disconnected
                        | orangedeck_domain::CodexConnectionState::Error
                        | orangedeck_domain::CodexConnectionState::Unsupported
                ) {
                    state.codex.pending_approvals.clear();
                    for thread in &mut state.codex.threads {
                        if matches!(
                            thread.status,
                            CodexThreadStatus::Working | CodexThreadStatus::WaitingApproval
                        ) {
                            thread.status = CodexThreadStatus::Unknown;
                            thread.active_turn_id = None;
                        }
                    }
                }
            }
            CodexEvent::ThreadsReplaced(threads) => {
                let mut replacement = threads.clone();
                for thread in &mut replacement {
                    if let Some(previous) = state
                        .codex
                        .threads
                        .iter()
                        .find(|previous| previous.id == thread.id)
                    {
                        if thread.token_usage.is_none() {
                            thread.token_usage.clone_from(&previous.token_usage);
                        }
                        if thread.observation.is_none() {
                            thread.observation.clone_from(&previous.observation);
                        }
                        if thread.active_turn_id.is_none()
                            && matches!(
                                thread.status,
                                CodexThreadStatus::Working | CodexThreadStatus::WaitingApproval
                            )
                        {
                            thread.active_turn_id.clone_from(&previous.active_turn_id);
                        }
                        merge_activity(thread, previous);
                    }
                }
                state.codex.threads = replacement;
            }
            CodexEvent::ThreadUpdated(thread) => {
                if let Some(existing) = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|existing| existing.id == thread.id)
                {
                    let mut updated = thread.clone();
                    if updated.token_usage.is_none() {
                        updated.token_usage.clone_from(&existing.token_usage);
                    }
                    if updated.observation.is_none() {
                        updated.observation.clone_from(&existing.observation);
                    }
                    if updated.active_turn_id.is_none()
                        && matches!(
                            updated.status,
                            CodexThreadStatus::Working | CodexThreadStatus::WaitingApproval
                        )
                    {
                        updated.active_turn_id.clone_from(&existing.active_turn_id);
                    }
                    merge_activity(&mut updated, existing);
                    *existing = updated;
                } else {
                    state.codex.threads.push(thread.clone());
                }
            }
            CodexEvent::TurnStarted { thread_id, turn_id } => {
                if let Some(thread) = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|thread| &thread.id == thread_id)
                {
                    thread.status = CodexThreadStatus::Working;
                    thread.active_turn_id = Some(turn_id.clone());
                    thread.updated_at = chrono::Utc::now().timestamp();
                    let now = chrono::Utc::now();
                    thread.activity = Some(Box::new(orangedeck_domain::ThreadActivity {
                        turn_id: turn_id.clone(),
                        status: CodexThreadStatus::Working,
                        started_at: Some(now),
                        updated_at: now,
                        observed_at: now,
                        user_input: None,
                        latest_user_prompt: None,
                    }));
                    thread.live_usage = None;
                    if thread
                        .observation
                        .as_ref()
                        .is_some_and(|value| value.turn_id.as_deref() != Some(turn_id))
                    {
                        thread.observation = None;
                    }
                }
            }
            CodexEvent::TurnCompleted {
                thread_id,
                turn_id,
                status,
            } => {
                if let Some(thread) = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|thread| &thread.id == thread_id)
                    && thread
                        .active_turn_id
                        .as_ref()
                        .is_none_or(|id| id == turn_id)
                {
                    thread.status = *status;
                    thread.active_turn_id = None;
                    thread.updated_at = chrono::Utc::now().timestamp();
                    let now = chrono::Utc::now();
                    let started_at = thread
                        .activity
                        .as_ref()
                        .filter(|value| value.turn_id == *turn_id)
                        .and_then(|value| value.started_at);
                    thread.activity = Some(Box::new(orangedeck_domain::ThreadActivity {
                        turn_id: turn_id.clone(),
                        status: *status,
                        started_at,
                        updated_at: now,
                        observed_at: now,
                        user_input: None,
                        latest_user_prompt: thread
                            .activity
                            .as_ref()
                            .filter(|value| value.turn_id == *turn_id)
                            .and_then(|value| value.latest_user_prompt.clone()),
                    }));
                }
            }
            CodexEvent::ApprovalRequested(approval) => {
                if let Some(thread_id) = &approval.thread_id
                    && let Some(thread) = state
                        .codex
                        .threads
                        .iter_mut()
                        .find(|thread| &thread.id == thread_id)
                {
                    thread.status = CodexThreadStatus::WaitingApproval;
                    if let Some(turn_id) = &approval.turn_id {
                        thread.active_turn_id = Some(turn_id.clone());
                    }
                }
                state.codex.pending_approvals.push(approval.clone());
            }
            CodexEvent::ApprovalResolved(approval_id) => {
                let thread_id = state
                    .codex
                    .pending_approvals
                    .iter()
                    .find(|approval| &approval.id == approval_id)
                    .and_then(|approval| approval.thread_id.clone());
                state
                    .codex
                    .pending_approvals
                    .retain(|approval| &approval.id != approval_id);
                if let Some(thread_id) = thread_id
                    && let Some(thread) = state
                        .codex
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == thread_id)
                    && thread.status == CodexThreadStatus::WaitingApproval
                {
                    thread.status = if thread.active_turn_id.is_some() {
                        CodexThreadStatus::Working
                    } else {
                        CodexThreadStatus::Idle
                    };
                }
            }
            CodexEvent::TokenUsageUpdated { thread_id, usage } => {
                if let Some(thread) = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|thread| &thread.id == thread_id)
                {
                    thread.token_usage = Some(usage.clone());
                }
            }
            CodexEvent::LimitsUpdated(limits) => state.codex.limits = Some(limits.clone()),
            CodexEvent::AccountUsageUpdated(usage) => {
                if usage.updated_at.is_none()
                    && let Some(previous) = &mut state.codex.account_usage
                {
                    previous
                        .unavailable_reason
                        .clone_from(&usage.unavailable_reason);
                } else {
                    state.codex.account_usage = Some(usage.clone());
                }
            }
            CodexEvent::Activity { kind, text, .. } => {
                state.push_activity(format!("Codex {kind}: {text}"));
            }
            CodexEvent::Error(message) => {
                state.codex.message = Some(message.clone());
                state.push_activity(format!("Codex error: {message}"));
            }
        }
    }
}

fn merge_activity(
    thread: &mut orangedeck_domain::CodexThread,
    previous: &orangedeck_domain::CodexThread,
) {
    if let Some(old) = &previous.activity
        && thread
            .activity
            .as_ref()
            .is_none_or(|new| old.updated_at > new.updated_at)
    {
        let observed = thread
            .activity
            .as_ref()
            .filter(|new| new.turn_id == old.turn_id && new.status == old.status)
            .map(|new| new.observed_at);
        let prompt = thread
            .activity
            .as_ref()
            .filter(|new| new.turn_id == old.turn_id)
            .and_then(|new| new.latest_user_prompt.clone());
        thread.activity.clone_from(&previous.activity);
        if let Some(observed) = observed
            && let Some(activity) = &mut thread.activity
        {
            activity.observed_at = activity.observed_at.max(observed);
            if activity.latest_user_prompt.is_none() {
                activity.latest_user_prompt = prompt;
            }
        }
    }
    if let Some(activity) = &thread.activity {
        if matches!(
            activity.status,
            CodexThreadStatus::Working | CodexThreadStatus::WaitingApproval
        ) {
            thread.active_turn_id = Some(activity.turn_id.clone());
        } else {
            thread.active_turn_id = None;
        }
        if thread.observation.as_ref().is_some_and(|value| {
            value
                .turn_id
                .as_deref()
                .is_some_and(|id| id != activity.turn_id)
        }) {
            thread.observation = None;
        }
    }
}

fn trim_job_history(state: &mut DashboardState) {
    const MAX_JOBS: usize = 50;
    let excess = state.jobs.len().saturating_sub(MAX_JOBS);
    if excess == 0 {
        return;
    }
    let mut terminal = state
        .jobs
        .values()
        .filter(|job| job.status.is_terminal())
        .map(|job| {
            (
                job.started_at.map_or(i64::MIN, |at| at.timestamp_millis()),
                job.id,
            )
        })
        .collect::<Vec<_>>();
    terminal.sort_unstable();
    for (_, id) in terminal.into_iter().take(excess) {
        state.jobs.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use orangedeck_domain::{
        ApprovalKind, ApprovalRequest, CodexConnectionState, CodexThread, JobKind, JobRecord,
        JobStatus, ProjectId, ThreadOwnership, TokenUsage,
    };
    use uuid::Uuid;

    use super::*;

    fn thread() -> CodexThread {
        CodexThread {
            live_usage: None,
            activity: None,
            observation: None,
            id: "thread-1".to_owned(),
            project_id: None,
            cwd: "/tmp/project".to_owned(),
            title: "Test thread".to_owned(),
            preview: String::new(),
            status: CodexThreadStatus::Working,
            ownership: ThreadOwnership::OrangeDeck,
            updated_at: 1,
            active_turn_id: Some("turn-1".to_owned()),
            token_usage: Some(TokenUsage {
                total_tokens: 42,
                ..TokenUsage::default()
            }),
        }
    }

    #[tokio::test]
    async fn account_usage_failure_keeps_the_last_value_and_its_original_time() {
        let state = SharedDashboard::new(DashboardState::new("test".to_owned(), vec![]));
        let observed = chrono::Utc::now();
        state
            .apply_codex_event(&CodexEvent::AccountUsageUpdated(
                orangedeck_domain::AccountUsage {
                    lifetime_tokens: Some(123),
                    updated_at: Some(observed),
                    ..Default::default()
                },
            ))
            .await;
        state
            .apply_codex_event(&CodexEvent::AccountUsageUpdated(
                orangedeck_domain::AccountUsage {
                    unavailable_reason: Some("unavailable".to_owned()),
                    ..Default::default()
                },
            ))
            .await;
        let snapshot = state.snapshot().await;
        let usage = snapshot.codex.account_usage.unwrap();
        assert_eq!(usage.lifetime_tokens, Some(123));
        assert_eq!(usage.updated_at, Some(observed));
        assert!(usage.unavailable_reason.is_some());
    }

    #[tokio::test]
    async fn a_hook_completion_keeps_the_question_received_in_the_next_log_poll() {
        let mut dashboard = DashboardState::new("host", Vec::new());
        dashboard.codex.threads.push(thread());
        let shared = SharedDashboard::new(dashboard);
        shared
            .apply_codex_event(&CodexEvent::TurnCompleted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                status: CodexThreadStatus::Completed,
            })
            .await;
        let mut recorded = thread();
        let now = chrono::Utc::now();
        recorded.activity = Some(Box::new(orangedeck_domain::ThreadActivity {
            turn_id: "turn-1".to_owned(),
            status: CodexThreadStatus::Completed,
            started_at: None,
            updated_at: now - chrono::Duration::seconds(2),
            observed_at: now,
            user_input: None,
            latest_user_prompt: Some("latest question".to_owned()),
        }));
        shared
            .apply_codex_event(&CodexEvent::ThreadsReplaced(vec![recorded]))
            .await;
        let snapshot = shared.snapshot().await;
        assert_eq!(
            snapshot.codex.threads[0]
                .activity
                .as_ref()
                .unwrap()
                .latest_user_prompt
                .as_deref(),
            Some("latest question")
        );
    }

    #[tokio::test]
    async fn thread_refresh_preserves_live_usage_and_turn_identity() {
        let mut dashboard = DashboardState::new("host", Vec::new());
        dashboard.codex.threads.push(thread());
        let shared = SharedDashboard::new(dashboard);
        let mut refreshed = thread();
        refreshed.token_usage = None;
        refreshed.active_turn_id = None;

        shared
            .apply_codex_event(&CodexEvent::ThreadsReplaced(vec![refreshed]))
            .await;

        shared
            .read(|state| {
                let thread = &state.codex.threads[0];
                assert_eq!(thread.active_turn_id.as_deref(), Some("turn-1"));
                assert_eq!(
                    thread.token_usage.as_ref().map(|usage| usage.total_tokens),
                    Some(42)
                );
            })
            .await;
    }

    #[tokio::test]
    async fn a_late_completion_or_history_refresh_cannot_replace_the_current_question() {
        let mut dashboard = DashboardState::new("host", Vec::new());
        dashboard.codex.threads.push(thread());
        let shared = SharedDashboard::new(dashboard);
        shared
            .apply_codex_event(&CodexEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: "new".to_owned(),
            })
            .await;
        shared
            .apply_codex_event(&CodexEvent::TurnCompleted {
                thread_id: "thread-1".to_owned(),
                turn_id: "old".to_owned(),
                status: CodexThreadStatus::Completed,
            })
            .await;
        let mut stale = thread();
        stale.active_turn_id = None;
        stale.status = CodexThreadStatus::NotLoaded;
        stale.observation = Some(orangedeck_domain::ThreadObservation {
            turn_id: Some("old".to_owned()),
            latest_user_prompt: Some("old question".to_owned()),
            latest_codex_reply: Some("old answer".to_owned()),
            last_turn_status: CodexThreadStatus::Completed,
            model: None,
            observed_at: chrono::Utc::now(),
        });
        shared
            .apply_codex_event(&CodexEvent::ThreadsReplaced(vec![stale]))
            .await;
        let snapshot = shared.snapshot().await;
        assert_eq!(
            snapshot.codex.threads[0].active_turn_id.as_deref(),
            Some("new")
        );
        assert!(snapshot.codex.threads[0].observation.is_none());
        assert_eq!(
            snapshot.codex.threads[0].activity.as_ref().unwrap().status,
            orangedeck_protocol::CodexThreadStatusDto::Working
        );
    }

    #[tokio::test]
    async fn approval_resolution_resumes_status_and_disconnect_clears_stale_control_state() {
        let approval_id = Uuid::new_v4();
        let mut dashboard = DashboardState::new("host", Vec::new());
        let mut waiting = thread();
        waiting.status = CodexThreadStatus::WaitingApproval;
        dashboard.codex.threads.push(waiting);
        dashboard.codex.pending_approvals.push(ApprovalRequest {
            turn_id: Some("turn-1".to_owned()),
            id: approval_id,
            thread_id: Some("thread-1".to_owned()),
            kind: ApprovalKind::CommandExecution,
            title: "Approval".to_owned(),
            summary: "cargo test".to_owned(),
            details: Vec::new(),
            requested_at: chrono::DateTime::default(),
        });
        let shared = SharedDashboard::new(dashboard);

        shared
            .apply_codex_event(&CodexEvent::ApprovalResolved(approval_id))
            .await;
        assert_eq!(
            shared.read(|state| state.codex.threads[0].status).await,
            CodexThreadStatus::Working
        );

        shared
            .apply_codex_event(&CodexEvent::ConnectionChanged {
                state: CodexConnectionState::Disconnected,
                message: Some("process exited".to_owned()),
            })
            .await;
        shared
            .read(|state| {
                assert!(state.codex.pending_approvals.is_empty());
                assert_eq!(state.codex.threads[0].status, CodexThreadStatus::Unknown);
                assert!(state.codex.threads[0].active_turn_id.is_none());
            })
            .await;
    }

    #[tokio::test]
    async fn completed_job_history_is_bounded() {
        let shared = SharedDashboard::new(DashboardState::new("host", Vec::new()));
        for offset in 0..55 {
            let id = Uuid::new_v4();
            let mut job =
                JobRecord::queued(id, JobKind::CargoCheck, ProjectId::new("project").unwrap());
            let at = chrono::DateTime::from_timestamp(offset, 0).unwrap();
            job.start(at).unwrap();
            shared
                .apply_job_event(&JobEvent::Started(job.clone()))
                .await;
            job.finish(JobStatus::Succeeded, at, Some(0), 1).unwrap();
            shared.apply_job_event(&JobEvent::Completed(job)).await;
        }

        assert_eq!(shared.read(|state| state.jobs.len()).await, 50);
    }
}
