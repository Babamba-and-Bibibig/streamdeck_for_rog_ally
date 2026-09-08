use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use orangedeck_domain::UiLanguage;
use orangedeck_protocol::{
    ApprovalDecisionDto, ApprovalDto, ApprovalKindDto, ClientCommand, CodexActivityDto,
    CodexConnectionDto, CodexConnectionStateDto, CodexLimitsDto, CodexSnapshotDto, CodexThreadDto,
    CodexThreadStatusDto, CodexTurnUpdateDto, CodexUsageUpdateDto, CommitDto, ConnectionChangedDto,
    ConnectionStateDto, DiffSummaryDto, GitCountsDto, GitDto, HostDto, JobDto, JobKindDto,
    JobOutputDto, JobStatusDto, NotificationDto, NotificationLevelDto, OutputStreamDto, ProjectDto,
    RateLimitWindowDto, ServerEnvelope, ServerEvent, SnapshotDto, SystemDto, ThreadOwnershipDto,
    TokenUsageDto,
};
use tokio::sync::{Mutex, RwLock, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::server::{BackendError, BackendResult, ConnectorBackend};

const DEMO_PROJECT_ID: &str = "orange-project";

#[derive(Clone)]
pub struct MockBackend {
    inner: Arc<MockInner>,
}

struct MockInner {
    language: UiLanguage,
    state: RwLock<SnapshotDto>,
    events: broadcast::Sender<ServerEnvelope>,
    cancellations: Mutex<HashMap<Uuid, CancellationToken>>,
    turn_cancellations: Mutex<HashMap<String, CancellationToken>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self::with_english_demo(false)
    }

    pub fn with_english_demo(english: bool) -> Self {
        let (events, _) = broadcast::channel(512);
        let mut snapshot = demo_snapshot();
        if english {
            for thread in &mut snapshot.codex.threads {
                if let Some(observation) = &mut thread.observation {
                    observation.latest_user_prompt = Some("Build a clean LIVE dashboard so I can follow my Codex project, tokens and remaining quotas at a glance.".to_owned());
                    observation.latest_codex_reply = Some("I am organizing the dashboard around current work and usage, with clear navigation and a configurable control deck.".to_owned());
                }
            }
        }
        prepare_paired_demo(&mut snapshot, english);
        Self {
            inner: Arc::new(MockInner {
                language: if english {
                    UiLanguage::English
                } else {
                    UiLanguage::Korean
                },
                state: RwLock::new(snapshot),
                events,
                cancellations: Mutex::new(HashMap::new()),
                turn_cancellations: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Only used by the explicitly labelled loopback demo, never by RealBackend.
    pub fn start_usage_preview(&self) {
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            tick.tick().await;
            let mut announce_pair = true;
            loop {
                tick.tick().await;
                let Some(inner) = weak.upgrade() else { break };
                let backend = Self { inner };
                let updated = {
                    let mut state = backend.inner.state.write().await;
                    state
                        .codex
                        .threads
                        .iter_mut()
                        .find(|thread| thread.id == "external-tui")
                        .map(|thread| {
                            if let Some(usage) = &mut thread.live_usage {
                                usage.updated_at = Utc::now();
                                usage.observed_at = usage.updated_at;
                                if let Some(turn) = &mut usage.turn_tokens {
                                    turn.input_tokens += usage.last_request.input_tokens;
                                    turn.cached_input_tokens +=
                                        usage.last_request.cached_input_tokens;
                                    turn.output_tokens += usage.last_request.output_tokens;
                                    turn.reasoning_output_tokens +=
                                        usage.last_request.reasoning_output_tokens;
                                    turn.total_tokens = turn.input_tokens + turn.output_tokens;
                                }
                                usage.recent_requests.push(usage.last_request.total_tokens);
                                if usage.recent_requests.len() > 16 {
                                    usage.recent_requests.remove(0);
                                }
                            }
                            if let Some(observation) = &mut thread.observation {
                                observation.observed_at = Utc::now();
                            }
                            thread.clone()
                        })
                };
                if let Some(thread) = updated {
                    backend.publish(ServerEvent::CodexThreadUpdated(thread));
                }
                if announce_pair {
                    announce_pair = false;
                    backend.publish(ServerEvent::Notification(NotificationDto {
                        id: Some(Uuid::new_v4()),
                        thread_id: Some("demo-build".to_owned()),
                        turn_id: Some("turn-demo-active".to_owned()),
                        created_at: Some(Utc::now()),
                        level: NotificationLevelDto::Success,
                        title: backend
                            .inner
                            .language
                            .text("Codex 응답 완료", "Codex response ready")
                            .to_owned(),
                        body: backend
                            .inner
                            .language
                            .text(
                                "화면 수정이 끝났습니다 · 모의 데이터",
                                "Interface changes are ready · simulated data",
                            )
                            .to_owned(),
                    }));
                }
            }
        });
    }

    fn publish(&self, event: ServerEvent) {
        let _ = self.inner.events.send(ServerEnvelope::new(event));
    }

    async fn publish_snapshot(&self) {
        self.publish(ServerEvent::Snapshot(self.inner.state.read().await.clone()));
    }

    async fn require_project(&self, project_id: &str) -> Result<(), BackendError> {
        if self
            .inner
            .state
            .read()
            .await
            .projects
            .iter()
            .any(|project| project.id == project_id)
        {
            Ok(())
        } else {
            Err(BackendError::bad_request(
                "project_not_allowed",
                format!("project `{project_id}` is not registered"),
            ))
        }
    }

    async fn start_job(
        &self,
        project_id: String,
        kind: JobKindDto,
        fail: bool,
    ) -> Result<BackendResult, BackendError> {
        self.require_project(&project_id).await?;
        let id = Uuid::new_v4();
        let job = JobDto {
            id,
            kind,
            project_id,
            status: JobStatusDto::Running,
            started_at: Some(Utc::now()),
            finished_at: None,
            exit_code: None,
            duration_ms: None,
            warning_count: 0,
            output_tail: Vec::new(),
        };
        self.inner.state.write().await.jobs.push(job.clone());
        self.publish(ServerEvent::JobStarted(job));

        let cancellation = CancellationToken::new();
        self.inner
            .cancellations
            .lock()
            .await
            .insert(id, cancellation.clone());
        let backend = self.clone();
        tokio::spawn(async move {
            let lines = job_lines(kind, fail);
            for (sequence, line) in lines.iter().enumerate() {
                tokio::select! {
                    () = cancellation.cancelled() => {
                        backend.finish_job(id, JobStatusDto::Cancelled, None).await;
                        return;
                    }
                    () = tokio::time::sleep(Duration::from_millis(360)) => {}
                }
                backend
                    .append_job_output(id, u64::try_from(sequence).unwrap_or(u64::MAX), line)
                    .await;
            }
            let (status, code) = if fail {
                (JobStatusDto::Failed, Some(101))
            } else {
                (JobStatusDto::Succeeded, Some(0))
            };
            backend.finish_job(id, status, code).await;
        });

        Ok(BackendResult {
            message: "Mock Cargo job started".to_owned(),
            job_id: Some(id),
        })
    }

    async fn append_job_output(&self, id: Uuid, sequence: u64, line: &str) {
        if let Some(job) = self
            .inner
            .state
            .write()
            .await
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
        {
            job.output_tail.push(line.to_owned());
        }
        self.publish(ServerEvent::JobOutput(JobOutputDto {
            job_id: id,
            stream: OutputStreamDto::Stdout,
            sequence,
            line: line.to_owned(),
        }));
    }

    async fn finish_job(&self, id: Uuid, status: JobStatusDto, exit_code: Option<i32>) {
        let completed = {
            let mut state = self.inner.state.write().await;
            state.jobs.iter_mut().find(|job| job.id == id).map(|job| {
                job.status = status;
                job.finished_at = Some(Utc::now());
                job.exit_code = exit_code;
                job.duration_ms = Some(1_820);
                job.clone()
            })
        };
        self.inner.cancellations.lock().await.remove(&id);
        if let Some(job) = completed {
            self.publish(ServerEvent::JobCompleted(job.clone()));
            self.publish(ServerEvent::Notification(NotificationDto {
                turn_id: None,
                id: Some(uuid::Uuid::new_v4()),
                thread_id: None,
                created_at: Some(chrono::Utc::now()),
                level: match job.status {
                    JobStatusDto::Succeeded => NotificationLevelDto::Success,
                    JobStatusDto::Cancelled => NotificationLevelDto::Warning,
                    _ => NotificationLevelDto::Error,
                },
                title: "Cargo job finished".to_owned(),
                body: format!("{:?}: {:?}", job.kind, job.status),
            }));
        }
    }

    async fn add_approval(&self) -> ApprovalDto {
        let lang = self.inner.language;
        let approval = ApprovalDto {
            turn_id: Some("simulated-turn".to_owned()),
            id: Uuid::new_v4(),
            thread_id: Some("external-tui".to_owned()),
            kind: ApprovalKindDto::CommandExecution,
            title: lang
                .text("Codex 명령 실행 승인", "CODEX APPROVAL REQUIRED")
                .to_owned(),
            summary: lang
                .text(
                    "선택한 프로젝트에서 `cargo test --workspace`를 실행할까요?",
                    "Run `cargo test --workspace` in the selected project?",
                )
                .to_owned(),
            details: vec![
                lang.text(
                    "명령 실행 · 모의 요청",
                    "Command execution · simulated request",
                )
                .to_owned(),
                lang.text(
                    "데모이므로 실제 명령은 실행하지 않습니다.",
                    "Demo only; no real command is executed.",
                )
                .to_owned(),
            ],
            requested_at: Utc::now(),
        };
        self.inner
            .state
            .write()
            .await
            .codex
            .pending_approvals
            .push(approval.clone());
        self.publish(ServerEvent::CodexApprovalRequested(approval.clone()));
        approval
    }

    async fn start_mock_turn(&self, thread_id: String) -> Result<BackendResult, BackendError> {
        let turn_id = format!("turn-{}", Uuid::new_v4().simple());
        {
            let mut state = self.inner.state.write().await;
            let thread = state
                .codex
                .threads
                .iter_mut()
                .find(|thread| thread.id == thread_id)
                .ok_or_else(|| BackendError::bad_request("thread_not_found", "unknown thread"))?;
            if thread.ownership != ThreadOwnershipDto::OrangeDeck {
                return Err(BackendError::conflict(
                    "external_thread_read_only",
                    "external Codex sessions are observe-only",
                ));
            }
            if matches!(
                thread.status,
                CodexThreadStatusDto::Working | CodexThreadStatusDto::WaitingApproval
            ) {
                return Err(BackendError::conflict(
                    "turn_already_active",
                    "this thread already has an active turn",
                ));
            }
            thread.status = CodexThreadStatusDto::Working;
            thread.active_turn_id = Some(turn_id.clone());
            thread.updated_at = Utc::now().timestamp();
        }
        self.publish(ServerEvent::CodexTurnUpdated(CodexTurnUpdateDto {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            status: CodexThreadStatusDto::Working,
        }));

        let cancellation = CancellationToken::new();
        self.inner
            .turn_cancellations
            .lock()
            .await
            .insert(thread_id.clone(), cancellation.clone());

        let backend = self.clone();
        let spawned_turn_id = turn_id.clone();
        tokio::spawn(async move {
            tokio::select! {
                () = cancellation.cancelled() => return,
                () = tokio::time::sleep(Duration::from_millis(900)) => {}
            }
            backend.publish(ServerEvent::CodexActivity(CodexActivityDto {
                thread_id: Some(thread_id.clone()),
                turn_id: Some(spawned_turn_id.clone()),
                kind: "codex_reply".to_owned(),
                text: "Analyzing the selected Rust workspace".to_owned(),
            }));
            tokio::select! {
                () = cancellation.cancelled() => return,
                () = tokio::time::sleep(Duration::from_millis(1_100)) => {}
            }
            let usage = TokenUsageDto {
                input_tokens: 2_410,
                cached_input_tokens: 1_200,
                output_tokens: 684,
                reasoning_output_tokens: 302,
                total_tokens: 3_094,
                model_context_window: Some(200_000),
            };
            {
                let mut state = backend.inner.state.write().await;
                if let Some(thread) = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|item| item.id == thread_id)
                {
                    thread.status = CodexThreadStatusDto::Completed;
                    thread.active_turn_id = None;
                    thread.token_usage = Some(usage.clone());
                    thread.updated_at = Utc::now().timestamp();
                }
            }
            backend.publish(ServerEvent::CodexUsageUpdated(CodexUsageUpdateDto {
                thread_id: thread_id.clone(),
                usage,
            }));
            backend.publish(ServerEvent::CodexTurnUpdated(CodexTurnUpdateDto {
                thread_id: thread_id.clone(),
                turn_id: spawned_turn_id,
                status: CodexThreadStatusDto::Completed,
            }));
            backend
                .inner
                .turn_cancellations
                .lock()
                .await
                .remove(&thread_id);
            backend.publish(ServerEvent::Notification(NotificationDto {
                turn_id: None,
                id: Some(uuid::Uuid::new_v4()),
                thread_id: None,
                created_at: Some(chrono::Utc::now()),
                level: NotificationLevelDto::Success,
                title: "Codex turn finished".to_owned(),
                body: format!("Mock thread {thread_id}: COMPLETE"),
            }));
        });

        Ok(BackendResult::accepted(format!(
            "Mock turn {turn_id} started"
        )))
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConnectorBackend for MockBackend {
    async fn snapshot(&self) -> SnapshotDto {
        self.inner.state.read().await.clone()
    }

    async fn execute(&self, command: &ClientCommand) -> Result<BackendResult, BackendError> {
        match command {
            ClientCommand::SelectProject { project_id } => {
                self.require_project(project_id).await?;
                self.inner.state.write().await.selected_project_id = Some(project_id.clone());
                self.publish_snapshot().await;
                Ok(BackendResult::accepted("Project selected"))
            }
            ClientCommand::RefreshState
            | ClientCommand::CodexRefreshThreads
            | ClientCommand::CodexReadThread { .. }
            | ClientCommand::CodexWatchThreads { .. } => {
                self.publish_snapshot().await;
                Ok(BackendResult::accepted("Mock state refreshed"))
            }
            ClientCommand::OpenCodexChange {
                thread_id,
                turn_id,
                path,
                ..
            }
            | ClientCommand::RegisterCodexProject {
                thread_id,
                turn_id,
                path,
                ..
            } => {
                let state = self.inner.state.read().await;
                let found = state
                    .codex
                    .threads
                    .iter()
                    .find(|thread| &thread.id == thread_id)
                    .and_then(|thread| thread.observation.as_ref())
                    .filter(|observation| observation.turn_id.as_ref() == Some(turn_id))
                    .and_then(|observation| observation.changes.as_ref())
                    .is_some_and(|changes| {
                        changes.files.iter().any(|file| {
                            &file.path == path
                                && file.kind != orangedeck_protocol::CodeChangeKindDto::Deleted
                        })
                    });
                if !found {
                    return Err(BackendError::bad_request(
                        "file_not_allowed",
                        "Not a recorded mock file change",
                    ));
                }
                Ok(BackendResult::accepted(
                    "Demo: file selected; no Mac editor was opened",
                ))
            }
            ClientCommand::RunCargoCheck { project_id } => {
                self.start_job(project_id.clone(), JobKindDto::CargoCheck, false)
                    .await
            }
            ClientCommand::RunCargoTest { project_id } => {
                self.start_job(project_id.clone(), JobKindDto::CargoTest, false)
                    .await
            }
            ClientCommand::RunCargoClippy { project_id } => {
                self.start_job(project_id.clone(), JobKindDto::CargoClippy, false)
                    .await
            }
            ClientCommand::RunCargoFmt { project_id } => {
                self.start_job(project_id.clone(), JobKindDto::CargoFmtCheck, false)
                    .await
            }
            ClientCommand::RunCargoBuild { project_id } => {
                self.start_job(project_id.clone(), JobKindDto::CargoBuild, false)
                    .await
            }
            ClientCommand::CancelJob { job_id } => {
                let token = self.inner.cancellations.lock().await.get(job_id).cloned();
                token.map_or_else(
                    || {
                        Err(BackendError::conflict(
                            "job_not_running",
                            "job is not running",
                        ))
                    },
                    |token| {
                        token.cancel();
                        Ok(BackendResult::accepted("Cancellation requested"))
                    },
                )
            }
            ClientCommand::RefreshGit { project_id } => {
                self.require_project(project_id).await?;
                let git = self
                    .inner
                    .state
                    .read()
                    .await
                    .git
                    .iter()
                    .find(|git| git.project_id == *project_id)
                    .cloned()
                    .ok_or_else(|| BackendError::bad_request("git_unavailable", "no Git state"))?;
                self.publish(ServerEvent::GitUpdated(git));
                Ok(BackendResult::accepted("Git state refreshed"))
            }
            ClientCommand::OpenEditor { project_id }
            | ClientCommand::OpenTerminal { project_id }
            | ClientCommand::OpenBrowser { project_id }
            | ClientCommand::OpenProject { project_id } => {
                self.require_project(project_id).await?;
                self.publish(ServerEvent::Notification(NotificationDto {
                    turn_id: None,
                    id: Some(uuid::Uuid::new_v4()),
                    thread_id: None,
                    created_at: Some(chrono::Utc::now()),
                    level: NotificationLevelDto::Info,
                    title: "Mock Mac action".to_owned(),
                    body: "The allow-listed host action was accepted".to_owned(),
                }));
                Ok(BackendResult::accepted("Mock host action accepted"))
            }
            ClientCommand::CodexStartThread { project_id } => {
                self.require_project(project_id).await?;
                let thread = CodexThreadDto {
                    activity: None,
                    live_usage: None,
                    observation: None,
                    id: format!("od-{}", Uuid::new_v4().simple()),
                    project_id: Some(project_id.clone()),
                    cwd: "/mock/orange-project".to_owned(),
                    title: "New OrangeDeck thread".to_owned(),
                    preview: "Ready for a prompt from OrangeDeck".to_owned(),
                    status: CodexThreadStatusDto::Idle,
                    ownership: ThreadOwnershipDto::OrangeDeck,
                    updated_at: Utc::now().timestamp(),
                    active_turn_id: None,
                    token_usage: None,
                };
                self.inner
                    .state
                    .write()
                    .await
                    .codex
                    .threads
                    .insert(0, thread.clone());
                self.publish(ServerEvent::CodexThreadUpdated(thread));
                Ok(BackendResult::accepted(
                    "OrangeDeck-owned Codex thread created",
                ))
            }
            ClientCommand::CodexSendPrompt { thread_id, .. } => {
                self.start_mock_turn(thread_id.clone()).await
            }
            ClientCommand::CodexInterrupt { thread_id, turn_id } => {
                let mut state = self.inner.state.write().await;
                let thread = state
                    .codex
                    .threads
                    .iter_mut()
                    .find(|thread| thread.id == *thread_id)
                    .ok_or_else(|| {
                        BackendError::bad_request("thread_not_found", "unknown thread")
                    })?;
                if thread.ownership != ThreadOwnershipDto::OrangeDeck {
                    return Err(BackendError::conflict(
                        "external_thread_read_only",
                        "external Codex sessions are observe-only",
                    ));
                }
                if thread.active_turn_id.as_deref() != Some(turn_id) {
                    return Err(BackendError::conflict(
                        "turn_not_active",
                        "the requested turn is not active on this thread",
                    ));
                }
                thread.status = CodexThreadStatusDto::Idle;
                thread.active_turn_id = None;
                drop(state);
                if let Some(cancellation) =
                    self.inner.turn_cancellations.lock().await.remove(thread_id)
                {
                    cancellation.cancel();
                }
                self.publish(ServerEvent::CodexTurnUpdated(CodexTurnUpdateDto {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    status: CodexThreadStatusDto::Idle,
                }));
                Ok(BackendResult::accepted("Mock turn interrupted"))
            }
            ClientCommand::CodexApprovalResponse {
                approval_id,
                decision,
            } => {
                let removed = {
                    let mut state = self.inner.state.write().await;
                    let before = state.codex.pending_approvals.len();
                    state
                        .codex
                        .pending_approvals
                        .retain(|item| item.id != *approval_id);
                    before != state.codex.pending_approvals.len()
                };
                if !removed {
                    return Err(BackendError::conflict(
                        "approval_not_found",
                        "approval expired",
                    ));
                }
                self.publish(ServerEvent::CodexApprovalResolved {
                    approval_id: *approval_id,
                });
                Ok(BackendResult::accepted(match decision {
                    ApprovalDecisionDto::Approve => "Approved once",
                    ApprovalDecisionDto::Reject => "Rejected",
                }))
            }
            ClientCommand::DemoScenario { scenario } => match scenario {
                orangedeck_protocol::DemoScenario::Approval => {
                    self.add_approval().await;
                    Ok(BackendResult::accepted(self.inner.language.text(
                        "모의 승인 요청을 표시했습니다.",
                        "Approval scenario started",
                    )))
                }
                orangedeck_protocol::DemoScenario::CargoFailure => {
                    self.start_job(DEMO_PROJECT_ID.to_owned(), JobKindDto::CargoTest, true)
                        .await
                }
                orangedeck_protocol::DemoScenario::Reconnect => {
                    {
                        let mut state = self.inner.state.write().await;
                        state.host.state = ConnectionStateDto::Disconnected;
                        state.host.latency_ms = None;
                    }
                    self.publish(ServerEvent::ConnectionChanged(ConnectionChangedDto {
                        state: ConnectionStateDto::Disconnected,
                        latency_ms: None,
                        message: Some("Demo network interruption".to_owned()),
                    }));
                    let backend = self.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(1_800)).await;
                        {
                            let mut state = backend.inner.state.write().await;
                            state.host.state = ConnectionStateDto::Connected;
                            state.host.latency_ms = Some(7);
                        }
                        backend.publish(ServerEvent::ConnectionChanged(ConnectionChangedDto {
                            state: ConnectionStateDto::Connected,
                            latency_ms: Some(7),
                            message: Some("Demo connection restored".to_owned()),
                        }));
                        backend.publish_snapshot().await;
                    });
                    Ok(BackendResult::accepted("Reconnect scenario started"))
                }
            },
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<ServerEnvelope> {
        self.inner.events.subscribe()
    }

    fn mode(&self) -> &'static str {
        "mock"
    }
}

fn job_lines(kind: JobKindDto, fail: bool) -> &'static [&'static str] {
    if fail {
        &[
            "running 82 tests",
            "test parser::safe_command ... ok",
            "test connector::connect ... FAILED",
            "error: test failed",
        ]
    } else {
        match kind {
            JobKindDto::CargoTest => &[
                "running 82 tests",
                "test protocol::round_trip ... ok",
                "test result: ok. 82 passed; 0 failed",
            ],
            JobKindDto::CargoClippy => &[
                "Checking orangedeck-domain",
                "Checking orangedeck-connector",
                "Finished dev profile",
            ],
            JobKindDto::CargoFmtCheck => &[
                "Checking Rust formatting",
                "All workspace files are formatted",
            ],
            JobKindDto::CargoBuild => &["Compiling OrangeDeck", "Finished dev profile"],
            JobKindDto::CargoCheck => &[
                "Checking orangedeck-domain",
                "Checking orangedeck-ui",
                "Finished dev profile",
            ],
        }
    }
}

