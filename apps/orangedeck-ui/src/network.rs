use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use eframe::egui;
use futures_util::StreamExt;
use orangedeck_application::ReconnectBackoff;
use orangedeck_infra::AuthToken;
use orangedeck_protocol::{
    ApiError, ClientCommand, ClientRequest, CommandResponse, HealthResponse, PROTOCOL_VERSION,
    ServerEnvelope,
};
use reqwest::StatusCode;
use tokio::sync::mpsc as tokio_mpsc;
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

use crate::model::NetworkEvent;

pub struct NetworkHandle {
    commands: tokio_mpsc::UnboundedSender<ClientCommand>,
    events: mpsc::Receiver<NetworkEvent>,
}

impl NetworkHandle {
    #[cfg(test)]
    pub fn for_test() -> (Self, tokio_mpsc::UnboundedReceiver<ClientCommand>) {
        let (commands, receiver) = tokio_mpsc::unbounded_channel();
        let (_, events) = mpsc::channel();
        (Self { commands, events }, receiver)
    }
    pub fn start(
        connector_url: String,
        token: AuthToken,
        expected_mode: &'static str,
        repaint: egui::Context,
    ) -> Result<Self, String> {
        let (commands, command_rx) = tokio_mpsc::unbounded_channel();
        let (event_tx, events) = mpsc::channel();
        thread::Builder::new()
            .name("orangedeck-network".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(connection_loop(
                        connector_url,
                        token,
                        expected_mode,
                        command_rx,
                        event_tx,
                        repaint,
                    )),
                    Err(error) => {
                        let _ = event_tx.send(NetworkEvent::Disconnected {
                            message: format!("Cannot start network runtime: {error}"),
                            retry_ms: 0,
                        });
                    }
                }
            })
            .map_err(|error| format!("cannot start network thread: {error}"))?;
        Ok(Self { commands, events })
    }

    pub fn send(&self, command: ClientCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "OrangeDeck network worker has stopped".to_owned())
    }

    pub fn drain(&self) -> Vec<NetworkEvent> {
        self.events.try_iter().collect()
    }
}

async fn connection_loop(
    connector_url: String,
    token: AuthToken,
    expected_mode: &'static str,
    mut commands: tokio_mpsc::UnboundedReceiver<ClientCommand>,
    events: mpsc::Sender<NetworkEvent>,
    repaint: egui::Context,
) {
    let client = match http_client() {
        Ok(client) => client,
        Err(error) => {
            emit(
                &events,
                &repaint,
                NetworkEvent::Disconnected {
                    message: format!("Cannot construct HTTP client: {error}"),
                    retry_ms: 0,
                },
            );
            return;
        }
    };
    let mut backoff = ReconnectBackoff::default();
    loop {
        if !reject_pending_commands(&mut commands, &events, &repaint) {
            return;
        }
        emit(&events, &repaint, NetworkEvent::Connecting);
        match connect(&client, &connector_url, &token, expected_mode).await {
            Ok((mut socket, latency_ms)) => {
                if !reject_pending_commands(&mut commands, &events, &repaint) {
                    return;
                }
                backoff.reset();
                emit(&events, &repaint, NetworkEvent::Connected { latency_ms });
                let disconnect_message = loop {
                    tokio::select! {
                        message = tokio::time::timeout(Duration::from_secs(35), socket.next()) => match message {
                            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                                match serde_json::from_str::<ServerEnvelope>(&text) {
                                    Ok(envelope) => emit(&events, &repaint, NetworkEvent::Server(envelope)),
                                    Err(error) => emit(
                                        &events,
                                        &repaint,
                                        NetworkEvent::CommandCompleted(Err(format!("Invalid Connector event: {error}"))),
                                    ),
                                }
                            }
                            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))) => {
                                break "Connector event stream closed".to_owned();
                            }
                            Ok(Some(Err(error))) => {
                                break format!("Connector event stream failed: {error}");
                            }
                            Ok(None) => break "Connector event stream ended".to_owned(),
                            Err(_) => break "Connector heartbeat timed out".to_owned(),
                            Ok(Some(Ok(_))) => {}
                        },
                        command = commands.recv() => match command {
                            Some(command) => {
                                let command_client = client.clone();
                                let command_url = connector_url.clone();
                                let command_token = token.clone();
                                let command_events = events.clone();
                                let command_repaint = repaint.clone();
                                tokio::spawn(async move {
                                    let approval_id = match &command {
                                        ClientCommand::CodexApprovalResponse { approval_id, .. } => Some(*approval_id),
                                        _ => None,
                                    };
                                    let navigation_id = match &command {
                                        ClientCommand::OpenCodexChange { navigation_id, .. }
                                        | ClientCommand::RegisterCodexProject { navigation_id, .. } => Some(*navigation_id),
                                        _ => None,
                                    };
                                    let result = execute(
                                        &command_client,
                                        &command_url,
                                        &command_token,
                                        command,
                                    )
                                    .await;
                                    emit(
                                        &command_events,
                                        &command_repaint,
                                        navigation_id.map_or_else(|| approval_id.map_or_else(
                                            || NetworkEvent::CommandCompleted(result.clone()),
                                            |approval_id| NetworkEvent::ApprovalCompleted { approval_id, result: result.clone() },
                                        ), |navigation_id| NetworkEvent::FileOpenCompleted { navigation_id, result: result.clone() }),
                                    );
                                });
                            }
                            None => return,
                        }
                    }
                };
                emit(
                    &events,
                    &repaint,
                    NetworkEvent::Disconnected {
                        message: disconnect_message,
                        retry_ms: 0,
                    },
                );
            }
            Err(error) => {
                let delay = backoff.next_delay();
                emit(
                    &events,
                    &repaint,
                    NetworkEvent::Disconnected {
                        message: error,
                        retry_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                    },
                );
                let retry_sleep = tokio::time::sleep(delay);
                tokio::pin!(retry_sleep);
                loop {
                    tokio::select! {
                        () = &mut retry_sleep => break,
                        command = commands.recv() => match command {
                            Some(command) => emit_command_rejected(&events, &repaint, &command),
                            None => return,
                        }
                    }
                }
            }
        }
    }
}

