use std::{
    path::Path,
    time::{Duration, Instant},
};

use orangedeck_infra::{AuthToken, UiConfig, command_stdout, tailscale_stdout};
use orangedeck_protocol::{HealthResponse, SnapshotDto};

pub async fn run(config_path: Option<&Path>, demo: bool, workspaces: bool) -> bool {
    println!("OrangeDeck UI Doctor {}", env!("CARGO_PKG_VERSION"));
    println!("OS: {}", std::env::consts::OS);
    println!("Architecture: {}", std::env::consts::ARCH);
    println!(
        "Session: {}",
        std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_owned())
    );
    println!(
        "Wayland display: {}",
        std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "not set".to_owned())
    );
    let display = command_stdout("kscreen-doctor", &["-o"]).await;
    println!(
        "Display probe: {}",
        display
            .as_deref()
            .and_then(|value| value.lines().next())
            .unwrap_or("UNAVAILABLE")
    );

    match gilrs::Gilrs::new() {
        Ok(gilrs) => {
            let controllers = gilrs
                .gamepads()
                .map(|(_, gamepad)| gamepad.name().to_owned())
                .collect::<Vec<_>>();
            println!("Controller count: {}", controllers.len());
            for controller in controllers {
                println!("  {controller}");
            }
        }
        Err(error) => println!("Controller probe: ERROR ({error})"),
    }

    let tailscale_version = tailscale_stdout(&["version"]).await;
    let tailscale_ip = tailscale_stdout(&["ip", "-4"]).await;
    println!(
        "Tailscale: {} / {}",
        tailscale_version.as_deref().unwrap_or("NOT FOUND"),
        tailscale_ip.as_deref().unwrap_or("NO ADDRESS")
    );
    let online = orangedeck_infra::tailscale_command()
        .args(["status", "--json"])
        .output()
        .await
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok())
        .and_then(|value| {
            value
                .pointer("/Self/Online")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false);
    println!("Tailscale online: {online}");

    let (config, token) = if demo {
        (
            UiConfig::demo(),
            AuthToken::parse(orangedeck_infra::DEMO_TOKEN).ok(),
        )
    } else if let Some(path) = config_path {
        match UiConfig::load(path) {
            Ok(config) => {
                let token = match AuthToken::load(&config.token_file) {
                    Ok(token) => {
                        println!("Pairing token: OK (redacted, private permissions)");
                        Some(token)
                    }
                    Err(error) => {
                        println!("Pairing token: ERROR ({error})");
                        None
                    }
                };
                println!("Config: OK ({})", path.display());
                (config, token)
            }
            Err(error) => {
                println!("Config: ERROR ({error})");
                (UiConfig::default(), None)
            }
        }
    } else {
        println!("Config: not checked");
        (UiConfig::default(), None)
    };

    let mut reachable = false;
    let mut snapshot_ready = false;
    if let Some(token) = token {
        println!("Agent URL: {}", config.agent_url);
        let client = match crate::network::http_client() {
            Ok(client) => client,
            Err(error) => {
                println!("HTTP client: ERROR ({error})");
                return false;
            }
        };
        let started = Instant::now();
        match client
            .get(format!("{}/api/v1/health", config.agent_url))
            .timeout(Duration::from_secs(5))
            .bearer_auth(token.expose())
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                match response.json::<HealthResponse>().await {
                    Ok(health) => {
                        let expected_mode = if demo { "mock" } else { "real" };
                        reachable = health.ready
                            && health.protocol_version == orangedeck_protocol::PROTOCOL_VERSION
                            && health.mode == expected_mode;
                        println!(
                            "Agent: {} {} / mode {} / protocol {} / {}ms",
                            health.name,
                            health.version,
                            health.mode,
                            health.protocol_version,
                            started.elapsed().as_millis()
                        );
                        if health.mode != expected_mode {
                            println!(
                                "Agent mode: ERROR (expected {expected_mode}, received {})",
                                health.mode
                            );
                        }
                    }
                    Err(error) => println!("Agent: invalid response ({error})"),
                }
            }
            Ok(response) => println!("Agent: HTTP {}", response.status()),
            Err(error) => println!("Agent: UNREACHABLE ({error})"),
        }
        if reachable {
            match fetch_snapshot(&client, &config, &token).await {
                Ok(snapshot) => {
                    print_snapshot_summary(&snapshot);
                    if workspaces {
                        print_workspaces(&snapshot);
                    }
                    match validate_snapshot(&snapshot, &config, demo) {
                        Ok(()) => snapshot_ready = true,
                        Err(error) => println!("Snapshot: ERROR ({error})"),
                    }
                }
                Err(error) => println!("Snapshot: ERROR ({error})"),
            }
        }
    }
    let healthy = reachable && snapshot_ready && (demo || online);
    println!("Overall: {}", if healthy { "OK" } else { "NOT READY" });
    healthy
}