fn demo_snapshot() -> SnapshotDto {
    let now = Utc::now();
    SnapshotDto {
        notifications: Vec::new(),
        host: HostDto {
            name: "SIMULATED MAC".to_owned(),
            os: "SIMULATED".to_owned(),
            architecture: "MOCK".to_owned(),
            address: Some("127.0.0.1".to_owned()),
            state: ConnectionStateDto::Connected,
            tailscale: false,
            latency_ms: Some(7),
            last_seen: now,
        },
        projects: vec![ProjectDto {
            id: DEMO_PROJECT_ID.to_owned(),
            name: "Demo Project".to_owned(),
            path: "/mock/orange-project".to_owned(),
            has_browser_url: true,
        }],
        selected_project_id: Some(DEMO_PROJECT_ID.to_owned()),
        codex: CodexSnapshotDto {
            account_usage: Some(orangedeck_protocol::AccountUsageDto {
                lifetime_tokens: Some(12_345_678), peak_daily_tokens: Some(2_345_678),
                daily: vec![orangedeck_protocol::DailyTokenUsageDto { date: now.format("%Y-%m-%d").to_string(), tokens: 456_789 }],
                updated_at: Some(now), unavailable_reason: None,
            }),
            connection: CodexConnectionDto {
                state: CodexConnectionStateDto::Connected,
                version: Some("0.153.2".to_owned()),
                compatible: true,
                message: None,
            },
            threads: vec![
                CodexThreadDto {
                    activity: None,
                    id: "demo-build".to_owned(),
                    live_usage: None,
                    observation: None,
                    project_id: Some(DEMO_PROJECT_ID.to_owned()),
                    cwd: "/mock/orange-project".to_owned(),
                    title: "Implement typed Tailscale transport".to_owned(),
                    preview: "Running workspace integration checks".to_owned(),
                    status: CodexThreadStatusDto::Working,
                    ownership: ThreadOwnershipDto::OrangeDeck,
                    updated_at: now.timestamp(),
                    active_turn_id: Some("turn-demo-active".to_owned()),
                    token_usage: Some(TokenUsageDto {
                        input_tokens: 31_420,
                        cached_input_tokens: 24_130,
                        output_tokens: 5_920,
                        reasoning_output_tokens: 2_200,
                        total_tokens: 37_340,
                        model_context_window: Some(200_000),
                    }),
                },
                CodexThreadDto {
                    activity: None,
                    id: "demo-review".to_owned(),
                    live_usage: None,
                    observation: None,
                    project_id: Some(DEMO_PROJECT_ID.to_owned()),
                    cwd: "/mock/orange-project".to_owned(),
                    title: "Review Cargo job cancellation".to_owned(),
                    preview: "Waiting for one explicit command approval".to_owned(),
                    status: CodexThreadStatusDto::WaitingApproval,
                    ownership: ThreadOwnershipDto::OrangeDeck,
                    updated_at: now.timestamp() - 32,
                    active_turn_id: Some("turn-demo-approval".to_owned()),
                    token_usage: None,
                },
                CodexThreadDto {
                    activity: None,
                    id: "external-tui".to_owned(),
                    live_usage: Some(demo_live_usage(now)),
                    observation: Some(orangedeck_protocol::ThreadObservationDto {
                        turn_id: Some("simulated-turn".to_owned()),
                        latest_user_prompt: Some("지금 작업 중인 Codex의 토큰 사용량과 프로젝트를 한눈에 볼 수 있게, LIVE 화면을 멋있게 바꿔줘.".to_owned()),
                        latest_codex_reply: Some("작업 현황과 사용량을 중심으로 화면을 구성하고 있습니다.".to_owned()),
                        changes: Some(orangedeck_protocol::TurnChangesDto::default()),
                        last_turn_status: CodexThreadStatusDto::Working,
                        model: Some("gpt-5.5".to_owned()), observed_at: now,
                    }),
                    project_id: Some(DEMO_PROJECT_ID.to_owned()),
                    cwd: "/mock/orange-project".to_owned(),
                    title: "External terminal Codex session".to_owned(),
                    preview: "Observe only; live state may be unknown".to_owned(),
                    status: CodexThreadStatusDto::Unknown,
                    ownership: ThreadOwnershipDto::ExternalReadOnly,
                    updated_at: now.timestamp() - 180,
                    active_turn_id: None,
                    token_usage: None,
                },
                CodexThreadDto {
                    id: "demo-website".to_owned(), cwd: "/mock/website".to_owned(), title: "Website project".to_owned(), preview: String::new(), project_id: None,
                    status: CodexThreadStatusDto::Completed, ownership: ThreadOwnershipDto::ExternalReadOnly,
                    updated_at: now.timestamp() - 3600, active_turn_id: None, token_usage: None, live_usage: None, observation: None, activity: None,
                },
            ],
            pending_approvals: Vec::new(),
            limits: Some(CodexLimitsDto {
                limit_id: Some("codex".to_owned()),
                limit_name: None,
                additional: vec![orangedeck_protocol::LimitBucketDto {
                    id: "codex_bengalfox".to_owned(), name: Some("GPT-5.3-Codex-Spark".to_owned()),
                    primary: Some(RateLimitWindowDto {
                        used_percent: 28, remaining_percent: 72,
                        resets_at: Some(now.timestamp() + 4200), window_duration_minutes: Some(300),
                    }),
                    secondary: Some(RateLimitWindowDto {
                        used_percent: 7, remaining_percent: 93,
                        resets_at: Some(now.timestamp() + 190_000), window_duration_minutes: Some(10_080),
                    }),
                }],
                plan_type: Some("plus".to_owned()),
                primary: Some(RateLimitWindowDto {
                    used_percent: 41,
                    remaining_percent: 59,
                    resets_at: Some(now.timestamp() + 190_000),
                    window_duration_minutes: Some(10_080),
                }),
                secondary: None,
                credits_balance: None,
                updated_at: Some(now),
            }),
            supported_features: vec![
                "approvals".to_owned(),
                "owned_thread_control".to_owned(),
                "rate_limits".to_owned(),
                "token_usage".to_owned(),
                "read_only_monitor".to_owned(),
                "paired_conversations".to_owned(),
                "turn_file_changes".to_owned(),
                "conversation_editor_root".to_owned(),
                "session_log_usage".to_owned(),
            ],
        },
        jobs: vec![
            completed_job(JobKindDto::CargoCheck, 2_420, now),
            completed_job(JobKindDto::CargoTest, 8_730, now),
            completed_job(JobKindDto::CargoClippy, 3_110, now),
        ],
        git: vec![GitDto {
            project_id: DEMO_PROJECT_ID.to_owned(),
            branch: "feature/orangedeck".to_owned(),
            clean: false,
            counts: GitCountsDto {
                modified: 4,
                staged: 1,
                untracked: 2,
                conflicted: 0,
            },
            ahead: 3,
            behind: 0,
            files: Vec::new(),
            recent_commits: vec![CommitDto {
                hash: "a42fd1c".to_owned(),
                author: "Orange".to_owned(),
                timestamp: now.timestamp() - 1_200,
                subject: "Wire Codex app-server events".to_owned(),
            }],
            diff: DiffSummaryDto {
                files_changed: 7,
                insertions: 384,
                deletions: 51,
                summary: "7 files changed, 384 insertions(+), 51 deletions(-)".to_owned(),
            },
            updated_at: now,
            error: None,
        }],
        system: SystemDto {
            cpu_percent: Some(18.4),
            memory_used_bytes: Some(9_420_000_000),
            memory_total_bytes: Some(18_000_000_000),
            load_average: Some(2.1),
            uptime_seconds: Some(184_200),
        },
        activity: vec![
            "Codex is analyzing the selected workspace".to_owned(),
            "Cargo test completed in 8.7s".to_owned(),
            "Git state refreshed".to_owned(),
        ],
    }
}

