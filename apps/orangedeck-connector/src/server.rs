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
pub trait ConnectorBackend: Send + Sync {
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
    backend: Arc<dyn ConnectorBackend>,
}

pub fn router(token: AuthToken, backend: Arc<dyn ConnectorBackend>) -> Router {
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
    backend: Arc<dyn ConnectorBackend>,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(address).await?;
    info!(%address, mode = backend.mode(), "OrangeDeck Connector listening");
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
        name: "OrangeDeck Connector".to_owned(),
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

async fn websocket_loop(socket: WebSocket, backend: Arc<dyn ConnectorBackend>) {
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
