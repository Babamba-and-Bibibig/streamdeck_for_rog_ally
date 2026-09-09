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
