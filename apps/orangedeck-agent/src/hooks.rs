//! Codex's opt-in, human-operated PermissionRequest/Stop hooks.
//! A private local socket belongs to the existing Agent; no remote shell or new daemon.
use std::{
    collections::HashMap,
    io,
    os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::Utc;
use orangedeck_domain::{ApprovalKind, ApprovalRequest, CodexThreadStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex, Semaphore, broadcast, oneshot},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_INPUT: u64 = 65_536;
const WAIT: Duration = Duration::from_mins(2);

#[derive(Clone, Debug)]
pub enum HookEvent {
    Lifecycle {
        thread_id: String,
        turn_id: String,
        cwd: String,
        status: CodexThreadStatus,
    },
    Approval(ApprovalRequest),
    Resolved(Uuid),
}

#[derive(Deserialize, Serialize)]
struct HookPayload {
    hook_event_name: String,
    session_id: String,
    cwd: String,
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_input: Option<Value>,
}

struct Pending {
    sender: oneshot::Sender<Option<bool>>,
}

#[derive(Clone)]
pub struct HookHub {
    pending: Arc<Mutex<HashMap<Uuid, Pending>>>,
    events: broadcast::Sender<HookEvent>,
    shutdown: CancellationToken,
    socket: PathBuf,
}

pub fn default_socket() -> PathBuf {
    orangedeck_infra::default_config_dir()
        .join("hooks")
        .join("codex.sock")
}

impl HookHub {
    pub async fn bind(socket: PathBuf) -> io::Result<Self> {
        let parent = socket
            .parent()
            .ok_or_else(|| io::Error::other("missing socket directory"))?;
        if !parent.exists() {
            std::fs::DirBuilder::new().mode(0o700).create(parent)?;
        }
        let parent_metadata = std::fs::metadata(parent)?;
        if parent_metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "OrangeDeck hook socket needs a private 0700 directory",
            ));
        }
        match std::fs::symlink_metadata(&socket) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() || metadata.uid() != parent_metadata.uid() {
                    return Err(io::Error::other(
                        "refusing to replace an unexpected socket path",
                    ));
                }
                match UnixStream::connect(&socket).await {
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            "another OrangeDeck hook listener is running",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                        std::fs::remove_file(&socket)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        let owner = std::fs::metadata(&socket)?.uid();
        let (events, _) = broadcast::channel(256);
        let hub = Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            events,
            shutdown: CancellationToken::new(),
            socket,
        };
        let task_hub = hub.clone();
        tokio::spawn(async move {
            let capacity = Arc::new(Semaphore::new(32));
            loop {
                tokio::select! {
                    () = task_hub.shutdown.cancelled() => break,
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break };
                        if !stream.peer_cred().is_ok_and(|peer| peer.uid() == owner) { continue; }
                        let Ok(permit) = capacity.clone().try_acquire_owned() else { continue; };
                        let handler = task_hub.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            let _ = handler.handle(stream).await;
                        });
                    }
                }
            }
            task_hub.pending.lock().await.clear();
        });
        Ok(hub)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HookEvent> {
        self.events.subscribe()
    }

    /// Removing under one lock makes two taps or two clients resolve at most once.
    pub async fn resolve(&self, id: Uuid, decision: Option<bool>) -> Option<bool> {
        self.pending
            .lock()
            .await
            .remove(&id)
            .map(|pending| pending.sender.send(decision).is_ok())
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        self.pending.lock().await.clear();
        let _ = std::fs::remove_file(&self.socket);
    }

    async fn handle(&self, stream: UnixStream) -> io::Result<()> {
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::new();
        tokio::time::timeout(
            Duration::from_secs(3),
            (&mut reader)
                .take(MAX_INPUT + 1)
                .read_until(b'\n', &mut bytes),
        )
        .await
        .map_err(|_| io::Error::other("hook input timed out"))??;
        if bytes.len() > usize::try_from(MAX_INPUT).unwrap_or(0) || bytes.last() != Some(&b'\n') {
            return Err(io::Error::other("invalid hook input length"));
        }
        let input: HookPayload = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        validate(&input)?;
        let mut stream = reader.into_inner();
        if input.hook_event_name != "PermissionRequest" {
            let status = match input.hook_event_name.as_str() {
                "UserPromptSubmit" => CodexThreadStatus::Working,
                "Stop" => CodexThreadStatus::Completed,
                "Interrupt" => CodexThreadStatus::Idle,
                _ => return Err(io::Error::other("unsupported hook event")),
            };
            let turn_id = input
                .turn_id
                .ok_or_else(|| io::Error::other("hook has no turn id"))?;
            let _ = self.events.send(HookEvent::Lifecycle {
                thread_id: input.session_id,
                turn_id,
                cwd: input.cwd,
                status,
            });
            stream.write_all(b"{}\n").await?;
            return Ok(());
        }
        let approval = approval(&input)?;
        let id = approval.id;
        let (sender, reply) = oneshot::channel();
        self.pending.lock().await.insert(id, Pending { sender });
        if self.events.send(HookEvent::Approval(approval)).is_err() {
            self.pending.lock().await.remove(&id);
            stream.write_all(b"{}\n").await?;
            return Ok(());
        }
        let mut closed = [0_u8; 1];
        let decision = tokio::select! {
            () = self.shutdown.cancelled() => None,
            () = tokio::time::sleep(WAIT) => None,
            _ = stream.read(&mut closed) => None,
            value = reply => value.ok().flatten(),
        };
        self.pending.lock().await.remove(&id);
        let _ = self.events.send(HookEvent::Resolved(id));
        let response = match decision {
            Some(true) => b"{\"decision\":\"allow\"}\n".as_slice(),
            Some(false) => b"{\"decision\":\"deny\"}\n".as_slice(),
            None => b"{}\n".as_slice(),
        };
        stream.write_all(response).await
    }
}

