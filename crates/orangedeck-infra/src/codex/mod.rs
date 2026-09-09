mod changes;
mod history;
mod parser;
mod recorded_edits;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use orangedeck_domain::{
    AccountUsage, CodexConnectionState, CodexEvent, CodexLimits, CodexThread, Project,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, RwLock, broadcast, oneshot},
    time::{Duration, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::write_secure;

pub use parser::{
    activity_from_item, approval_result, parse_account_usage, parse_approval, parse_limits,
    parse_observation, parse_thread, parse_thread_status, parse_token_usage, parse_turn_status,
};

pub const TESTED_CODEX_VERSION: &str = "0.153.2";
mod usage_log;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
type RpcResponder = oneshot::Sender<Result<Value, RpcFailure>>;
type PendingRequests = Arc<Mutex<HashMap<u64, RpcResponder>>>;

#[derive(Clone, Debug)]
pub struct CodexProbe {
    pub executable: PathBuf,
    pub version: Option<String>,
    pub app_server_supported: bool,
    pub schema_generation_supported: bool,
    pub error: Option<String>,
}

pub async fn probe_codex(executable: &Path) -> CodexProbe {
    let version_result = Command::new(executable).arg("--version").output().await;
    let (version, mut error) = match version_result {
        Ok(output) if output.status.success() => (
            Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
            None,
        ),
        Ok(output) => (
            None,
            Some(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
        ),
        Err(source) => (None, Some(source.to_string())),
    };
    let help = Command::new(executable)
        .args(["app-server", "--help"])
        .output()
        .await;
    let (app_server_supported, schema_generation_supported) = match help {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            (true, stdout.contains("generate-json-schema"))
        }
        Ok(output) => {
            if error.is_none() {
                error = Some(String::from_utf8_lossy(&output.stderr).trim().to_owned());
            }
            (false, false)
        }
        Err(source) => {
            if error.is_none() {
                error = Some(source.to_string());
            }
            (false, false)
        }
    };
    CodexProbe {
        executable: executable.to_path_buf(),
        version,
        app_server_supported,
        schema_generation_supported,
        error,
    }
}

pub async fn generate_codex_schema(executable: &Path, output: &Path) -> Result<(), CodexError> {
    let result = Command::new(executable)
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(output)
        .output()
        .await
        .map_err(CodexError::Spawn)?;
    if result.status.success() {
        Ok(())
    } else {
        Err(CodexError::Process(
            String::from_utf8_lossy(&result.stderr).trim().to_owned(),
        ))
    }
}

#[derive(Clone)]
pub struct CodexClient {
    writer: Arc<Mutex<ChildStdin>>,
    pending: PendingRequests,
    pending_approvals: Arc<Mutex<HashMap<Uuid, PendingApproval>>>,
    owned_threads: Arc<RwLock<HashSet<String>>>,
    loaded_threads: Arc<RwLock<HashSet<String>>>,
    owned_threads_path: PathBuf,
    projects: Arc<Vec<Project>>,
    events: broadcast::Sender<CodexEvent>,
    next_id: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
    shutdown: CancellationToken,
    version: String,
    compatible: bool,
    usage_reader: Arc<std::sync::Mutex<usage_log::UsageLogReader>>,
}

#[derive(Clone, Debug)]
struct PendingApproval {
    rpc_id: Value,
    method: String,
    params: Value,
    thread_id: Option<String>,
}

impl CodexClient {
    pub async fn spawn(
        executable: &Path,
        owned_threads_path: PathBuf,
        projects: Vec<Project>,
    ) -> Result<Self, CodexError> {
        let probe = probe_codex(executable).await;
        if !probe.app_server_supported {
            return Err(CodexError::Unsupported(
                probe
                    .error
                    .unwrap_or_else(|| "codex app-server is unavailable".to_owned()),
            ));
        }
        let raw_version = probe.version.unwrap_or_else(|| "unknown".to_owned());
        let parsed_version = raw_version
            .split_whitespace()
            .last()
            .unwrap_or(&raw_version)
            .to_owned();
        let compatible = parsed_version == TESTED_CODEX_VERSION;
        let owned_threads = load_owned_threads(&owned_threads_path)?;

        let mut command = Command::new(executable);
        command
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(CodexError::Spawn)?;
        let stdin = child.stdin.take().ok_or(CodexError::MissingPipe("stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(CodexError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(CodexError::MissingPipe("stderr"))?;
        let (events, _) = broadcast::channel(512);
        let client = Self {
            writer: Arc::new(Mutex::new(stdin)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            owned_threads: Arc::new(RwLock::new(owned_threads)),
            loaded_threads: Arc::new(RwLock::new(HashSet::new())),
            owned_threads_path,
            projects: Arc::new(projects),
            events,
            next_id: Arc::new(AtomicU64::new(1)),
            alive: Arc::new(AtomicBool::new(true)),
            shutdown: CancellationToken::new(),
            version: parsed_version,
            compatible,
            usage_reader: Arc::new(std::sync::Mutex::new(usage_log::UsageLogReader::default())),
        };

        client.start_stdout_reader(stdout);
        Self::start_stderr_reader(stderr);
        client.start_process_watcher(child);

        let initialize = serde_json::json!({
            "clientInfo": {
                "name": "orangedeck",
                "title": "OrangeDeck",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "experimentalApi": true
            }
        });
        if let Err(error) = client.request("initialize", Some(initialize)).await {
            client.shutdown.cancel();
            return Err(error);
        }
        client
            .notify("initialized", Some(serde_json::json!({})))
            .await?;
        let _ = client.events.send(CodexEvent::ConnectionChanged {
            state: CodexConnectionState::Connected,
            message: (!client.compatible).then(|| {
                format!(
                    "Codex {} has not been regression-tested; expected {}",
                    client.version, TESTED_CODEX_VERSION
                )
            }),
        });
        Ok(client)
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn compatible(&self) -> bool {
        self.compatible
    }

    pub fn is_connected(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CodexEvent> {
        self.events.subscribe()
    }

    pub async fn refresh_threads(&self) -> Result<Vec<CodexThread>, CodexError> {
        let mut data = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen_cursors = HashSet::new();
        loop {
            let response = self.request("thread/list", Some(serde_json::json!({
                "limit": 100, "sortKey": "updated_at", "sortDirection": "desc",
                "sourceKinds": ["cli", "vscode", "exec", "appServer", "subAgent", "subAgentReview", "subAgentCompact", "subAgentThreadSpawn", "subAgentOther", "unknown"],
                "cursor": cursor
            }))).await?;
            let page = response
                .get("data")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    CodexError::Protocol("thread/list response has no data".to_owned())
                })?;
            data.extend(page.iter().cloned());
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let Some(next) = &cursor else {
                break;
            };
            if !seen_cursors.insert(next.clone()) || data.len() >= 10_000 {
                return Err(CodexError::Protocol(
                    "Project catalog is incomplete; pagination did not finish".to_owned(),
                ));
            }
        }
        let mut seen_ids = HashSet::new();
        data.retain(|value| {
            value
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| seen_ids.insert(id.to_owned()))
        });
        let owned = self.owned_threads.read().await;
        let mut threads = data
            .iter()
            .filter_map(|value| match parse_thread(value, &owned, &self.projects) {
                Ok(thread) => Some(thread),
                Err(error) => {
                    warn!(%error, "skipping malformed Codex thread");
                    None
                }
            })
            .collect::<Vec<_>>();
        drop(owned);
        let (mut usage, mut activity) = self.read_session_usage(&data, true).await;
        for thread in &mut threads {
            thread.live_usage = usage.remove(&thread.id).map(Box::new);
            thread.activity = activity.remove(&thread.id).map(Box::new);
        }
        let _ = self
            .events
            .send(CodexEvent::ThreadsReplaced(threads.clone()));
        Ok(threads)
    }

    pub async fn start_thread(&self, project: &Project) -> Result<CodexThread, CodexError> {
        let response = self
            .request("thread/start", Some(thread_start_params(project)))
            .await?;
        let value = response.get("thread").ok_or_else(|| {
            CodexError::Protocol("thread/start response has no thread".to_owned())
        })?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| CodexError::Protocol("new thread has no id".to_owned()))?
            .to_owned();
        self.owned_threads.write().await.insert(id.clone());
        self.loaded_threads.write().await.insert(id);
        self.persist_owned_threads().await?;
        let owned = self.owned_threads.read().await;
        let thread = parse_thread(value, &owned, &self.projects)?;
        let _ = self.events.send(CodexEvent::ThreadUpdated(thread.clone()));
        Ok(thread)
    }

    /// Read persisted history without loading/resuming or subscribing to the thread.
    pub async fn read_thread(&self, thread_id: &str) -> Result<CodexThread, CodexError> {
        let history = self.read_history(thread_id).await?;
        let value = &history;
        let owned = self.owned_threads.read().await;
        let mut thread = parse_thread(value, &owned, &self.projects)?;
        thread.observation = Some(parse_observation(value)?);
        drop(owned);
        let (mut usage, mut activity) = self
            .read_session_usage(std::slice::from_ref(value), false)
            .await;
        thread.live_usage = usage.remove(&thread.id).map(Box::new);
        thread.activity = activity.remove(&thread.id).map(Box::new);
        if let Some(observation) = &mut thread.observation
            && let Some(turn_id) = observation.turn_id.as_deref()
            && observation
                .changes
                .as_ref()
                .is_none_or(|changes| changes.files.is_empty())
            && let Some(mut changes) = self
                .usage_reader
                .lock()
                .ok()
                .and_then(|reader| reader.changes(&thread.id, turn_id))
        {
            changes.truncated |= observation
                .changes
                .as_ref()
                .is_some_and(|value| value.truncated);
            observation.changes = Some(changes);
        }
        let _ = self.events.send(CodexEvent::ThreadUpdated(thread.clone()));
        Ok(thread)
    }

    async fn read_session_usage(
        &self,
        records: &[Value],
        prune: bool,
    ) -> (
        HashMap<String, orangedeck_domain::LiveTokenUsage>,
        HashMap<String, orangedeck_domain::ThreadActivity>,
    ) {
        let listed: Vec<_> = records
            .iter()
            .filter_map(|value| {
                Some((
                    value.get("id")?.as_str()?.to_owned(),
                    PathBuf::from(value.get("path")?.as_str()?),
                ))
            })
            .take(100)
            .collect();
        let reader = self.usage_reader.clone();
        let events = self.events.clone();
        tokio::task::spawn_blocking(move || {
            let Ok(mut reader) = reader.lock() else {
                return (HashMap::new(), HashMap::new());
            };
            let now = chrono::Utc::now();
            let usage = if prune {
                reader.read_listed(&listed, now)
            } else {
                listed
                    .iter()
                    .filter_map(|(id, path)| {
                        let result = reader.read_one(id, path, now);
                        if result.is_err() {
                            reader.forget(id);
                        }
                        result.ok().flatten().map(|usage| (id.clone(), usage))
                    })
                    .collect()
            };
            for event in reader.drain_events() {
                let _ = events.send(event);
            }
            let activity = listed
                .iter()
                .filter_map(|(id, _)| reader.activity(id, now).map(|value| (id.clone(), value)))
                .collect();
            (usage, activity)
        })
        .await
        .unwrap_or_default()
    }

    pub async fn refresh_account_usage(&self) -> Result<AccountUsage, CodexError> {
        let response = self
            .request("account/usage/read", Some(serde_json::json!({})))
            .await?;
        let usage = parse_account_usage(&response)?;
        let _ = self
            .events
            .send(CodexEvent::AccountUsageUpdated(usage.clone()));
        Ok(usage)
    }

    pub async fn send_prompt(&self, thread_id: &str, prompt: &str) -> Result<String, CodexError> {
        self.ensure_owned(thread_id).await?;
        self.ensure_loaded(thread_id).await?;
        let response = self
            .request(
                "turn/start",
                Some(serde_json::json!({
                    "threadId": thread_id,
                    "input": [{ "type": "text", "text": prompt }]
                })),
            )
            .await?;
        let turn_id = response
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| CodexError::Protocol("turn/start response has no turn id".to_owned()))?
            .to_owned();
        let _ = self.events.send(CodexEvent::TurnStarted {
            thread_id: thread_id.to_owned(),
            turn_id: turn_id.clone(),
        });
        Ok(turn_id)
    }

    pub async fn interrupt(&self, thread_id: &str, turn_id: &str) -> Result<(), CodexError> {
        self.ensure_owned(thread_id).await?;
        self.request(
            "turn/interrupt",
            Some(serde_json::json!({ "threadId": thread_id, "turnId": turn_id })),
        )
        .await?;
        Ok(())
    }

    pub async fn refresh_limits(&self) -> Result<CodexLimits, CodexError> {
        let response = self.request("account/rateLimits/read", None).await?;
        let limits = parse_limits(&response)?;
        let _ = self.events.send(CodexEvent::LimitsUpdated(limits.clone()));
        Ok(limits)
    }

    pub async fn resolve_approval(
        &self,
        approval_id: Uuid,
        approve: bool,
    ) -> Result<(), CodexError> {
        let pending = self
            .pending_approvals
            .lock()
            .await
            .remove(&approval_id)
            .ok_or(CodexError::UnknownApproval(approval_id))?;
        let external_thread = if approve {
            if let Some(thread_id) = pending.thread_id.as_ref() {
                (!self.owned_threads.read().await.contains(thread_id)).then(|| thread_id.clone())
            } else {
                None
            }
        } else {
            None
        };
        if let Some(thread_id) = external_thread {
            self.pending_approvals
                .lock()
                .await
                .insert(approval_id, pending);
            return Err(CodexError::ExternalThread(thread_id));
        }
        let result = approval_result(&pending.method, approve, &pending.params);
        self.write_value(&serde_json::json!({
            "id": pending.rpc_id,
            "result": result
        }))
        .await?;
        let _ = self.events.send(CodexEvent::ApprovalResolved(approval_id));
        Ok(())
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        if timeout(Duration::from_secs(4), async {
            while self.is_connected() {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .is_err()
        {
            warn!("timed out while stopping Codex app-server");
        }
    }

    async fn ensure_owned(&self, thread_id: &str) -> Result<(), CodexError> {
        if self.owned_threads.read().await.contains(thread_id) {
            Ok(())
        } else {
            Err(CodexError::ExternalThread(thread_id.to_owned()))
        }
    }

    async fn ensure_loaded(&self, thread_id: &str) -> Result<(), CodexError> {
        if self.loaded_threads.read().await.contains(thread_id) {
            return Ok(());
        }
        self.request(
            "thread/resume",
            Some(serde_json::json!({ "threadId": thread_id })),
        )
        .await?;
        self.loaded_threads
            .write()
            .await
            .insert(thread_id.to_owned());
        Ok(())
    }

    async fn persist_owned_threads(&self) -> Result<(), CodexError> {
        let mut thread_ids = self
            .owned_threads
            .read()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        thread_ids.sort();
        let file = OwnedThreadFile {
            version: 1,
            thread_ids,
        };
        let json = serde_json::to_string_pretty(&file)?;
        write_secure(&self.owned_threads_path, &json).map_err(CodexError::Config)
    }

    async fn request(
        &self,
        method: &'static str,
        params: Option<Value>,
    ) -> Result<Value, CodexError> {
        if !self.is_connected() {
            return Err(CodexError::Disconnected);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        let mut message = serde_json::json!({ "id": id, "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        debug!(method, request_id = id, "sending Codex app-server request");
        if let Err(error) = self.write_value(&message).await {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        match timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(error))) => Err(CodexError::Rpc(error)),
            Ok(Err(_)) => Err(CodexError::Disconnected),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(CodexError::Timeout(method))
            }
        }
    }

    async fn notify(&self, method: &'static str, params: Option<Value>) -> Result<(), CodexError> {
        let mut message = serde_json::json!({ "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.write_value(&message).await
    }

    async fn write_value(&self, value: &Value) -> Result<(), CodexError> {
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        let mut writer = self.writer.lock().await;
        writer.write_all(&bytes).await.map_err(CodexError::Io)?;
        writer.flush().await.map_err(CodexError::Io)
    }

    fn start_stdout_reader<R>(&self, stdout: R)
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let client = self.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => match serde_json::from_str::<Value>(&line) {
                        Ok(message) => client.handle_message(message).await,
                        Err(error) => {
                            let _ = client.events.send(CodexEvent::Error(format!(
                                "invalid JSON from Codex app-server: {error}"
                            )));
                        }
                    },
                    Ok(None) => break,
                    Err(error) => {
                        let _ = client.events.send(CodexEvent::Error(format!(
                            "cannot read Codex app-server output: {error}"
                        )));
                        break;
                    }
                }
            }
        });
    }

    fn start_stderr_reader<R>(stderr: R)
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(
                    diagnostic_bytes = line.len(),
                    "Codex app-server diagnostic content suppressed"
                );
            }
        });
    }

    fn start_process_watcher(&self, mut child: Child) {
        let client = self.clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                status = child.wait() => status.map(|status| format!("Codex app-server exited with {status}")),
                () = client.shutdown.cancelled() => {
                    terminate_child_group(&mut child).await;
                    Ok("Codex app-server stopped".to_owned())
                }
            };
            client.alive.store(false, Ordering::Release);
            let message =
                result.unwrap_or_else(|error| format!("Codex app-server wait failed: {error}"));
            let pending = std::mem::take(&mut *client.pending.lock().await);
            for (_, sender) in pending {
                let _ = sender.send(Err(RpcFailure {
                    code: -32_000,
                    message: message.clone(),
                }));
            }
            let _ = client.events.send(CodexEvent::ConnectionChanged {
                state: CodexConnectionState::Disconnected,
                message: Some(message),
            });
        });
    }

    async fn handle_message(&self, message: Value) {
        if let (Some(method), Some(rpc_id)) = (
            message.get("method").and_then(Value::as_str),
            message.get("id"),
        ) {
            self.handle_server_request(method, rpc_id.clone(), &message["params"])
                .await;
            return;
        }
        if let Some(id) = message.get("id").and_then(Value::as_u64) {
            if let Some(sender) = self.pending.lock().await.remove(&id) {
                let response = if let Some(error) = message.get("error") {
                    Err(RpcFailure {
                        code: error.get("code").and_then(Value::as_i64).unwrap_or(-32_000),
                        message: error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown Codex RPC error")
                            .to_owned(),
                    })
                } else {
                    Ok(message.get("result").cloned().unwrap_or(Value::Null))
                };
                let _ = sender.send(response);
            }
            return;
        }
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            self.handle_notification(method, &message["params"]).await;
        }
    }

    async fn handle_server_request(&self, method: &str, rpc_id: Value, params: &Value) {
        if matches!(
            method,
            "item/commandExecution/requestApproval"
                | "item/fileChange/requestApproval"
                | "item/permissions/requestApproval"
                | "execCommandApproval"
                | "applyPatchApproval"
        ) {
            let approval = parse_approval(method, params);
            self.pending_approvals.lock().await.insert(
                approval.id,
                PendingApproval {
                    rpc_id,
                    method: method.to_owned(),
                    params: params.clone(),
                    thread_id: approval.thread_id.clone(),
                },
            );
            let _ = self.events.send(CodexEvent::ApprovalRequested(approval));
            return;
        }

        let response = serde_json::json!({
            "id": rpc_id,
            "error": {
                "code": -32601,
                "message": "OrangeDeck does not support this server request"
            }
        });
        if let Err(error) = self.write_value(&response).await {
            let _ = self.events.send(CodexEvent::Error(error.to_string()));
        }
    }

    async fn handle_notification(&self, method: &str, params: &Value) {
        match method {
            "thread/started" => {
                if let Some(value) = params.get("thread") {
                    let owned = self.owned_threads.read().await;
                    if let Ok(thread) = parse_thread(value, &owned, &self.projects) {
                        let _ = self.events.send(CodexEvent::ThreadUpdated(thread));
                    }
                }
            }
            "turn/started" => {
                if let (Some(thread_id), Some(turn_id)) = (
                    params.get("threadId").and_then(Value::as_str),
                    params
                        .get("turn")
                        .and_then(|turn| turn.get("id"))
                        .and_then(Value::as_str),
                ) {
                    let _ = self.events.send(CodexEvent::TurnStarted {
                        thread_id: thread_id.to_owned(),
                        turn_id: turn_id.to_owned(),
                    });
                }
            }
            "turn/completed" => {
                if let (Some(thread_id), Some(turn)) = (
                    params.get("threadId").and_then(Value::as_str),
                    params.get("turn"),
                ) && let Some(turn_id) = turn.get("id").and_then(Value::as_str)
                {
                    let _ = self.events.send(CodexEvent::TurnCompleted {
                        thread_id: thread_id.to_owned(),
                        turn_id: turn_id.to_owned(),
                        status: parse_turn_status(turn.get("status")),
                    });
                }
            }
            "thread/tokenUsage/updated" => {
                if let Some(thread_id) = params.get("threadId").and_then(Value::as_str)
                    && let Ok(usage) = parse_token_usage(params)
                {
                    let _ = self.events.send(CodexEvent::TokenUsageUpdated {
                        thread_id: thread_id.to_owned(),
                        usage,
                    });
                }
            }
            "account/rateLimits/updated" => {
                if let Ok(limits) = parse_limits(params) {
                    let _ = self.events.send(CodexEvent::LimitsUpdated(limits));
                }
            }
            "item/agentMessage/delta" => {
                let text = params
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !text.is_empty() {
                    let _ = self.events.send(CodexEvent::Activity {
                        thread_id: params
                            .get("threadId")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        turn_id: params
                            .get("turnId")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        kind: "message".to_owned(),
                        text: text.to_owned(),
                    });
                }
            }
            "item/started" => {
                let (thread_id, turn_id, kind, text) = activity_from_item(params);
                let _ = self.events.send(CodexEvent::Activity {
                    thread_id,
                    turn_id,
                    kind,
                    text,
                });
            }
            "error" => {
                let message = params
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex reported an unknown error")
                    .to_owned();
                let _ = self.events.send(CodexEvent::Error(message));
            }
            "thread/closed" => {
                if let Some(thread_id) = params.get("threadId").and_then(Value::as_str) {
                    self.loaded_threads.write().await.remove(thread_id);
                }
            }
            _ => debug!(method, "ignored Codex notification"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OwnedThreadFile {
    version: u16,
    thread_ids: Vec<String>,
}

fn load_owned_threads(path: &Path) -> Result<HashSet<String>, CodexError> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let text = std::fs::read_to_string(path).map_err(CodexError::Io)?;
    let file: OwnedThreadFile = serde_json::from_str(&text)?;
    if file.version != 1 {
        return Err(CodexError::Protocol(format!(
            "unsupported owned-thread registry version {}",
            file.version
        )));
    }
    Ok(file.thread_ids.into_iter().collect())
}

fn thread_start_params(project: &Project) -> Value {
    serde_json::json!({
        "cwd": project.path,
        "approvalPolicy": "on-request",
        "sandbox": "workspace-write",
        "serviceName": "orangedeck"
    })
}

#[cfg(unix)]
async fn terminate_child_group(child: &mut Child) {
    use nix::{
        sys::signal::{Signal, killpg},
        unistd::Pid,
    };

    if let Some(raw_id) = child.id()
        && let Ok(id) = i32::try_from(raw_id)
    {
        let _ = killpg(Pid::from_raw(id), Signal::SIGTERM);
    }
    if timeout(Duration::from_secs(2), child.wait()).await.is_err() {
        if let Some(raw_id) = child.id()
            && let Ok(id) = i32::try_from(raw_id)
        {
            let _ = killpg(Pid::from_raw(id), Signal::SIGKILL);
        }
        let _ = child.wait().await;
    }
}

#[cfg(not(unix))]
async fn terminate_child_group(child: &mut Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[derive(Clone, Debug, Error)]
#[error("Codex RPC error {code}: {message}")]
pub struct RpcFailure {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum CodexError {
    #[error("cannot start Codex: {0}")]
    Spawn(std::io::Error),
    #[error("Codex process I/O failed: {0}")]
    Io(std::io::Error),
    #[error("Codex process is missing its {0} pipe")]
    MissingPipe(&'static str),
    #[error("Codex app-server is unsupported: {0}")]
    Unsupported(String),
    #[error("Codex app-server is disconnected")]
    Disconnected,
    #[error("Codex request `{0}` timed out")]
    Timeout(&'static str),
    #[error(transparent)]
    Rpc(#[from] RpcFailure),
    #[error("Codex protocol error: {0}")]
    Protocol(String),
    #[error("thread `{0}` is external/read-only and cannot be controlled by OrangeDeck")]
    ExternalThread(String),
    #[error("approval `{0}` no longer exists")]
    UnknownApproval(Uuid),
    #[error("Codex process failed: {0}")]
    Process(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Config(#[from] crate::ConfigError),
}