fn reject_pending_commands(
    commands: &mut tokio_mpsc::UnboundedReceiver<ClientCommand>,
    events: &mpsc::Sender<NetworkEvent>,
    repaint: &egui::Context,
) -> bool {
    loop {
        match commands.try_recv() {
            Ok(command) => emit_command_rejected(events, repaint, &command),
            Err(tokio_mpsc::error::TryRecvError::Empty) => return true,
            Err(tokio_mpsc::error::TryRecvError::Disconnected) => return false,
        }
    }
}

fn emit_command_rejected(
    events: &mpsc::Sender<NetworkEvent>,
    repaint: &egui::Context,
    command: &ClientCommand,
) {
    let result = Err("통신이 끊겨 요청을 보내지 못했습니다. 다시 연결한 뒤 재시도하세요 / Connector is disconnected; command was not sent".to_owned());
    let event = match command {
        ClientCommand::OpenCodexChange { navigation_id, .. }
        | ClientCommand::RegisterCodexProject { navigation_id, .. } => {
            NetworkEvent::FileOpenCompleted {
                navigation_id: *navigation_id,
                result,
            }
        }
        ClientCommand::CodexApprovalResponse { approval_id, .. } => {
            NetworkEvent::ApprovalCompleted {
                approval_id: *approval_id,
                result,
            }
        }
        _ => NetworkEvent::CommandCompleted(result),
    };
    emit(events, repaint, event);
}

pub(crate) fn http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        // Pairing credentials belong only in the direct encrypted tailnet connection.
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(45))
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

pub(crate) async fn connect(
    client: &reqwest::Client,
    connector_url: &str,
    token: &AuthToken,
    expected_mode: &str,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        u64,
    ),
    String,
> {
    let started = Instant::now();
    let health = client
        .get(format!("{connector_url}/api/v1/health"))
        .timeout(Duration::from_secs(5))
        .bearer_auth(token.expose())
        .send()
        .await
        .map_err(|error| format!("Connector health check failed: {error}"))?;
    if health.status() == StatusCode::UNAUTHORIZED {
        return Err("Connector rejected the pairing token".to_owned());
    }
    if !health.status().is_success() {
        return Err(format!(
            "Connector health check returned {}",
            health.status()
        ));
    }
    let health = health
        .json::<HealthResponse>()
        .await
        .map_err(|error| format!("Invalid Connector health response: {error}"))?;
    if health.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "Protocol mismatch: UI {PROTOCOL_VERSION}, Connector {}",
            health.protocol_version
        ));
    }
    validate_connector_mode(&health, expected_mode)?;
    let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let websocket_url = if let Some(rest) = connector_url.strip_prefix("https://") {
        format!("wss://{rest}/api/v1/events")
    } else if let Some(rest) = connector_url.strip_prefix("http://") {
        format!("ws://{rest}/api/v1/events")
    } else {
        return Err("Connector URL must start with http:// or https://".to_owned());
    };
    let mut request = websocket_url
        .into_client_request()
        .map_err(|error| format!("Invalid WebSocket URL: {error}"))?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", token.expose())
            .parse()
            .map_err(|_| "Invalid pairing token header".to_owned())?,
    );
    let (socket, _) = tokio::time::timeout(Duration::from_secs(5), connect_async(request))
        .await
        .map_err(|_| "Connector event connection timed out".to_owned())?
        .map_err(|error| format!("Connector event connection failed: {error}"))?;
    Ok((socket, elapsed))
}

