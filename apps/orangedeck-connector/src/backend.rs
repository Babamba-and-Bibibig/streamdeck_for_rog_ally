use std::{
    collections::{BTreeSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use orangedeck_application::{
    SharedDashboard, ValidatedCommand, account_usage_to_dto, approval_to_dto,
    codex_connection_to_dto, codex_status_to_dto, codex_thread_to_dto, git_to_dto, job_to_dto,
    limits_to_dto, output_stream_to_dto, system_to_dto, token_usage_to_dto, validate_command,
};
use orangedeck_domain::{
    CodexConnectionState, CodexEvent, DashboardState, DiffSummary, GitCounts, GitSnapshot,
    JobEvent, Project, ProjectId, ProjectRegistry,
};
use orangedeck_infra::{
    CargoJobRunner, CodexClient, ConnectorConfig, GitInspector, JobError, collect_system_snapshot,
    open_browser, open_editor, open_project, open_terminal, probe_codex, tailscale_ip,
};
use orangedeck_protocol::{
    CodexActivityDto, CodexTurnUpdateDto, CodexUsageUpdateDto, JobOutputDto, NotificationDto,
    NotificationLevelDto, ServerEnvelope, ServerEvent, SnapshotDto,
};
use tokio::sync::{Mutex, RwLock, broadcast};
use tracing::{info, warn};

use crate::server::{BackendError, BackendResult, ConnectorBackend};

#[derive(Clone)]
pub struct RealBackend {
    inner: Arc<RealBackendInner>,
}

struct RealBackendInner {
    config: ConnectorConfig,
    registry: ProjectRegistry,
    state: SharedDashboard,
    events: broadcast::Sender<ServerEnvelope>,
    jobs: CargoJobRunner,
    git: GitInspector,
    codex: RwLock<Option<CodexClient>>,
    codex_connect_lock: Mutex<()>,
    owned_threads_path: PathBuf,
    watched_thread: RwLock<Option<String>>,
    watched_deck: RwLock<Vec<String>>,
    thread_refresh_lock: Mutex<()>,
    hooks: RwLock<Option<crate::hooks::HookHub>>,
    notifications: std::sync::Mutex<VecDeque<NotificationDto>>,
    completed_turns: Mutex<VecDeque<(String, String)>>,
}

impl RealBackend {
    pub async fn new(
        config: ConnectorConfig,
        owned_threads_path: PathBuf,
    ) -> Result<Self, BackendError> {
        let registry = config
            .project_registry()
            .map_err(|error| BackendError::bad_request("invalid_config", error.to_string()))?;
        let projects = registry.projects().cloned().collect::<Vec<_>>();
        let mut dashboard = DashboardState::new(config.host_name.clone(), projects);
        dashboard.system = collect_system_snapshot().await;
        if let Ok(ip) = tailscale_ip().await {
            dashboard.host.address = Some(ip.to_string());
            dashboard.host.tailscale = true;
        }
        let (events, _) = broadcast::channel(512);
        let cargo_binary = config.cargo_binary.clone();
        let backend = Self {
            inner: Arc::new(RealBackendInner {
                config,
                registry,
                state: SharedDashboard::new(dashboard),
                events,
                jobs: CargoJobRunner::new(cargo_binary),
                git: GitInspector,
                codex: RwLock::new(None),
                codex_connect_lock: Mutex::new(()),
                owned_threads_path,
                watched_thread: RwLock::new(None),
                watched_deck: RwLock::new(Vec::new()),
                thread_refresh_lock: Mutex::new(()),
                hooks: RwLock::new(None),
                notifications: std::sync::Mutex::new(VecDeque::new()),
                completed_turns: Mutex::new(VecDeque::new()),
            }),
        };
        backend.start_job_forwarder();
        if let Err(error) = backend.refresh_codex().await {
            warn!(%error, "Codex is unavailable; the rest of OrangeDeck remains active");
        }
        backend.start_background_refresh();
        Ok(backend)
    }

    pub async fn enable_hooks(&self, socket: PathBuf) -> std::io::Result<()> {
        let hub = crate::hooks::HookHub::bind(socket).await?;
        let mut events = hub.subscribe();
        *self.inner.hooks.write().await = Some(hub.clone());
        self.inner
            .state
            .update(|state| {
                state
                    .codex
                    .supported_features
                    .insert("codex_hooks".to_owned());
            })
            .await;
        let backend = self.clone();
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                backend
                    .inner
                    .state
                    .update(|state| {
                        state
                            .codex
                            .supported_features
                            .insert("codex_hooks_active".to_owned());
                    })
                    .await;
                match event {
                    crate::hooks::HookEvent::Lifecycle {
                        thread_id,
                        turn_id,
                        cwd,
                        status,
                    } => {
                        backend.ensure_hook_thread(&thread_id, &cwd).await;
                        let event = if status == orangedeck_domain::CodexThreadStatus::Working {
                            CodexEvent::TurnStarted { thread_id, turn_id }
                        } else {
                            CodexEvent::TurnCompleted {
                                thread_id,
                                turn_id,
                                status,
                            }
                        };
                        backend.forward_codex_event(event).await;
                    }
                    crate::hooks::HookEvent::Approval(request) => {
                        if backend.inner.events.receiver_count() == 0 {
                            hub.resolve(request.id, None).await;
                        } else {
                            backend
                                .forward_codex_event(CodexEvent::ApprovalRequested(request))
                                .await;
                        }
                    }
                    crate::hooks::HookEvent::Resolved(id) => {
                        backend
                            .forward_codex_event(CodexEvent::ApprovalResolved(id))
                            .await;
                    }
                    crate::hooks::HookEvent::FilesChanged {
                        thread_id,
                        turn_id,
                        cwd,
                        starting,
                    } => {
                        backend.ensure_hook_thread(&thread_id, &cwd).await;
                        let new_turn = starting
                            && backend
                                .inner
                                .state
                                .read(|state| {
                                    state
                                        .codex
                                        .threads
                                        .iter()
                                        .find(|thread| thread.id == thread_id)
                                        .is_some_and(|thread| {
                                            thread.active_turn_id.as_deref().or_else(|| {
                                                thread
                                                    .activity
                                                    .as_ref()
                                                    .map(|value| value.turn_id.as_str())
                                            }) != Some(&turn_id)
                                        })
                                })
                                .await;
                        if new_turn {
                            backend
                                .forward_codex_event(CodexEvent::TurnStarted {
                                    thread_id: thread_id.clone(),
                                    turn_id,
                                })
                                .await;
                        }
                        let updated = backend
                            .inner
                            .state
                            .update(|state| {
                                let thread = state
                                    .codex
                                    .threads
                                    .iter_mut()
                                    .find(|thread| thread.id == thread_id)?;
                                hub.enrich_files(thread);
                                Some(codex_thread_to_dto(thread))
                            })
                            .await;
                        if let Some(thread) = updated {
                            backend.publish(ServerEvent::CodexThreadUpdated(thread));
                        }
                    }
                }
            }
        });
        Ok(())
    }

    async fn ensure_hook_thread(&self, id: &str, cwd: &str) {
        let thread = self
            .inner
            .state
            .update(|state| {
                if state.codex.threads.iter().any(|thread| thread.id == id) {
                    return None;
                }
                let thread = orangedeck_domain::CodexThread {
                    id: id.to_owned(),
                    project_id: None,
                    cwd: cwd.to_owned(),
                    title: cwd
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(cwd)
                        .to_owned(),
                    preview: String::new(),
                    status: orangedeck_domain::CodexThreadStatus::Unknown,
                    ownership: orangedeck_domain::ThreadOwnership::ExternalReadOnly,
                    updated_at: Utc::now().timestamp(),
                    active_turn_id: None,
                    token_usage: None,
                    observation: None,
                    live_usage: None,
                    activity: None,
                };
                state.codex.threads.insert(0, thread.clone());
                Some(thread)
            })
            .await;
        if let Some(thread) = thread {
            self.publish(ServerEvent::CodexThreadUpdated(codex_thread_to_dto(
                &thread,
            )));
        }
    }

    fn start_job_forwarder(&self) {
        let backend = self.clone();
        let mut events = self.inner.jobs.subscribe();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => backend.forward_job_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(skipped, "cargo event forwarder lagged");
                        backend.publish_snapshot().await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    fn start_codex_forwarder(&self, client: &CodexClient) {
        let backend = self.clone();
        let mut events = client.subscribe();
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(event) => backend.forward_codex_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(skipped, "Codex event forwarder lagged");
                        backend.publish_snapshot().await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    fn start_background_refresh(&self) {
        let backend = self.clone();
        tokio::spawn(async move {
            let mut system_tick = tokio::time::interval(Duration::from_secs(10));
            system_tick.tick().await;
            loop {
                system_tick.tick().await;
                backend.refresh_system().await;
            }
        });
        for seconds in [5, 15, 60] {
            let backend = self.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(seconds));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if seconds == 5 {
                        let _ = backend.refresh_monitored_threads().await;
                    } else if seconds == 15
                        && let Ok(client) = backend.connect_codex().await
                    {
                        let _ = client.refresh_limits().await;
                    } else if seconds == 60
                        && let Ok(client) = backend.connect_codex().await
                        && client.refresh_account_usage().await.is_err()
                    {
                        backend
                            .forward_codex_event(CodexEvent::AccountUsageUpdated(
                                orangedeck_domain::AccountUsage {
                                    unavailable_reason: Some(
                                        "공식 계정 토큰 통계를 현재 조회할 수 없습니다".to_owned(),
                                    ),
                                    ..Default::default()
                                },
                            ))
                            .await;
                    }
                }
            });
        }
    }

    async fn forward_job_event(&self, event: JobEvent) {
        self.inner.state.apply_job_event(&event).await;
        let notification = match &event {
            JobEvent::Completed(job) => Some(NotificationDto {
                turn_id: None,
                id: Some(uuid::Uuid::new_v4()),
                thread_id: None,
                created_at: Some(chrono::Utc::now()),
                level: match job.status {
                    orangedeck_domain::JobStatus::Succeeded => NotificationLevelDto::Success,
                    orangedeck_domain::JobStatus::Cancelled => NotificationLevelDto::Warning,
                    _ => NotificationLevelDto::Error,
                },
                title: format!("{} finished", job.kind.label()),
                body: format!(
                    "{} after {:.1}s",
                    match job.status {
                        orangedeck_domain::JobStatus::Succeeded => "PASS",
                        orangedeck_domain::JobStatus::Failed => "FAILED",
                        orangedeck_domain::JobStatus::Cancelled => "CANCELLED",
                        orangedeck_domain::JobStatus::Queued
                        | orangedeck_domain::JobStatus::Running => "STOPPED",
                    },
                    Duration::from_millis(job.duration_ms.unwrap_or_default()).as_secs_f32()
                ),
            }),
            JobEvent::Started(_) | JobEvent::Output { .. } => None,
        };
        let protocol_event = match &event {
            JobEvent::Started(job) => ServerEvent::JobStarted(job_to_dto(job)),
            JobEvent::Output {
                job_id,
                stream,
                sequence,
                line,
            } => ServerEvent::JobOutput(JobOutputDto {
                job_id: *job_id,
                stream: output_stream_to_dto(*stream),
                sequence: *sequence,
                line: line.clone(),
            }),
            JobEvent::Completed(job) => {
                let completed = self
                    .inner
                    .state
                    .read(|state| state.jobs.get(&job.id).cloned())
                    .await
                    .unwrap_or_else(|| job.clone());
                ServerEvent::JobCompleted(job_to_dto(&completed))
            }
        };
        self.publish(protocol_event);
        if let Some(notification) = notification {
            self.publish(ServerEvent::Notification(notification));
        }
    }

    async fn forward_codex_event(&self, event: CodexEvent) {
        if let CodexEvent::TurnCompleted {
            thread_id, turn_id, ..
        } = &event
        {
            let key = (thread_id.clone(), turn_id.clone());
            let mut completed = self.inner.completed_turns.lock().await;
            if completed.contains(&key) {
                return;
            }
            completed.push_back(key);
            if completed.len() > 512 {
                completed.pop_front();
            }
        }
        self.inner.state.apply_codex_event(&event).await;
        if matches!(
            event,
            CodexEvent::ThreadsReplaced(_)
                | CodexEvent::ThreadUpdated(_)
                | CodexEvent::TurnStarted { .. }
                | CodexEvent::TurnCompleted { .. }
        ) && let Some(hub) = self.inner.hooks.read().await.as_ref()
        {
            self.inner
                .state
                .update(|state| {
                    for thread in &mut state.codex.threads {
                        hub.enrich_files(thread);
                    }
                })
                .await;
        }
        let notification = if let CodexEvent::TurnCompleted {
            thread_id,
            turn_id,
            status,
        } = &event
        {
            let body = self
                .inner
                .state
                .read(|state| {
                    state
                        .codex
                        .threads
                        .iter()
                        .find(|thread| &thread.id == thread_id)
                        .map_or_else(
                            || format!("대화 {thread_id}"),
                            |thread| format!("{} · {}", thread.title, thread.cwd),
                        )
                })
                .await;
            let (level, title) = match status {
                orangedeck_domain::CodexThreadStatus::Completed => {
                    (NotificationLevelDto::Success, "Codex 응답 완료")
                }
                orangedeck_domain::CodexThreadStatus::Error => {
                    (NotificationLevelDto::Error, "Codex 작업 오류")
                }
                _ => (NotificationLevelDto::Warning, "Codex 작업 중단"),
            };
            Some(NotificationDto {
                turn_id: Some(turn_id.clone()),
                id: Some(uuid::Uuid::new_v4()),
                thread_id: Some(thread_id.clone()),
                created_at: Some(Utc::now()),
                level,
                title: title.to_owned(),
                body,
            })
        } else if let CodexEvent::Activity {
            thread_id,
            turn_id,
            kind,
            text,
        } = &event
            && kind == "user_input"
        {
            Some(NotificationDto {
                id: Some(uuid::Uuid::new_v4()),
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                created_at: Some(Utc::now()),
                level: NotificationLevelDto::Warning,
                title: "Codex 판단 요청".to_owned(),
                body: text.clone(),
            })
        } else {
            None
        };
        let protocol_event = match &event {
            CodexEvent::ConnectionChanged { .. } => {
                let connection = self
                    .inner
                    .state
                    .read(|state| codex_connection_to_dto(&state.codex))
                    .await;
                ServerEvent::CodexConnectionChanged(connection)
            }
            CodexEvent::ThreadsReplaced(_) => ServerEvent::CodexThreadsReplaced {
                threads: self
                    .inner
                    .state
                    .read(|state| {
                        state
                            .codex
                            .threads
                            .iter()
                            .map(codex_thread_to_dto)
                            .collect()
                    })
                    .await,
            },
            CodexEvent::ThreadUpdated(thread) => ServerEvent::CodexThreadUpdated(
                self.inner
                    .state
                    .read(|state| {
                        codex_thread_to_dto(
                            state
                                .codex
                                .threads
                                .iter()
                                .find(|item| item.id == thread.id)
                                .unwrap_or(thread),
                        )
                    })
                    .await,
            ),
            CodexEvent::TurnStarted { thread_id, turn_id } => {
                ServerEvent::CodexTurnUpdated(CodexTurnUpdateDto {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    status: orangedeck_protocol::CodexThreadStatusDto::Working,
                })
            }
            CodexEvent::TurnCompleted {
                thread_id,
                turn_id,
                status,
            } => ServerEvent::CodexTurnUpdated(CodexTurnUpdateDto {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                status: codex_status_to_dto(*status),
            }),
            CodexEvent::ApprovalRequested(approval) => {
                ServerEvent::CodexApprovalRequested(approval_to_dto(approval))
            }
            CodexEvent::ApprovalResolved(approval_id) => ServerEvent::CodexApprovalResolved {
                approval_id: *approval_id,
            },
            CodexEvent::TokenUsageUpdated { thread_id, usage } => {
                ServerEvent::CodexUsageUpdated(CodexUsageUpdateDto {
                    thread_id: thread_id.clone(),
                    usage: token_usage_to_dto(usage),
                })
            }
            CodexEvent::LimitsUpdated(limits) => {
                ServerEvent::CodexLimitsUpdated(limits_to_dto(limits))
            }
            CodexEvent::AccountUsageUpdated(usage) => ServerEvent::CodexAccountUsageUpdated(
                self.inner
                    .state
                    .read(|state| {
                        account_usage_to_dto(state.codex.account_usage.as_ref().unwrap_or(usage))
                    })
                    .await,
            ),
            CodexEvent::Activity {
                thread_id,
                turn_id,
                kind,
                text,
            } => ServerEvent::CodexActivity(CodexActivityDto {
                thread_id: thread_id.clone(),
                turn_id: turn_id.clone(),
                kind: kind.clone(),
                text: text.clone(),
            }),
            CodexEvent::Error(message) => ServerEvent::CodexError {
                message: message.clone(),
            },
        };
        self.publish(protocol_event);
        if let Some(notification) = notification {
            self.publish(ServerEvent::Notification(notification));
        }
    }

    async fn connect_codex(&self) -> Result<CodexClient, BackendError> {
        if let Some(client) = self.inner.codex.read().await.as_ref()
            && client.is_connected()
        {
            return Ok(client.clone());
        }
        let _guard = self.inner.codex_connect_lock.lock().await;
        if let Some(client) = self.inner.codex.read().await.as_ref()
            && client.is_connected()
        {
            return Ok(client.clone());
        }

        self.inner
            .state
            .update(|state| state.codex.connection = CodexConnectionState::Connecting)
            .await;
        let executable = self.inner.config.codex_binary.clone();
        let probe = probe_codex(&executable).await;
        self.inner
            .state
            .update(|state| {
                state.codex.version = probe.version.clone().map(|version| {
                    version
                        .split_whitespace()
                        .last()
                        .unwrap_or(&version)
                        .to_owned()
                });
            })
            .await;
        let projects = self.inner.registry.projects().cloned().collect();
        match CodexClient::spawn(&executable, self.inner.owned_threads_path.clone(), projects).await
        {
            Ok(client) => {
                let version = client.version().to_owned();
                let compatible = client.compatible();
                self.inner
                    .state
                    .update(|state| {
                        state.codex.connection = CodexConnectionState::Connected;
                        state.codex.version = Some(version);
                        state.codex.compatible = compatible;
                        state.codex.message = (!compatible).then(|| {
                            format!(
                                "Version differs from tested Codex {}",
                                orangedeck_infra::TESTED_CODEX_VERSION
                            )
                        });
                        state.codex.supported_features.extend(BTreeSet::from([
                            "approvals".to_owned(),
                            "owned_thread_control".to_owned(),
                            "rate_limits".to_owned(),
                            "thread_list".to_owned(),
                            "token_usage".to_owned(),
                            "turn_interrupt".to_owned(),
                            "read_only_monitor".to_owned(),
                            "session_log_usage".to_owned(),
                            "completion_notifications".to_owned(),
                            "paired_conversations".to_owned(),
                            "turn_file_changes".to_owned(),
                            "conversation_editor_root".to_owned(),
                        ]));
                    })
                    .await;
                self.start_codex_forwarder(&client);
                *self.inner.codex.write().await = Some(client.clone());
                self.publish(ServerEvent::CodexConnectionChanged(
                    self.inner
                        .state
                        .read(|state| codex_connection_to_dto(&state.codex))
                        .await,
                ));
                Ok(client)
            }
            Err(error) => {
                let unsupported = !probe.app_server_supported;
                self.inner
                    .state
                    .update(|state| {
                        state.codex.connection = if unsupported {
                            CodexConnectionState::Unsupported
                        } else {
                            CodexConnectionState::Error
                        };
                        state.codex.message = Some(error.to_string());
                    })
                    .await;
                self.publish(ServerEvent::CodexConnectionChanged(
                    self.inner
                        .state
                        .read(|state| codex_connection_to_dto(&state.codex))
                        .await,
                ));
                Err(BackendError::internal(
                    "codex_unavailable",
                    error.to_string(),
                ))
            }
        }
    }

    async fn refresh_codex(&self) -> Result<(), BackendError> {
        self.refresh_monitored_threads().await?;
        let client = self.connect_codex().await?;
        if let Err(error) = client.refresh_limits().await {
            warn!(%error, "Codex rate-limit data is unavailable");
        }
        if client.refresh_account_usage().await.is_err() {
            self.forward_codex_event(CodexEvent::AccountUsageUpdated(
                orangedeck_domain::AccountUsage {
                    unavailable_reason: Some("공식 계정 토큰 통계 미제공".to_owned()),
                    ..Default::default()
                },
            ))
            .await;
        }
        Ok(())
    }

    async fn refresh_monitored_threads(&self) -> Result<(), BackendError> {
        let _guard = self.inner.thread_refresh_lock.lock().await;
        let client = self.connect_codex().await?;
        let threads = client.refresh_threads().await.map_err(codex_error)?;
        let recent = threads
            .iter()
            .filter(|thread| {
                thread.ownership == orangedeck_domain::ThreadOwnership::ExternalReadOnly
            })
            .max_by_key(|thread| thread.updated_at)
            .or_else(|| threads.first());
        let mut ids = recent
            .map(|thread| vec![thread.id.clone()])
            .unwrap_or_default();
        if let Some(id) = self.inner.watched_thread.read().await.as_ref()
            && !ids.contains(id)
            && threads.iter().any(|thread| &thread.id == id)
        {
            ids.push(id.clone());
        }
        for id in self.inner.watched_deck.read().await.iter() {
            if !ids.contains(id) && threads.iter().any(|thread| &thread.id == id) {
                ids.push(id.clone());
            }
        }
        for id in ids {
            if let Err(error) = client.read_thread(&id).await {
                warn!(%error, "Read-only thread observation unavailable");
            }
        }
        Ok(())
    }

    async fn refresh_git(&self, project_id: &ProjectId) -> Result<(), BackendError> {
        let project = self.project(project_id)?;
        let snapshot = match self.inner.git.inspect(&project).await {
            Ok(snapshot) => snapshot,
            Err(error) => GitSnapshot {
                project_id: project.id,
                branch: "UNAVAILABLE".to_owned(),
                clean: false,
                counts: GitCounts::default(),
                ahead: 0,
                behind: 0,
                files: Vec::new(),
                recent_commits: Vec::new(),
                diff: DiffSummary::default(),
                updated_at: Utc::now(),
                error: Some(error.to_string()),
            },
        };
        self.inner
            .state
            .update(|state| {
                state
                    .git
                    .insert(snapshot.project_id.clone(), snapshot.clone());
            })
            .await;
        self.publish(ServerEvent::GitUpdated(git_to_dto(&snapshot)));
        Ok(())
    }

    async fn refresh_system(&self) {
        let system = collect_system_snapshot().await;
        self.inner
            .state
            .update(|state| {
                state.system = system.clone();
                state.host.last_seen = Utc::now();
            })
            .await;
        self.publish(ServerEvent::SystemUpdated(system_to_dto(&system)));
    }

    async fn publish_snapshot(&self) {
        self.publish(ServerEvent::Snapshot(self.snapshot().await));
    }

    fn publish(&self, event: ServerEvent) {
        if let ServerEvent::Notification(notification) = &event
            && let Ok(mut history) = self.inner.notifications.lock()
        {
            history.push_back(notification.clone());
            if history.len() > 100 {
                history.pop_front();
            }
        }
        let _ = self.inner.events.send(ServerEnvelope::new(event));
    }

    fn project(&self, id: &ProjectId) -> Result<Project, BackendError> {
        self.inner
            .registry
            .resolve(id)
            .cloned()
            .map_err(|error| BackendError::bad_request("project_not_allowed", error.to_string()))
    }

    async fn open_codex_change(
        &self,
        thread_id: &str,
        turn_id: &str,
        path: &str,
        expected_cwd: Option<&str>,
    ) -> Result<BackendResult, BackendError> {
        let (workspace, change) = self
            .inner
            .state
            .read(|state| {
                prepare_editor_navigation(
                    state,
                    &EditorNavigation {
                        thread_id,
                        turn_id,
                        path,
                        expected_cwd,
                    },
                )
            })
            .await?;
        orangedeck_infra::open_changed_file(
            &workspace,
            &change.path,
            change.first_line,
            self.inner.config.editor,
        )
        .await
        .map_err(|message| BackendError::bad_request("editor_unavailable", message))?;
        Ok(BackendResult::accepted(
            "Mac 편집기에 파일 열기를 요청했습니다 / File sent to your Mac editor",
        ))
    }
}