fn validate(input: &HookPayload) -> io::Result<()> {
    let safe_id = |id: &str| !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control);
    if !safe_id(&input.session_id)
        || !input.turn_id.as_deref().is_some_and(safe_id)
        || !Path::new(&input.cwd).is_absolute()
        || input.cwd.len() > 4096
        || input.cwd.chars().any(char::is_control)
    {
        return Err(io::Error::other("invalid Codex hook identity"));
    }
    Ok(())
}

fn approval(input: &HookPayload) -> io::Result<ApprovalRequest> {
    let tool = input
        .tool_name
        .as_deref()
        .filter(|tool| !tool.is_empty())
        .ok_or_else(|| io::Error::other("missing approval tool"))?;
    let args = input
        .tool_input
        .as_ref()
        .ok_or_else(|| io::Error::other("missing approval arguments"))?;
    let arguments = serde_json::to_string_pretty(args).map_err(io::Error::other)?;
    // A decision requires the whole action to be reviewable. Oversized requests stay in Codex.
    if tool.len() > 256 || arguments.len() > 24_000 {
        return Err(io::Error::other(
            "approval is too large to review on OrangeDeck",
        ));
    }
    let summary = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or(tool)
        .chars()
        .take(240)
        .collect();
    Ok(ApprovalRequest {
        id: Uuid::new_v4(),
        thread_id: Some(input.session_id.clone()),
        turn_id: input.turn_id.clone(),
        kind: match tool {
            "Bash" => ApprovalKind::CommandExecution,
            "apply_patch" => ApprovalKind::FileChange,
            _ => ApprovalKind::Permissions,
        },
        title: "Mac Codex 승인 요청".to_owned(),
        summary,
        details: vec![
            format!("작업 폴더: {}", input.cwd),
            format!("도구: {tool}"),
            arguments,
        ],
        requested_at: Utc::now(),
    })
}

/// Returns no decision on absent Agent, timeout, malformed input, or a closed socket.
/// The normal Codex approval prompt remains the fallback in all those cases.
pub fn run_hook(socket: &Path) {
    use std::io::{BufRead, Read, Write};
    let run = || -> io::Result<Value> {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_INPUT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > usize::try_from(MAX_INPUT).unwrap_or(0) {
            return Err(io::Error::other("oversized hook"));
        }
        let input: HookPayload = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        validate(&input)?;
        if input.hook_event_name == "PermissionRequest" {
            approval(&input)?;
        }
        let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.set_read_timeout(Some(if input.hook_event_name == "PermissionRequest" {
            WAIT + Duration::from_secs(2)
        } else {
            Duration::from_secs(3)
        }))?;
        // Deserializing and reserializing intentionally removes prompts, transcripts and replies.
        let mut message = serde_json::to_vec(&input).map_err(io::Error::other)?;
        message.push(b'\n');
        stream.write_all(&message)?;
        let mut reply = String::new();
        std::io::BufReader::new(stream)
            .take(1024)
            .read_line(&mut reply)?;
        let reply: Value = serde_json::from_str(&reply).map_err(io::Error::other)?;
        Ok(hook_output(
            input.hook_event_name == "PermissionRequest",
            &reply,
        ))
    };
    let output = run().unwrap_or_else(|_| json!({}));
    println!("{output}");
}