pub(crate) async fn fetch_snapshot(
    client: &reqwest::Client,
    config: &UiConfig,
    token: &AuthToken,
) -> Result<SnapshotDto, String> {
    client
        .get(format!("{}/api/v1/snapshot", config.agent_url))
        .timeout(Duration::from_secs(5))
        .bearer_auth(token.expose())
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("snapshot request failed: {error}"))?
        .json::<SnapshotDto>()
        .await
        .map_err(|error| format!("invalid snapshot: {error}"))
}

pub(crate) fn validate_snapshot(
    snapshot: &SnapshotDto,
    config: &UiConfig,
    demo: bool,
) -> Result<(), String> {
    if !demo {
        let url = reqwest::Url::parse(&config.agent_url).map_err(|error| error.to_string())?;
        if snapshot.host.name != config.host_label
            || !snapshot.host.tailscale
            || snapshot.host.address.as_deref() != url.host_str()
        {
            return Err("host identity does not match the paired Tailscale host".to_owned());
        }
    }
    if !snapshot
        .projects
        .iter()
        .any(|project| Some(project.id.as_str()) == snapshot.selected_project_id.as_deref())
    {
        return Err("Agent has no selected registered project".to_owned());
    }
    Ok(())
}

pub(crate) fn print_snapshot_summary(snapshot: &SnapshotDto) {
    println!(
        "Host: {} / tailscale {} / address {}",
        snapshot.host.name,
        snapshot.host.tailscale,
        snapshot.host.address.as_deref().unwrap_or("UNAVAILABLE")
    );
    println!("Projects visible: {}", snapshot.projects.len());
    for project in &snapshot.projects {
        println!("  {}: {}", project.id, project.path);
    }
    println!(
        "Codex: {:?} / version {} / threads {} / approvals {}",
        snapshot.codex.connection.state,
        snapshot
            .codex
            .connection
            .version
            .as_deref()
            .unwrap_or("UNAVAILABLE"),
        snapshot.codex.threads.len(),
        snapshot.codex.pending_approvals.len()
    );
    if let Some(message) = &snapshot.codex.connection.message {
        println!("Codex compatibility: {message}");
    }
    if let Some(project_id) = snapshot.selected_project_id.as_deref()
        && let Some(git) = snapshot
            .git
            .iter()
            .find(|state| state.project_id == project_id)
    {
        println!(
            "Git: {} / modified {} / staged {} / untracked {}",
            git.branch, git.counts.modified, git.counts.staged, git.counts.untracked
        );
        if let Some(error) = &git.error {
            println!("Git unavailable: {error}");
        }
    }
}

fn print_workspaces(snapshot: &SnapshotDto) {
    let mut paths = std::collections::BTreeMap::<&str, (usize, usize)>::new();
    for thread in &snapshot.codex.threads {
        let counts = paths.entry(&thread.cwd).or_default();
        counts.0 += 1;
        counts.1 +=
            usize::from(thread.ownership == orangedeck_protocol::ThreadOwnershipDto::OrangeDeck);
    }
    println!(
        "Conversation workspace source: {} / {}",
        snapshot.host.name,
        snapshot.host.address.as_deref().unwrap_or("UNAVAILABLE")
    );
    println!(
        "Unique recorded paths: {}; not a disk scan or a list of running processes",
        paths.len()
    );
    for (path, (count, managed)) in paths {
        println!("  {count} conversation(s), {managed} OrangeDeck-owned: {path}");
    }
}