struct EditorNavigation<'a> {
    thread_id: &'a str,
    turn_id: &'a str,
    path: &'a str,
    expected_cwd: Option<&'a str>,
}

fn prepare_editor_navigation(
    state: &DashboardState,
    request: &EditorNavigation<'_>,
) -> Result<(PathBuf, orangedeck_domain::CodeChange), BackendError> {
    let (thread, change) = orangedeck_application::recorded_change(
        state,
        request.thread_id,
        request.turn_id,
        request.path,
    )
    .map_err(|message| BackendError::bad_request("file_not_allowed", message))?;
    if request
        .expected_cwd
        .is_some_and(|expected| expected != thread.cwd)
    {
        return Err(BackendError::bad_request(
            "file_not_allowed",
            "대화의 폴더가 바뀌었습니다. 파일 창을 다시 열어 주세요 / Conversation folder changed; reopen the files window",
        ));
    }
    let workspace = orangedeck_infra::conversation_editor_workspace(&thread.cwd)
        .map_err(|message| BackendError::bad_request("editor_workspace_unavailable", message))?;
    orangedeck_infra::resolve_editor_file(&workspace, &change.path)
        .map_err(|message| BackendError::bad_request("file_not_allowed", message))?;
    Ok((workspace, change.clone()))
}

#[async_trait]
impl ConnectorBackend for RealBackend {
    async fn snapshot(&self) -> SnapshotDto {
        let mut snapshot = self.inner.state.snapshot().await;
        if let Ok(history) = self.inner.notifications.lock() {
            snapshot.notifications = history.iter().cloned().collect();
        }
        snapshot
    }