fn validate_connector_mode(health: &HealthResponse, expected_mode: &str) -> Result<(), String> {
    if !health.ready {
        return Err("Connector is not ready".to_owned());
    }
    if health.mode != expected_mode {
        return Err(format!(
            "Connector mode mismatch: UI requires {expected_mode}, endpoint reports {}",
            health.mode
        ));
    }
    Ok(())
}

pub(crate) async fn execute(
    client: &reqwest::Client,
    connector_url: &str,
    token: &AuthToken,
    command: ClientCommand,
) -> Result<CommandResponse, String> {
    let request = ClientRequest::new(command);
    let response = client
        .post(format!("{connector_url}/api/v1/command"))
        .bearer_auth(token.expose())
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Command request failed: {error}"))?;
    if response.status().is_success() {
        let response = response
            .json::<CommandResponse>()
            .await
            .map_err(|error| format!("Invalid command response: {error}"))?;
        if response.protocol_version != PROTOCOL_VERSION
            || response.request_id != request.request_id
            || !response.accepted
        {
            return Err(
                "Connector did not acknowledge this command with the expected protocol".to_owned(),
            );
        }
        Ok(response)
    } else {
        let status = response.status();
        let error = response.json::<ApiError>().await.ok();
        Err(error.map_or_else(
            || format!("Connector rejected command with {status}"),
            |error| format!("{}: {}", error.code, error.message),
        ))
    }
}

fn emit(events: &mpsc::Sender<NetworkEvent>, repaint: &egui::Context, event: NetworkEvent) {
    let _ = events.send(event);
    repaint.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn health(mode: &str) -> HealthResponse {
        HealthResponse {
            name: "OrangeDeck Connector".to_owned(),
            version: "0.1.0".to_owned(),
            protocol_version: PROTOCOL_VERSION,
            ready: true,
            mode: mode.to_owned(),
        }
    }

    #[test]
    fn real_and_mock_endpoints_cannot_be_confused() {
        assert!(validate_connector_mode(&health("real"), "real").is_ok());
        assert!(validate_connector_mode(&health("mock"), "mock").is_ok());
        assert!(validate_connector_mode(&health("mock"), "real").is_err());
        assert!(validate_connector_mode(&health("real"), "mock").is_err());
    }

    #[test]
    fn pending_commands_are_rejected_instead_of_replayed_after_reconnect() {
        let (command_tx, mut command_rx) = tokio_mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::channel();
        let repaint = egui::Context::default();
        command_tx.send(ClientCommand::RefreshState).unwrap();

        assert!(reject_pending_commands(
            &mut command_rx,
            &event_tx,
            &repaint
        ));
        assert!(matches!(
            event_rx.recv().unwrap(),
            NetworkEvent::CommandCompleted(Err(message))
                if message.contains("was not sent")
        ));
        assert!(matches!(
            command_rx.try_recv(),
            Err(tokio_mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn disconnected_editor_requests_complete_the_exact_request_instead_of_leaving_it_busy() {
        for register in [false, true] {
            let id = uuid::Uuid::new_v4();
            let command = if register {
                ClientCommand::RegisterCodexProject {
                    navigation_id: id,
                    thread_id: "thread".into(),
                    turn_id: "turn".into(),
                    expected_cwd: "/project".into(),
                    path: "new.rs".into(),
                }
            } else {
                ClientCommand::OpenCodexChange {
                    navigation_id: id,
                    thread_id: "thread".into(),
                    turn_id: "turn".into(),
                    path: "new.rs".into(),
                }
            };
            let (tx, mut rx) = tokio_mpsc::unbounded_channel();
            let (events, received) = mpsc::channel();
            tx.send(command).unwrap();
            assert!(reject_pending_commands(
                &mut rx,
                &events,
                &egui::Context::default()
            ));
            assert!(
                matches!(received.recv().unwrap(), NetworkEvent::FileOpenCompleted { navigation_id, result: Err(_) } if navigation_id == id)
            );
            assert!(rx.try_recv().is_err());
        }
    }
}
