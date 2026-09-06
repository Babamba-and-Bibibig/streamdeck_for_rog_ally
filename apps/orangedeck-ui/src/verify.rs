//! A bounded acceptance check using the same transport and state reducer as the UI.
//! Default operation is read-only. Fixed Cargo/Codex checks require explicit CLI flags.

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use orangedeck_infra::{AuthToken, UiConfig};
use orangedeck_protocol::{
    ClientCommand, CodexConnectionStateDto, CodexThreadStatusDto, JobStatusDto, PROTOCOL_VERSION,
    ServerEnvelope, ServerEvent, SnapshotDto, ThreadOwnershipDto,
};
use tokio::time::timeout;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

use crate::{
    doctor,
    model::{NetworkEvent, UiModel},
    network,
};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub async fn run(
    config: UiConfig,
    token: AuthToken,
    project_id: Option<String>,
    cargo_check: bool,
    codex_prompt: bool,
) -> Result<(), String> {
    println!(
        "OrangeDeck real connection verification {}",
        env!("CARGO_PKG_VERSION")
    );
    println!("Agent URL: {}", config.agent_url);
    let client = network::http_client().map_err(|error| error.to_string())?;
    let (mut socket, latency_ms) =
        network::connect(&client, &config.agent_url, &token, "real").await?;
    let snapshot = doctor::fetch_snapshot(&client, &config, &token).await?;
    doctor::validate_snapshot(&snapshot, &config, false)?;
    doctor::print_snapshot_summary(&snapshot);
    println!("HTTP authentication / real mode / protocol: PASS ({latency_ms}ms)");
    let mut model = UiModel::default();
    model.apply_network(NetworkEvent::Connected { latency_ms });
    expect_snapshot(&mut socket, &mut model, &config).await?;
    loop {
        if matches!(
            next_event(&mut socket, &mut model).await?,
            ServerEvent::Heartbeat { .. }
        ) {
            break;
        }
    }
    println!("WebSocket snapshot / heartbeat / UI state: PASS");

    if cargo_check || codex_prompt {
        let project_id = project_id.ok_or("an explicit --project-id is required")?;
        if !snapshot
            .projects
            .iter()
            .any(|project| project.id == project_id)
        {
            return Err("requested project is not registered on this Agent".to_owned());
        }
        if cargo_check {
            timeout(
                Duration::from_mins(10),
                check_cargo(
                    &client,
                    &config,
                    &token,
                    &mut socket,
                    &mut model,
                    &snapshot,
                    &project_id,
                ),
            )
            .await
            .map_err(|_| "Cargo check timed out; inspect or cancel the job from the UI")??;
        }
        if codex_prompt {
            timeout(
                Duration::from_mins(3),
                check_codex(
                    &client,
                    &config,
                    &token,
                    &mut socket,
                    &mut model,
                    &snapshot,
                    &project_id,
                ),
            )
            .await
            .map_err(|_| "Codex check timed out; inspect the owned thread from the UI")??;
        }
    }

    socket
        .close(None)
        .await
        .map_err(|error| error.to_string())?;
    model.apply_network(NetworkEvent::Disconnected {
        message: "verification reconnect".to_owned(),
        retry_ms: 0,
    });
    if model.connected {
        return Err("UI did not enter disconnected state".to_owned());
    }
    let (mut socket, latency_ms) =
        network::connect(&client, &config.agent_url, &token, "real").await?;
    model.apply_network(NetworkEvent::Connected { latency_ms });
    expect_snapshot(&mut socket, &mut model, &config).await?;
    socket
        .close(None)
        .await
        .map_err(|error| error.to_string())?;
    println!("WebSocket close / reconnect / fresh snapshot: PASS");
    println!("Overall: PASS");
    Ok(())
}

async fn next_event(socket: &mut Socket, model: &mut UiModel) -> Result<ServerEvent, String> {
    loop {
        let message = timeout(Duration::from_secs(35), socket.next())
            .await
            .map_err(|_| "Agent event stream timed out")?
            .ok_or("Agent event stream closed")?
            .map_err(|error| error.to_string())?;
        match message {
            Message::Text(text) => {
                let envelope: ServerEnvelope =
                    serde_json::from_str(&text).map_err(|error| error.to_string())?;
                if envelope.protocol_version != PROTOCOL_VERSION {
                    return Err("event protocol mismatch".to_owned());
                }
                let event = envelope.event.clone();
                model.apply_network(NetworkEvent::Server(envelope));
                return Ok(event);
            }
            Message::Close(_) => return Err("Agent closed its event stream".to_owned()),
            _ => {}
        }
    }
}

