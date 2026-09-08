use std::collections::{HashSet, VecDeque};

use orangedeck_protocol::{
    CodexSnapshotDto, CodexThreadDto, CodexThreadStatusDto, CommandResponse, ConnectionStateDto,
    GitDto, JobDto, ServerEnvelope, ServerEvent, SnapshotDto,
};
use uuid::Uuid;

use crate::alerts::AlertCenter;

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum NetworkEvent {
    Connecting,
    Connected {
        latency_ms: u64,
    },
    Disconnected {
        message: String,
        retry_ms: u64,
    },
    Server(ServerEnvelope),
    CommandCompleted(Result<CommandResponse, String>),
    ApprovalCompleted {
        approval_id: Uuid,
        result: Result<CommandResponse, String>,
    },
    FileOpenCompleted {
        navigation_id: Uuid,
        result: Result<CommandResponse, String>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapshotState {
    Missing,
    Current,
    Stale,
}

pub struct UiModel {
    pub snapshot: Option<SnapshotDto>,
    pub connected: bool,
    pub connecting: bool,
    pub latency_ms: Option<u64>,
    pub connection_message: Option<String>,
    pub retry_ms: Option<u64>,
    pub alerts: AlertCenter,
    pub approvals_in_flight: HashSet<Uuid>,
    snapshot_state: SnapshotState,
    pub command_message: Option<String>,
    pub approval_error: Option<(Uuid, String)>,
    pub protocol_error: Option<String>,
    codex_output: VecDeque<(String, String)>,
}

impl Default for UiModel {
    fn default() -> Self {
        Self {
            snapshot: None,
            connected: false,
            connecting: true,
            latency_ms: None,
            connection_message: None,
            retry_ms: None,
            alerts: AlertCenter::default(),
            approvals_in_flight: HashSet::new(),
            snapshot_state: SnapshotState::Missing,
            command_message: None,
            approval_error: None,
            protocol_error: None,
            codex_output: VecDeque::new(),
        }
    }
}

impl UiModel {
    pub fn apply_network(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::Connecting => {
                if self.snapshot_state != SnapshotState::Missing {
                    self.snapshot_state = SnapshotState::Stale;
                }
                self.connected = false;
                self.connecting = true;
                self.latency_ms = None;
                self.connection_message = Some("Connecting to OrangeDeck Connector".to_owned());
            }
            NetworkEvent::Connected { latency_ms } => {
                if self.snapshot_state != SnapshotState::Missing {
                    self.snapshot_state = SnapshotState::Stale;
                }
                self.connected = true;
                self.connecting = false;
                self.latency_ms = Some(latency_ms);
                self.retry_ms = None;
                self.connection_message = None;
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.host.state = ConnectionStateDto::Connected;
                    snapshot.host.latency_ms = Some(latency_ms);
                }
            }
            NetworkEvent::Disconnected { message, retry_ms } => {
                if self.snapshot_state != SnapshotState::Missing {
                    self.snapshot_state = SnapshotState::Stale;
                }
                self.connected = false;
                self.approvals_in_flight.clear();
                self.connecting = false;
                self.latency_ms = None;
                self.retry_ms = Some(retry_ms);
                self.connection_message = Some(message);
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.host.state = ConnectionStateDto::Disconnected;
                    snapshot.host.latency_ms = None;
                }
            }
            NetworkEvent::Server(envelope) => self.apply_server(envelope),
            NetworkEvent::FileOpenCompleted { .. } => {}
            NetworkEvent::ApprovalCompleted {
                approval_id,
                result,
            } => match result {
                Ok(response) if response.accepted => {
                    self.command_message =
                        Some("선택을 전송했습니다. Mac의 처리 결과를 기다립니다.".to_owned());
                }
                Ok(response) => {
                    self.approvals_in_flight.remove(&approval_id);
                    self.approval_error = Some((approval_id, response.message.clone()));
                    self.command_message = Some(response.message);
                }
                Err(error) => {
                    self.approvals_in_flight.remove(&approval_id);
                    self.approval_error = Some((approval_id, error.clone()));
                    self.command_message = Some(error);
                }
            },
            NetworkEvent::CommandCompleted(result) => match result {
                Ok(response) => self.command_message = Some(response.message),
                Err(message) => self.command_message = Some(message),
            },
        }
    }

    fn apply_server(&mut self, envelope: ServerEnvelope) {
        if envelope.protocol_version != orangedeck_protocol::PROTOCOL_VERSION {
            self.protocol_error = Some(format!(
                "Protocol mismatch: UI {}, Connector {}",
                orangedeck_protocol::PROTOCOL_VERSION,
                envelope.protocol_version
            ));
            return;
        }
        match envelope.event {
            ServerEvent::Snapshot(snapshot) => {
                for notice in &snapshot.notifications {
                    self.alerts.record(
                        notice.clone(),
                        envelope.event_id,
                        envelope.emitted_at,
                        false,
                        self.snapshot_state == SnapshotState::Missing,
                    );
                }
                for approval in &snapshot.codex.pending_approvals {
                    self.alerts.approval(approval);
                }
                self.approvals_in_flight.retain(|id| {
                    snapshot
                        .codex
                        .pending_approvals
                        .iter()
                        .any(|approval| approval.id == *id)
                });
                self.snapshot_state = SnapshotState::Current;
                self.snapshot = Some(snapshot);
            }
            ServerEvent::ConnectionChanged(connection) => {
                self.connected = connection.state == ConnectionStateDto::Connected;
                self.latency_ms = connection.latency_ms;
                self.connection_message = connection.message;
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.host.state = connection.state;
                    snapshot.host.latency_ms = connection.latency_ms;
                }
            }
            ServerEvent::JobStarted(job) | ServerEvent::JobCompleted(job) => {
                self.upsert_job(job);
            }
            ServerEvent::JobOutput(output) => {
                if let Some(snapshot) = &mut self.snapshot
                    && let Some(job) = snapshot.jobs.iter_mut().find(|job| job.id == output.job_id)
                {
                    job.output_tail.push(output.line);
                    if job.output_tail.len() > 250 {
                        job.output_tail.remove(0);
                    }
                }
            }
            ServerEvent::GitUpdated(git) => self.upsert_git(git),
            ServerEvent::CodexConnectionChanged(connection) => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.codex.connection = connection;
                }
            }
            ServerEvent::CodexThreadsReplaced { threads } => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.codex.threads = threads;
                }
            }
            ServerEvent::CodexThreadUpdated(thread) => self.upsert_thread(thread),
            ServerEvent::CodexTurnUpdated(update) => {
                if let Some(snapshot) = &mut self.snapshot
                    && let Some(thread) = snapshot
                        .codex
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == update.thread_id)
                {
                    if update.status != CodexThreadStatusDto::Working
                        && thread
                            .active_turn_id
                            .as_ref()
                            .is_some_and(|id| id != &update.turn_id)
                    {
                        return;
                    }
                    if update.status == CodexThreadStatusDto::Working
                        && thread.active_turn_id.as_deref() != Some(&update.turn_id)
                    {
                        self.codex_output.retain(|(id, _)| id != &update.thread_id);
                        thread.observation = None;
                        thread.live_usage = None;
                    }
                    let now = envelope.emitted_at;
                    let started_at = if update.status == CodexThreadStatusDto::Working {
                        Some(now)
                    } else {
                        thread.activity.as_ref().and_then(|value| value.started_at)
                    };
                    thread.activity = Some(orangedeck_protocol::ThreadActivityDto {
                        turn_id: update.turn_id.clone(),
                        status: update.status,
                        started_at,
                        updated_at: now,
                        observed_at: now,
                        user_input: None,
                        latest_user_prompt: thread
                            .activity
                            .as_ref()
                            .filter(|value| value.turn_id == update.turn_id)
                            .and_then(|value| value.latest_user_prompt.clone()),
                    });
                    thread.updated_at = now.timestamp();
                    thread.status = update.status;
                    thread.active_turn_id =
                        (update.status == CodexThreadStatusDto::Working).then_some(update.turn_id);
                }
            }
            ServerEvent::CodexApprovalRequested(approval) => {
                self.alerts.approval(&approval);
                if let Some(snapshot) = &mut self.snapshot {
                    if let Some(thread_id) = &approval.thread_id
                        && let Some(thread) = snapshot
                            .codex
                            .threads
                            .iter_mut()
                            .find(|thread| &thread.id == thread_id)
                    {
                        thread.status = CodexThreadStatusDto::WaitingApproval;
                        if let Some(turn_id) = &approval.turn_id {
                            thread.active_turn_id = Some(turn_id.clone());
                        }
                    }
                    if !snapshot
                        .codex
                        .pending_approvals
                        .iter()
                        .any(|item| item.id == approval.id)
                    {
                        snapshot.codex.pending_approvals.push(approval);
                    }
                }
            }
            ServerEvent::CodexApprovalResolved { approval_id } => {
                self.approvals_in_flight.remove(&approval_id);
                if self
                    .approval_error
                    .as_ref()
                    .is_some_and(|(id, _)| *id == approval_id)
                {
                    self.approval_error = None;
                }
                self.alerts.mark_read(approval_id);
                if let Some(entry) = self
                    .alerts
                    .entries
                    .iter_mut()
                    .find(|entry| entry.id == approval_id)
                {
                    "승인 요청 종료".clone_into(&mut entry.notification.title);
                }
                if let Some(snapshot) = &mut self.snapshot {
                    resolve_approval(&mut snapshot.codex, approval_id);
                }
            }
            ServerEvent::CodexUsageUpdated(update) => {
                if let Some(snapshot) = &mut self.snapshot
                    && let Some(thread) = snapshot
                        .codex
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == update.thread_id)
                {
                    thread.token_usage = Some(update.usage);
                }
            }
            ServerEvent::CodexLimitsUpdated(limits) => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.codex.limits = Some(limits);
                }
            }
            ServerEvent::CodexActivity(activity) => {
                if activity.kind == "message"
                    && let Some(thread_id) = &activity.thread_id
                {
                    self.append_codex_output(thread_id, &activity.text);
                }
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot
                        .activity
                        .push(format!("Codex {}: {}", activity.kind, activity.text));
                    trim_activity(&mut snapshot.activity);
                }
            }
            ServerEvent::CodexAccountUsageUpdated(usage) => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.codex.account_usage = Some(usage);
                }
            }
            ServerEvent::CodexError { message } => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.codex.connection.message = Some(message.clone());
                    snapshot.activity.push(format!("Codex error: {message}"));
                    trim_activity(&mut snapshot.activity);
                }
            }
            ServerEvent::SystemUpdated(system) => {
                if let Some(snapshot) = &mut self.snapshot {
                    snapshot.system = system;
                }
            }
            ServerEvent::Notification(notification) => {
                self.alerts.record(
                    notification,
                    envelope.event_id,
                    envelope.emitted_at,
                    true,
                    false,
                );
            }
            ServerEvent::CompatibilityError { expected, received } => {
                self.protocol_error = Some(format!(
                    "Protocol mismatch: expected {expected}, received {received}"
                ));
            }
            ServerEvent::Heartbeat { .. } => {}
        }
    }

    pub fn begin_approval(&mut self, id: Uuid) -> bool {
        let started = self.approval_ready()
            && self.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .codex
                    .pending_approvals
                    .iter()
                    .any(|approval| approval.id == id)
            })
            && self.approvals_in_flight.insert(id);
        if started {
            self.approval_error = None;
        }
        started
    }

    pub fn file_navigation_ready(&self) -> bool {
        self.connected
            && self.snapshot_state == SnapshotState::Current
            && self.protocol_error.is_none()
    }

    pub fn approval_ready(&self) -> bool {
        self.connected
            && self.snapshot_state == SnapshotState::Current
            && self.protocol_error.is_none()
            && self.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.codex.connection.state
                    == orangedeck_protocol::CodexConnectionStateDto::Connected
            })
    }

    pub fn codex_reply(&self, thread_id: &str) -> Option<&str> {
        self.codex_output
            .iter()
            .find(|(id, _)| id == thread_id)
            .map(|(_, text)| text.as_str())
    }

    fn append_codex_output(&mut self, thread_id: &str, delta: &str) {
        let index = self
            .codex_output
            .iter()
            .position(|(id, _)| id == thread_id)
            .unwrap_or_else(|| {
                if self.codex_output.len() >= 20 {
                    self.codex_output.pop_front();
                }
                self.codex_output
                    .push_back((thread_id.to_owned(), String::new()));
                self.codex_output.len() - 1
            });
        let text = &mut self.codex_output[index].1;
        text.push_str(delta);
        if text.len() > 32_768 {
            let mut start = text.len() - 32_768;
            while !text.is_char_boundary(start) {
                start += 1;
            }
            text.drain(..start);
        }
    }

    fn upsert_job(&mut self, job: JobDto) {
        if let Some(snapshot) = &mut self.snapshot {
            if let Some(existing) = snapshot
                .jobs
                .iter_mut()
                .find(|existing| existing.id == job.id)
            {
                let existing_output = std::mem::take(&mut existing.output_tail);
                *existing = job;
                if existing.output_tail.is_empty() {
                    existing.output_tail = existing_output;
                }
            } else {
                snapshot.jobs.push(job);
            }
            snapshot
                .jobs
                .sort_by_key(|job| std::cmp::Reverse(job.started_at));
        }
    }

    fn upsert_git(&mut self, git: GitDto) {
        if let Some(snapshot) = &mut self.snapshot {
            if let Some(existing) = snapshot
                .git
                .iter_mut()
                .find(|existing| existing.project_id == git.project_id)
            {
                *existing = git;
            } else {
                snapshot.git.push(git);
            }
        }
    }

    fn upsert_thread(&mut self, thread: CodexThreadDto) {
        if let Some(snapshot) = &mut self.snapshot {
            if let Some(existing) = snapshot
                .codex
                .threads
                .iter_mut()
                .find(|existing| existing.id == thread.id)
            {
                *existing = thread;
            } else {
                snapshot.codex.threads.insert(0, thread);
            }
        }
    }
}