    async fn execute(
        &self,
        command: &orangedeck_protocol::ClientCommand,
    ) -> Result<BackendResult, BackendError> {
        let command = validate_command(command, &self.inner.registry)
            .map_err(|error| BackendError::bad_request("invalid_command", error.to_string()))?;
        match command {
            ValidatedCommand::SelectProject(project_id) => {
                self.inner
                    .state
                    .update(|state| state.selected_project = Some(project_id.clone()))
                    .await;
                self.publish_snapshot().await;
                Ok(BackendResult::accepted(format!("Selected {project_id}")))
            }
            ValidatedCommand::RefreshState => {
                self.refresh_system().await;
                if let Err(error) = self.refresh_codex().await {
                    warn!(%error, "Codex portion of state refresh failed");
                }
                self.publish_snapshot().await;
                Ok(BackendResult::accepted("State refreshed"))
            }
            ValidatedCommand::RunCargo { project_id, kind } => {
                let project = self.project(&project_id)?;
                let job_id = self
                    .inner
                    .jobs
                    .start(&project, kind)
                    .await
                    .map_err(job_error)?;
                Ok(BackendResult {
                    message: format!("{} started", kind.label()),
                    job_id: Some(job_id),
                })
            }
            ValidatedCommand::CancelJob(job_id) => {
                if self.inner.jobs.cancel(job_id).await {
                    Ok(BackendResult::accepted(format!(
                        "Cancellation requested for {job_id}"
                    )))
                } else {
                    Err(BackendError::conflict(
                        "job_not_running",
                        format!("Job {job_id} is not running"),
                    ))
                }
            }
            ValidatedCommand::RefreshGit(project_id) => {
                self.refresh_git(&project_id).await?;
                Ok(BackendResult::accepted("Git state refreshed"))
            }
            ValidatedCommand::OpenEditor(project_id) => {
                open_editor(&self.project(&project_id)?).map_err(system_error)?;
                Ok(BackendResult::accepted("Zed opened"))
            }
            ValidatedCommand::OpenTerminal(project_id) => {
                open_terminal(&self.project(&project_id)?).map_err(system_error)?;
                Ok(BackendResult::accepted("Terminal opened"))
            }
            ValidatedCommand::OpenBrowser(project_id) => {
                open_browser(&self.project(&project_id)?).map_err(system_error)?;
                Ok(BackendResult::accepted("Browser opened"))
            }
            ValidatedCommand::OpenProject(project_id) => {
                open_project(&self.project(&project_id)?).map_err(system_error)?;
                Ok(BackendResult::accepted("Project opened"))
            }
            ValidatedCommand::CodexRefreshThreads => {
                self.refresh_codex().await?;
                Ok(BackendResult::accepted("Codex state refreshed"))
            }
            ValidatedCommand::CodexReadThread(thread_id) => {
                let known = self
                    .inner
                    .state
                    .read(|state| {
                        state
                            .codex
                            .threads
                            .iter()
                            .any(|thread| thread.id == thread_id)
                    })
                    .await;
                if !known {
                    return Err(BackendError::bad_request(
                        "unknown_thread",
                        "Select a listed Codex thread",
                    ));
                }
                *self.inner.watched_thread.write().await = Some(thread_id.clone());
                let _guard = self.inner.thread_refresh_lock.lock().await;
                self.connect_codex()
                    .await?
                    .read_thread(&thread_id)
                    .await
                    .map_err(codex_error)?;
                Ok(BackendResult::accepted(
                    "대화 기록 읽음 · 외부 세션 제어 없음",
                ))
            }
            ValidatedCommand::CodexStartThread(project_id) => {
                let client = self.connect_codex().await?;
                let thread = client
                    .start_thread(&self.project(&project_id)?)
                    .await
                    .map_err(codex_error)?;
                Ok(BackendResult::accepted(format!(
                    "Created Codex thread {}",
                    thread.id
                )))
            }
            ValidatedCommand::CodexWatchThreads(ids) => {
                let known = self
                    .inner
                    .state
                    .read(|state| {
                        ids.iter()
                            .all(|id| state.codex.threads.iter().any(|thread| &thread.id == id))
                    })
                    .await;
                if !known {
                    return Err(BackendError::bad_request(
                        "unknown_thread",
                        "Select listed Codex conversations",
                    ));
                }
                *self.inner.watched_deck.write().await = ids;
                self.refresh_monitored_threads().await?;
                Ok(BackendResult::accepted(
                    "선택한 대화를 확인합니다 / Watching selected conversations",
                ))
            }
            ValidatedCommand::OpenCodexChange {
                thread_id,
                turn_id,
                path,
            } => {
                self.open_codex_change(&thread_id, &turn_id, &path, None)
                    .await
            }
            ValidatedCommand::RegisterCodexProject {
                thread_id,
                turn_id,
                expected_cwd,
                path,
            } => {
                // Compatibility with 0.1.25 clients; no registration is needed or saved.
                self.open_codex_change(&thread_id, &turn_id, &path, Some(&expected_cwd))
                    .await
            }
            ValidatedCommand::CodexSendPrompt { thread_id, prompt } => {
                let client = self.connect_codex().await?;
                let turn_id = client
                    .send_prompt(&thread_id, &prompt)
                    .await
                    .map_err(codex_error)?;
                info!(%thread_id, %turn_id, "Codex turn started from OrangeDeck");
                Ok(BackendResult::accepted(format!("Turn {turn_id} started")))
            }
            ValidatedCommand::CodexInterrupt { thread_id, turn_id } => {
                self.connect_codex()
                    .await?
                    .interrupt(&thread_id, &turn_id)
                    .await
                    .map_err(codex_error)?;
                Ok(BackendResult::accepted("Codex interrupt requested"))
            }
            ValidatedCommand::CodexApprovalResponse {
                approval_id,
                approve,
            } => {
                if let Some(hub) = self.inner.hooks.read().await.as_ref()
                    && let Some(sent) = hub.resolve(approval_id, Some(approve)).await
                {
                    return if sent {
                        Ok(BackendResult::accepted("선택을 Mac Codex에 전송했습니다"))
                    } else {
                        Err(BackendError::conflict(
                            "approval_expired",
                            "이 승인 요청은 이미 종료되었습니다",
                        ))
                    };
                }
                self.connect_codex()
                    .await?
                    .resolve_approval(approval_id, approve)
                    .await
                    .map_err(codex_error)?;
                Ok(BackendResult::accepted(if approve {
                    "Approval accepted for this action only"
                } else {
                    "Approval rejected"
                }))
            }
            ValidatedCommand::DemoScenario(_) => Err(BackendError::bad_request(
                "demo_only",
                "Demo scenarios are disabled on the real connector",
            )),
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<ServerEnvelope> {
        self.inner.events.subscribe()
    }

    async fn shutdown(&self) {
        if let Some(hub) = self.inner.hooks.read().await.as_ref() {
            hub.shutdown().await;
        }
        self.inner.jobs.shutdown().await;
        if let Some(client) = self.inner.codex.read().await.clone() {
            client.shutdown().await;
        }
    }

    fn mode(&self) -> &'static str {
        "real"
    }
}

#[allow(clippy::needless_pass_by_value)]
fn codex_error(error: orangedeck_infra::CodexError) -> BackendError {
    match error {
        orangedeck_infra::CodexError::ExternalThread(_) => {
            BackendError::conflict("external_thread_read_only", error.to_string())
        }
        orangedeck_infra::CodexError::UnknownApproval(_) => {
            BackendError::conflict("approval_not_found", error.to_string())
        }
        _ => BackendError::internal("codex_error", error.to_string()),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn job_error(error: JobError) -> BackendError {
    match error {
        JobError::Busy => BackendError::conflict("cargo_job_running", error.to_string()),
        _ => BackendError::internal("cargo_start_failed", error.to_string()),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn system_error(error: orangedeck_infra::SystemError) -> BackendError {
    BackendError::internal("host_action_failed", error.to_string())
}