async fn expect_snapshot(
    socket: &mut Socket,
    model: &mut UiModel,
    config: &UiConfig,
) -> Result<(), String> {
    match next_event(socket, model).await? {
        ServerEvent::Snapshot(snapshot) => doctor::validate_snapshot(&snapshot, config, false),
        _ => Err("first WebSocket event must be a snapshot".to_owned()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn check_cargo(
    client: &reqwest::Client,
    config: &UiConfig,
    token: &AuthToken,
    socket: &mut Socket,
    model: &mut UiModel,
    snapshot: &SnapshotDto,
    project_id: &str,
) -> Result<(), String> {
    if snapshot
        .jobs
        .iter()
        .any(|job| matches!(job.status, JobStatusDto::Running | JobStatusDto::Queued))
    {
        return Err("a Cargo job is already active; verification did not start another".to_owned());
    }
    let response = network::execute(
        client,
        &config.agent_url,
        token,
        ClientCommand::RunCargoCheck {
            project_id: project_id.to_owned(),
        },
    )
    .await?;
    let job_id = response.job_id.ok_or("Cargo command returned no job id")?;
    println!("Cargo Check accepted: {job_id}");
    let mut started = false;
    let mut output_lines = 0_u64;
    let mut completed = false;
    loop {
        match next_event(socket, model).await? {
            ServerEvent::JobStarted(job) if job.id == job_id => {
                started = job.status == JobStatusDto::Running;
                println!("Cargo: RUNNING");
            }
            ServerEvent::JobOutput(output) if output.job_id == job_id => {
                output_lines += 1;
                if output_lines <= 5 || output_lines.is_multiple_of(50) {
                    println!("Cargo output {output_lines}: {}", output.line);
                }
            }
            ServerEvent::JobCompleted(job) if job.id == job_id => {
                println!(
                    "Cargo: {:?}, exit {:?}, {}ms, {output_lines} streamed lines",
                    job.status,
                    job.exit_code,
                    job.duration_ms.unwrap_or_default()
                );
                if job.status != JobStatusDto::Succeeded {
                    for line in job.output_tail.iter().rev().take(12).rev() {
                        println!("  {line}");
                    }
                    return Err("real Cargo Check failed".to_owned());
                }
                if !started || output_lines == 0 || job.duration_ms.is_none() {
                    return Err("Cargo lifecycle or streamed output was incomplete".to_owned());
                }
                let rendered = model
                    .snapshot
                    .as_ref()
                    .and_then(|state| state.jobs.iter().find(|job| job.id == job_id));
                if !rendered.is_some_and(|job| {
                    job.status == JobStatusDto::Succeeded && !job.output_tail.is_empty()
                }) {
                    return Err("UI state did not retain the completed Cargo output".to_owned());
                }
                completed = true;
            }
            ServerEvent::Notification(notification)
                if completed && notification.title == "Cargo Check finished" =>
            {
                println!("Cargo completion notification / UI PASS state: PASS");
                return Ok(());
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn check_codex(
    client: &reqwest::Client,
    config: &UiConfig,
    token: &AuthToken,
    socket: &mut Socket,
    model: &mut UiModel,
    snapshot: &SnapshotDto,
    project_id: &str,
) -> Result<(), String> {
    if snapshot.codex.connection.state != CodexConnectionStateDto::Connected {
        return Err("Mac Codex is not connected".to_owned());
    }
    if !snapshot.codex.pending_approvals.is_empty() {
        return Err("resolve existing approvals manually before the Codex check".to_owned());
    }
    let existing: HashSet<_> = snapshot
        .codex
        .threads
        .iter()
        .map(|thread| thread.id.clone())
        .collect();
    network::execute(
        client,
        &config.agent_url,
        token,
        ClientCommand::CodexStartThread {
            project_id: project_id.to_owned(),
        },
    )
    .await?;
    let thread_id = loop {
        if let ServerEvent::CodexThreadUpdated(thread) = next_event(socket, model).await?
            && thread.ownership == ThreadOwnershipDto::OrangeDeck
            && thread.project_id.as_deref() == Some(project_id)
            && !existing.contains(&thread.id)
        {
            break thread.id;
        }
    };
    println!("Codex: created OrangeDeck-owned thread {thread_id}");
    network::execute(client, &config.agent_url, token, ClientCommand::CodexSendPrompt {
        thread_id: thread_id.clone(),
        prompt: "Reply with exactly ORANGEDECK_OK. Do not use tools, run commands, read files, or create, modify, or delete any files.".to_owned(),
    }).await?;
    let mut working = false;
    let mut reply = String::new();
    let mut completed = false;
    loop {
        match next_event(socket, model).await? {
            ServerEvent::CodexTurnUpdated(turn) if turn.thread_id == thread_id => {
                println!("Codex: {:?}", turn.status);
                match turn.status {
                    CodexThreadStatusDto::Working => working = true,
                    CodexThreadStatusDto::Completed => {
                        if !working
                            || reply.trim() != "ORANGEDECK_OK"
                            || model.codex_reply(&thread_id).map(str::trim) != Some("ORANGEDECK_OK")
                        {
                            return Err("Codex completed without the expected lifecycle and reply"
                                .to_owned());
                        }
                        completed = true;
                    }
                    CodexThreadStatusDto::Error => {
                        return Err("Codex turn failed; inspect its state in the UI".to_owned());
                    }
                    _ => {}
                }
            }
            ServerEvent::CodexActivity(activity)
                if activity.thread_id.as_deref() == Some(&thread_id)
                    && activity.kind == "message" =>
            {
                if reply.len() + activity.text.len() > 4096 {
                    return Err("Codex reply exceeded the verification limit".to_owned());
                }
                reply.push_str(&activity.text);
            }
            ServerEvent::CodexApprovalRequested(approval)
                if approval.thread_id.as_deref() == Some(&thread_id) =>
            {
                return Err(
                    "Codex requested approval; use the UI to decide. Nothing was auto-approved."
                        .to_owned(),
                );
            }
            ServerEvent::Notification(notification)
                if completed
                    && (notification.title == "Codex turn finished"
                        || notification.title == "Codex 응답 완료")
                    && (notification.thread_id.as_deref() == Some(&thread_id)
                        || notification.body.contains(&thread_id)) =>
            {
                println!("Codex reply ORANGEDECK_OK / completion notification: PASS");
                return Ok(());
            }
            _ => {}
        }
    }
}