fn resolve_approval(codex: &mut CodexSnapshotDto, approval_id: Uuid) {
    let thread_id = codex
        .pending_approvals
        .iter()
        .find(|approval| approval.id == approval_id)
        .and_then(|approval| approval.thread_id.clone());
    codex
        .pending_approvals
        .retain(|approval| approval.id != approval_id);
    let Some(thread_id) = thread_id else {
        return;
    };
    let still_waiting = codex
        .pending_approvals
        .iter()
        .any(|approval| approval.thread_id.as_deref() == Some(&thread_id));
    if still_waiting {
        return;
    }
    if let Some(thread) = codex
        .threads
        .iter_mut()
        .find(|thread| thread.id == thread_id)
        && thread.status == CodexThreadStatusDto::WaitingApproval
    {
        thread.status = if thread.active_turn_id.is_some() {
            CodexThreadStatusDto::Working
        } else {
            CodexThreadStatusDto::Idle
        };
    }
}

fn trim_activity(activity: &mut Vec<String>) {
    if activity.len() > 100 {
        activity.drain(..activity.len() - 100);
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use orangedeck_protocol::{
        ApprovalDto, ApprovalKindDto, CodexConnectionDto, ConnectionChangedDto, ThreadOwnershipDto,
    };

    use super::*;

    #[test]
    fn approval_is_sent_once_and_requires_a_fresh_snapshot_after_reconnect() {
        let id = Uuid::new_v4();
        let snapshot: SnapshotDto = serde_json::from_value(serde_json::json!({
            "host":{"name":"TEST","os":"test","architecture":"test","address":null,"state":"connected","tailscale":false,"latency_ms":null,"last_seen":Utc::now()},
            "projects":[],"selected_project_id":null,"jobs":[],"git":[],"system":{},"activity":[],
            "codex":{"connection":{"state":"connected","version":null,"compatible":true,"message":null},"threads":[],
                "pending_approvals":[{"id":id,"thread_id":"test","kind":"command_execution","title":"test","summary":"cargo check","details":[],"requested_at":Utc::now()}],"limits":null,"supported_features":[]}
        })).unwrap();
        let mut model = UiModel::default();
        model.apply_network(NetworkEvent::Connected { latency_ms: 1 });
        model.apply_network(NetworkEvent::Server(ServerEnvelope::new(
            ServerEvent::Snapshot(snapshot.clone()),
        )));
        assert!(model.begin_approval(id));
        assert!(!model.begin_approval(id));
        model.apply_network(NetworkEvent::ApprovalCompleted {
            approval_id: id,
            result: Err("request failed".to_owned()),
        });
        assert_eq!(
            model.approval_error,
            Some((id, "request failed".to_owned()))
        );
        assert!(model.begin_approval(id));
        assert!(model.approval_error.is_none());
        model.apply_network(NetworkEvent::Disconnected {
            message: "offline".to_owned(),
            retry_ms: 1,
        });
        assert!(!model.begin_approval(id));
        model.apply_network(NetworkEvent::Connected { latency_ms: 1 });
        assert!(!model.begin_approval(id));
        model.apply_network(NetworkEvent::Server(ServerEnvelope::new(
            ServerEvent::Snapshot(snapshot),
        )));
        assert!(model.begin_approval(id));
        model.apply_network(NetworkEvent::Server(ServerEnvelope::new(
            ServerEvent::CodexApprovalResolved { approval_id: id },
        )));
        assert!(!model.begin_approval(id));
        assert_eq!(model.alerts.entries.len(), 1);
    }

    #[test]
    fn codex_reply_streams_are_separate_bounded_and_unicode_safe() {
        let mut model = UiModel::default();
        for (id, delta) in [("a", "안녕"), ("b", "other"), ("a", "하세요")] {
            model.apply_network(NetworkEvent::Server(ServerEnvelope::new(
                ServerEvent::CodexActivity(orangedeck_protocol::CodexActivityDto {
                    thread_id: Some(id.to_owned()),
                    turn_id: Some("turn".to_owned()),
                    kind: "message".to_owned(),
                    text: delta.to_owned(),
                }),
            )));
        }
        assert_eq!(model.codex_reply("a"), Some("안녕하세요"));
        assert_eq!(model.codex_reply("b"), Some("other"));
        model.append_codex_output("a", &"가".repeat(20_000));
        assert!(model.codex_reply("a").unwrap().len() <= 32_768);
        assert!(model.codex_reply("a").unwrap().ends_with('가'));
    }

    #[test]
    fn disconnect_is_a_state_not_a_crash() {
        let mut model = UiModel::default();
        model.apply_network(NetworkEvent::Connected { latency_ms: 8 });
        model.apply_network(NetworkEvent::Server(ServerEnvelope::new(
            ServerEvent::ConnectionChanged(ConnectionChangedDto {
                state: ConnectionStateDto::Disconnected,
                latency_ms: None,
                message: Some("Mac asleep".to_owned()),
            }),
        )));
        assert!(!model.connected);
        assert_eq!(model.connection_message.as_deref(), Some("Mac asleep"));
        let _ = Utc::now();
    }

    #[test]
    fn reconnect_attempt_is_not_reported_as_still_connected() {
        let mut model = UiModel::default();
        model.apply_network(NetworkEvent::Connected { latency_ms: 7 });
        model.apply_network(NetworkEvent::Connecting);

        assert!(!model.connected);
        assert!(model.connecting);
        assert!(model.latency_ms.is_none());
    }

    #[test]
    fn approval_resolution_immediately_restores_the_thread_state() {
        let approval_id = Uuid::new_v4();
        let mut codex = CodexSnapshotDto {
            account_usage: None,
            connection: CodexConnectionDto {
                state: orangedeck_protocol::CodexConnectionStateDto::Connected,
                version: Some("0.153.2".to_owned()),
                compatible: true,
                message: None,
            },
            threads: vec![CodexThreadDto {
                activity: None,
                live_usage: None,
                observation: None,
                id: "thread-1".to_owned(),
                project_id: None,
                cwd: "/tmp/project".to_owned(),
                title: "Thread".to_owned(),
                preview: String::new(),
                status: CodexThreadStatusDto::WaitingApproval,
                ownership: ThreadOwnershipDto::OrangeDeck,
                updated_at: 0,
                active_turn_id: Some("turn-1".to_owned()),
                token_usage: None,
            }],
            pending_approvals: vec![ApprovalDto {
                turn_id: Some("turn-1".to_owned()),
                id: approval_id,
                thread_id: Some("thread-1".to_owned()),
                kind: ApprovalKindDto::CommandExecution,
                title: "Approval".to_owned(),
                summary: "cargo check".to_owned(),
                details: Vec::new(),
                requested_at: Utc::now(),
            }],
            limits: None,
            supported_features: Vec::new(),
        };

        resolve_approval(&mut codex, approval_id);

        assert!(codex.pending_approvals.is_empty());
        assert_eq!(codex.threads[0].status, CodexThreadStatusDto::Working);
    }
}
