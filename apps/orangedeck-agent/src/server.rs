use std::{net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use orangedeck_infra::AuthToken;
use orangedeck_protocol::{
    ApiError, ClientRequest, CommandResponse, HealthResponse, PROTOCOL_VERSION, ServerEnvelope,
    ServerEvent, SnapshotDto,
};
use tokio::{
    net::TcpListener,
    sync::broadcast,
    time::{Duration, interval},
};
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct BackendResult {
    pub message: String,
    pub job_id: Option<uuid::Uuid>,
}

impl BackendResult {
    pub fn accepted(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            job_id: None,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct BackendError {
    pub code: &'static str,
    pub message: String,
    pub status: StatusCode,
}

impl BackendError {
    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            status: StatusCode::BAD_REQUEST,
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            status: StatusCode::CONFLICT,
        }
    }

    pub fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            status: StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[async_trait]
pub trait AgentBackend: Send + Sync {
    async fn snapshot(&self) -> SnapshotDto;
    async fn execute(
        &self,
        command: &orangedeck_protocol::ClientCommand,
    ) -> Result<BackendResult, BackendError>;
    fn subscribe(&self) -> broadcast::Receiver<ServerEnvelope>;
    fn mode(&self) -> &'static str;
    async fn shutdown(&self) {}
}

#[derive(Clone)]
struct ServerState {
    token: AuthToken,
    backend: Arc<dyn AgentBackend>,
}

pub fn router(token: AuthToken, backend: Arc<dyn AgentBackend>) -> Router {
    let state = ServerState { token, backend };
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/snapshot", get(snapshot))
        .route("/api/v1/command", post(command))
        .route("/api/v1/events", get(events))
        .layer(DefaultBodyLimit::max(64 * 1024))
        // Authenticate before extractors wait for or parse an untrusted body/upgrade.
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .route("/readyz", get(ready))
        .with_state(state)
}

async fn authenticate(State(state): State<ServerState>, request: Request, next: Next) -> Response {
    if !authorized(request.headers(), &state.token) {
        return unauthorized_response();
    }
    next.run(request).await
}

pub async fn serve(
    address: SocketAddr,
    token: AuthToken,
    backend: Arc<dyn AgentBackend>,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(address).await?;
    info!(%address, mode = backend.mode(), "OrangeDeck Agent listening");
    let result = axum::serve(listener, router(token, backend.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await;
    backend.shutdown().await;
    result
}

#[cfg(unix)]
async fn shutdown_signal() {
    let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());
    match terminate {
        Ok(mut terminate) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
        }
        Err(error) => {
            warn!(%error, "cannot install SIGTERM handler; waiting for Ctrl-C");
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn ready() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ready": true }))
}

async fn health(State(state): State<ServerState>) -> Response {
    Json(HealthResponse {
        name: "OrangeDeck Agent".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        protocol_version: PROTOCOL_VERSION,
        ready: true,
        mode: state.backend.mode().to_owned(),
    })
    .into_response()
}

async fn snapshot(State(state): State<ServerState>) -> Response {
    Json(state.backend.snapshot().await).into_response()
}

async fn command(State(state): State<ServerState>, Json(request): Json<ClientRequest>) -> Response {
    if request.protocol_version != PROTOCOL_VERSION {
        return (
            StatusCode::UPGRADE_REQUIRED,
            Json(ApiError {
                code: "protocol_version_mismatch".to_owned(),
                message: format!(
                    "OrangeDeck protocol {} is required; client sent {}",
                    PROTOCOL_VERSION, request.protocol_version
                ),
                expected_protocol_version: Some(PROTOCOL_VERSION),
                received_protocol_version: Some(request.protocol_version),
            }),
        )
            .into_response();
    }
    match state.backend.execute(&request.command).await {
        Ok(result) => Json(CommandResponse {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id,
            accepted: true,
            message: result.message,
            job_id: result.job_id,
        })
        .into_response(),
        Err(error) => (
            error.status,
            Json(ApiError {
                code: error.code.to_owned(),
                message: error.message,
                expected_protocol_version: None,
                received_protocol_version: None,
            }),
        )
            .into_response(),
    }
}

async fn events(State(state): State<ServerState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| websocket_loop(socket, state.backend))
        .into_response()
}

async fn websocket_loop(socket: WebSocket, backend: Arc<dyn AgentBackend>) {
    let (mut sender, mut receiver) = socket.split();
    let initial = ServerEnvelope::new(ServerEvent::Snapshot(backend.snapshot().await));
    if send_event(&mut sender, &initial).await.is_err() {
        return;
    }
    let mut events = backend.subscribe();
    let mut heartbeat = interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    if send_event(&mut sender, &event).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(skipped, "WebSocket client lagged; replacing state with a snapshot");
                    let replacement = ServerEnvelope::new(ServerEvent::Snapshot(backend.snapshot().await));
                    if send_event(&mut sender, &replacement).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            incoming = receiver.next() => match incoming {
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    if sender.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                Some(Ok(_)) => {}
            },
            _ = heartbeat.tick() => {
                let event = ServerEnvelope::new(ServerEvent::Heartbeat {
                    server_time: chrono::Utc::now(),
                });
                if send_event(&mut sender, &event).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn send_event<S>(sender: &mut S, event: &ServerEnvelope) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let json = serde_json::to_string(event).map_err(|_| ())?;
    sender
        .send(Message::Text(json.into()))
        .await
        .map_err(|_| ())
}

fn authorized(headers: &HeaderMap, token: &AuthToken) -> bool {
    let candidate = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, credential) = value.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then_some(credential)
        });
    candidate.is_some_and(|candidate| token.matches(candidate))
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ApiError {
            code: "unauthorized".to_owned(),
            message: "a valid OrangeDeck bearer token is required".to_owned(),
            expected_protocol_version: None,
            received_protocol_version: None,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{HeaderValue, Request},
    };
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tower::ServiceExt;

    use crate::mock::MockBackend;

    use super::*;

    #[test]
    fn bearer_auth_is_required_and_exact() {
        let token = AuthToken::parse("a".repeat(32)).unwrap();
        let empty = HeaderMap::new();
        assert!(!authorized(&empty, &token));

        let mut wrong = HeaderMap::new();
        wrong.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        );
        assert!(!authorized(&wrong, &token));

        let mut correct = HeaderMap::new();
        correct.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        );
        assert!(authorized(&correct, &token));
    }

    #[tokio::test]
    async fn protected_routes_enforce_auth_and_protocol_version() {
        let token = AuthToken::parse("a".repeat(32)).unwrap();
        let app = router(token, Arc::new(MockBackend::new()));
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let request = ClientRequest {
            protocol_version: PROTOCOL_VERSION + 1,
            request_id: uuid::Uuid::new_v4(),
            command: orangedeck_protocol::ClientCommand::RefreshState,
        };
        let mismatch = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/command")
                    .header(AUTHORIZATION, "Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(mismatch.status(), StatusCode::UPGRADE_REQUIRED);
        let body = to_bytes(mismatch.into_body(), 64 * 1024).await.unwrap();
        let error: ApiError = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.code, "protocol_version_mismatch");
    }

    #[tokio::test]
    async fn command_body_is_bounded() {
        let app = router(
            AuthToken::parse("a".repeat(32)).unwrap(),
            Arc::new(MockBackend::new()),
        );
        let oversized = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/command")
                    .header(AUTHORIZATION, "Bearer aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                    .header("content-type", "application/json")
                    .body(Body::from(vec![b'x'; 65 * 1024]))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn authentication_precedes_body_reading_and_websocket_extraction() {
        let app = router(
            AuthToken::parse("a".repeat(32)).unwrap(),
            Arc::new(MockBackend::new()),
        );
        // An unauthenticated slow upload must be rejected without waiting for its body.
        let slow_body = Body::from_stream(futures_util::stream::pending::<
            Result<axum::body::Bytes, std::io::Error>,
        >());
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/command")
            .header("content-type", "application/json")
            .body(slow_body)
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(1), app.clone().oneshot(request))
            .await
            .expect("unauthenticated request must not read its body")
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        for path in ["/api/v1/events", "/api/v1/health", "/api/v1/snapshot"] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        }
    }

    #[tokio::test]
    async fn mock_websocket_delivers_an_initial_snapshot() {
        let token_text = "a".repeat(32);
        let app = router(
            AuthToken::parse(token_text.clone()).unwrap(),
            Arc::new(MockBackend::new()),
        );
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("loopback sockets are blocked by this test sandbox: {error}");
                return;
            }
            Err(error) => panic!("cannot bind integration test listener: {error}"),
        };
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut request = format!("ws://{address}/api/v1/events")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token_text}")).unwrap(),
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let envelope: ServerEnvelope = serde_json::from_str(message.to_text().unwrap()).unwrap();
        assert!(matches!(envelope.event, ServerEvent::Snapshot(_)));
        server.abort();
    }

    #[tokio::test]
    async fn mock_cargo_command_streams_output_and_completion_end_to_end() {
        let token_text = "a".repeat(32);
        let app = router(
            AuthToken::parse(token_text.clone()).unwrap(),
            Arc::new(MockBackend::new()),
        );
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("loopback sockets are blocked by this test sandbox: {error}");
                return;
            }
            Err(error) => panic!("cannot bind integration test listener: {error}"),
        };
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let mut websocket_request = format!("ws://{address}/api/v1/events")
            .into_client_request()
            .unwrap();
        websocket_request.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token_text}")).unwrap(),
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(websocket_request)
            .await
            .unwrap();
        let initial = socket.next().await.unwrap().unwrap();
        let initial: ServerEnvelope = serde_json::from_str(initial.to_text().unwrap()).unwrap();
        assert!(matches!(initial.event, ServerEvent::Snapshot(_)));

        let request = ClientRequest::new(orangedeck_protocol::ClientCommand::RunCargoCheck {
            project_id: "orange-project".to_owned(),
        });
        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/v1/command"))
            .bearer_auth(&token_text)
            .json(&request)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let response: CommandResponse = response.json().await.unwrap();
        let job_id = response.job_id.expect("cargo command returns a job id");

        let events = async {
            let mut saw_started = false;
            let mut saw_output = false;
            loop {
                let message = socket.next().await.unwrap().unwrap();
                let envelope: ServerEnvelope =
                    serde_json::from_str(message.to_text().unwrap()).unwrap();
                match envelope.event {
                    ServerEvent::JobStarted(job) if job.id == job_id => saw_started = true,
                    ServerEvent::JobOutput(output) if output.job_id == job_id => saw_output = true,
                    ServerEvent::JobCompleted(job) if job.id == job_id => {
                        assert_eq!(job.status, orangedeck_protocol::JobStatusDto::Succeeded);
                        assert!(saw_started);
                        assert!(saw_output);
                        break;
                    }
                    _ => {}
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), events)
            .await
            .expect("mock cargo stream completes promptly");
        server.abort();
    }
}