fn prepare_paired_demo(snapshot: &mut SnapshotDto, english: bool) {
    use orangedeck_protocol::{
        CodeChangeDto, CodeChangeKindDto, ThreadObservationDto, TurnChangesDto,
    };
    let lang = if english {
        UiLanguage::English
    } else {
        UiLanguage::Korean
    };
    let mut notes = snapshot.codex.threads[3].clone();
    "demo-notes".clone_into(&mut notes.id);
    "/mock/orange-project".clone_into(&mut notes.cwd);
    notes.project_id = Some(DEMO_PROJECT_ID.to_owned());
    snapshot.codex.threads.push(notes);
    for (index, thread) in snapshot.codex.threads.iter_mut().enumerate() {
        lang.text(
            [
                "화면 다듬기",
                "검사와 승인",
                "진행 중인 작업",
                "웹사이트 검토",
                "설명과 아이디어",
            ][index],
            [
                "Interface polish",
                "Checks and approval",
                "Work in progress",
                "Website review",
                "Notes and ideas",
            ][index],
        )
        .clone_into(&mut thread.title);
        let turn_id = thread
            .active_turn_id
            .clone()
            .or_else(|| {
                thread
                    .observation
                    .as_ref()
                    .and_then(|observation| observation.turn_id.clone())
            })
            .unwrap_or_else(|| format!("demo-turn-{index}"));
        if index == 0 {
            thread.status = CodexThreadStatusDto::Completed;
        }
        let files = if index < 3 {
            vec![
            CodeChangeDto { path:"src/interface.rs".to_owned(), previous_path:None, kind:CodeChangeKindDto::Modified, first_line:24,
                diff:"@@ -24,2 +24,3 @@\n-let columns = 2;\n+let columns = 5;\n+let paired_rows = 2;".to_owned(), truncated:false },
            CodeChangeDto { path:"src/notifications.rs".to_owned(), previous_path:None, kind:CodeChangeKindDto::Added, first_line:1,
                diff:"@@ -0,0 +1,3 @@\n+fn on_response(column: usize) {\n+    flash_pair(column);\n+}".to_owned(), truncated:false },
        ]
        } else {
            Vec::new()
        };
        thread.observation = Some(ThreadObservationDto {
            turn_id:Some(turn_id), latest_user_prompt:Some(lang.text(if index < 3 { "응답과 수정 파일을 나란히 확인할 수 있게 화면을 바꿔줘." } else { "코드는 바꾸지 말고 사용하기 쉬운 기능을 설명해줘." },
                if index < 3 { "Let me review each response alongside its changed files." } else { "Explain useful features without changing any code." }).to_owned()),
            latest_codex_reply:Some(lang.text(if index < 3 { "위아래 버튼을 같은 대화에 연결했습니다. 새 응답이 오면 두 버튼이 함께 빛납니다. 아래 버튼에서 수정 파일 두 개를 확인할 수 있습니다." } else { "응답은 위 버튼, 수정 파일은 바로 아래 버튼에서 확인하면 됩니다. 이번 질의에서는 설명만 했으며 파일을 수정하지 않았습니다." },
                if index < 3 { "Both keys now follow the same conversation. They light together when a response arrives. The lower key opens the two changed files." } else { "Use the upper key for the response and the lower key for changed files. This question only needed an explanation, so no files were changed." }).to_owned()),
            changes:Some(TurnChangesDto { files, truncated:false }), last_turn_status:if index == 2 { CodexThreadStatusDto::Working } else { thread.status },
            model:Some("demo".to_owned()), observed_at:Utc::now(),
        });
    }
}