fn hook_output(permission: bool, reply: &Value) -> Value {
    if !permission {
        return json!({});
    }
    match reply.get("decision").and_then(Value::as_str) {
        Some("allow") => {
            json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}})
        }
        Some("deny") => {
            json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"사용자가 OrangeDeck에서 이 요청을 거부했습니다."}}})
        }
        _ => json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission() -> Value {
        json!({"hook_event_name":"PermissionRequest","session_id":"session-a","turn_id":"turn-a",
            "cwd":"/tmp/project","tool_name":"Bash","tool_input":{"command":"cargo check","description":"빌드 확인"}})
    }

    async fn connect(socket: &Path, input: Value) -> BufReader<UnixStream> {
        let mut stream = UnixStream::connect(socket).await.unwrap();
        stream
            .write_all(format!("{input}\n").as_bytes())
            .await
            .unwrap();
        BufReader::new(stream)
    }

    #[tokio::test]
    async fn only_an_explicit_decision_reaches_the_original_waiting_hook() {
        let dir = tempfile::tempdir().unwrap();
        let hub = HookHub::bind(dir.path().join("private/hook.sock"))
            .await
            .unwrap();
        let mut events = hub.subscribe();
        for approve in [true, false] {
            let mut client = connect(&hub.socket, permission()).await;
            let HookEvent::Approval(request) = events.recv().await.unwrap() else {
                panic!("expected request")
            };
            assert!(
                request
                    .details
                    .iter()
                    .any(|detail| detail.contains("cargo check"))
            );
            let mut line = String::new();
            assert!(
                tokio::time::timeout(Duration::from_millis(20), client.read_line(&mut line))
                    .await
                    .is_err()
            );
            assert_eq!(hub.resolve(request.id, Some(approve)).await, Some(true));
            assert_eq!(hub.resolve(request.id, Some(!approve)).await, None);
            client.read_line(&mut line).await.unwrap();
            let response: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(response["decision"], if approve { "allow" } else { "deny" });
            assert!(
                matches!(events.recv().await.unwrap(), HookEvent::Resolved(id) if id == request.id)
            );
        }
        hub.shutdown().await;
    }

    #[tokio::test]
    async fn closing_codex_invalidates_the_request_without_any_approval() {
        let dir = tempfile::tempdir().unwrap();
        let hub = HookHub::bind(dir.path().join("private/hook.sock"))
            .await
            .unwrap();
        let mut events = hub.subscribe();
        let client = connect(&hub.socket, permission()).await;
        let HookEvent::Approval(request) = events.recv().await.unwrap() else {
            panic!("expected request")
        };
        drop(client);
        assert!(
            matches!(tokio::time::timeout(Duration::from_secs(2), events.recv()).await.unwrap().unwrap(),
            HookEvent::Resolved(id) if id == request.id)
        );
        assert_eq!(hub.resolve(request.id, Some(true)).await, None);
        hub.shutdown().await;
    }

    #[tokio::test]
    async fn completion_and_interrupt_have_different_outcomes_and_no_prompt_content() {
        let dir = tempfile::tempdir().unwrap();
        let hub = HookHub::bind(dir.path().join("private/hook.sock"))
            .await
            .unwrap();
        let mut events = hub.subscribe();
        for (event, expected) in [
            ("Stop", CodexThreadStatus::Completed),
            ("Interrupt", CodexThreadStatus::Idle),
        ] {
            let mut client = connect(
                &hub.socket,
                json!({"hook_event_name":event,"session_id":"s","turn_id":"t",
                "cwd":"/tmp/project","last_assistant_message":"private reply"}),
            )
            .await;
            let received = events.recv().await.unwrap();
            assert!(matches!(received, HookEvent::Lifecycle { status, .. } if status == expected));
            assert!(!format!("{received:?}").contains("private reply"));
            let mut reply = String::new();
            client.read_line(&mut reply).await.unwrap();
            assert_eq!(reply, "{}\n");
        }
        hub.shutdown().await;
    }

    #[test]
    fn malformed_or_oversized_requests_and_unknown_replies_cannot_approve() {
        let mut input: HookPayload = serde_json::from_value(permission()).unwrap();
        input.tool_input = Some(json!({"command":"x".repeat(24_001)}));
        assert!(approval(&input).is_err());
        input.session_id = "bad\nidentity".to_owned();
        assert!(validate(&input).is_err());
        assert_eq!(hook_output(true, &json!({})), json!({}));
        assert_eq!(hook_output(false, &json!({"decision":"allow"})), json!({}));
        assert_eq!(
            hook_output(true, &json!({"decision":"allow"}))["hookSpecificOutput"]["decision"]["behavior"],
            "allow"
        );
    }
}