fn completed_job(kind: JobKindDto, duration_ms: u64, now: chrono::DateTime<Utc>) -> JobDto {
    JobDto {
        id: Uuid::new_v4(),
        kind,
        project_id: DEMO_PROJECT_ID.to_owned(),
        status: JobStatusDto::Succeeded,
        started_at: Some(
            now - chrono::Duration::milliseconds(i64::try_from(duration_ms).unwrap_or(i64::MAX)),
        ),
        finished_at: Some(now),
        exit_code: Some(0),
        duration_ms: Some(duration_ms),
        warning_count: 0,
        output_tail: vec!["Finished successfully".to_owned()],
    }
}

fn demo_live_usage(now: chrono::DateTime<Utc>) -> orangedeck_protocol::LiveTokenUsageDto {
    orangedeck_protocol::LiveTokenUsageDto {
        turn_id: Some("simulated-turn".to_owned()),
        turn_tokens: Some(TokenUsageDto {
            input_tokens: 183_240,
            cached_input_tokens: 142_600,
            output_tokens: 9_120,
            reasoning_output_tokens: 4_100,
            total_tokens: 192_360,
            model_context_window: Some(258_400),
        }),
        last_request: TokenUsageDto {
            input_tokens: 35_000,
            cached_input_tokens: 26_000,
            output_tokens: 1_800,
            reasoning_output_tokens: 850,
            total_tokens: 36_800,
            model_context_window: Some(258_400),
        },
        recent_requests: vec![
            8_700, 14_300, 11_200, 19_000, 23_800, 17_900, 27_200, 31_300, 36_800,
        ],
        status: CodexThreadStatusDto::Working,
        updated_at: now,
        observed_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_rejects_unregistered_projects() {
        let mock = MockBackend::new();
        let result = mock
            .execute(&ClientCommand::RunCargoCheck {
                project_id: "outside".to_owned(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn external_threads_are_read_only() {
        let mock = MockBackend::new();
        let result = mock
            .execute(&ClientCommand::CodexSendPrompt {
                thread_id: "external-tui".to_owned(),
                prompt: "do not send this".to_owned(),
            })
            .await;
        assert!(result.is_err());
    }
}
