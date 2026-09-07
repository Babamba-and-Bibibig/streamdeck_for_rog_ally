use std::path::Path;

use orangedeck_infra::{
    AuthToken, ConnectorConfig, generate_codex_schema, is_tailscale_ip, probe_codex,
    tailscale_stdout,
};
use tokio::process::Command;

pub async fn run(
    config_path: Option<&Path>,
    schema_output: Option<&Path>,
    codex_monitor: bool,
) -> bool {
    println!("OrangeDeck Connector Doctor {}", env!("CARGO_PKG_VERSION"));
    println!("OS: {}", std::env::consts::OS);
    println!("Architecture: {}", std::env::consts::ARCH);

    let tailscale_version = tailscale_stdout(&["version"]).await;
    println!(
        "Tailscale executable: {}",
        tailscale_version.as_deref().unwrap_or("NOT FOUND")
    );
    let tailscale_ip = tailscale_stdout(&["ip", "-4"]).await;
    println!(
        "Tailscale IPv4: {}",
        tailscale_ip.as_deref().unwrap_or("UNAVAILABLE")
    );
    let tailscale_online = orangedeck_infra::tailscale_command()
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
    println!("Tailscale online: {tailscale_online}");

    let (config, config_ready) = if let Some(path) = config_path {
        match ConnectorConfig::load(path) {
            Ok(config) => {
                println!("Config: OK ({})", path.display());
                println!("Bind mode: {:?}", config.bind_mode);
                println!("Registered projects: {}", config.projects.len());
                for project in &config.projects {
                    println!("  {} -> {}", project.id, project.path.display());
                }
                let token_ready = match AuthToken::load(&config.token_file) {
                    Ok(_) => {
                        println!("Connector token: OK (redacted, private permissions)");
                        true
                    }
                    Err(error) => {
                        println!("Connector token: ERROR ({error})");
                        false
                    }
                };
                (config, token_ready)
            }
            Err(error) => {
                println!("Config: ERROR ({error})");
                (ConnectorConfig::default(), false)
            }
        }
    } else {
        println!("Config: not checked");
        (ConnectorConfig::default(), true)
    };

    let codex = probe_codex(&config.codex_binary).await;
    println!("Codex executable: {}", codex.executable.display());
    println!(
        "Codex version: {}",
        codex.version.as_deref().unwrap_or("UNAVAILABLE")
    );
    println!("Codex app-server: {}", status(codex.app_server_supported));
    println!(
        "Codex schema generation: {}",
        status(codex.schema_generation_supported)
    );
    let login_ok = Command::new(&config.codex_binary)
        .args(["login", "status"])
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    println!("Codex login: {}", status(login_ok));
    if let Some(output) = schema_output {
        match generate_codex_schema(&config.codex_binary, output).await {
            Ok(()) => println!("Codex schema snapshot: {}", output.display()),
            Err(error) => println!("Codex schema snapshot: ERROR ({error})"),
        }
    }

    let monitor_ready = !codex_monitor || probe_monitor(&config.codex_binary).await;

    let tailscale_address_ready = tailscale_ip
        .as_deref()
        .and_then(|value| value.parse().ok())
        .is_some_and(is_tailscale_ip);
    let healthy = tailscale_version.is_some()
        && tailscale_address_ready
        && tailscale_online
        && codex.app_server_supported
        && login_ok
        && monitor_ready
        && config_ready;
    println!("Overall: {}", status(healthy));
    healthy
}

async fn probe_monitor(executable: &Path) -> bool {
    // Read-only methods never persist this non-existing owned-thread registry.
    let registry =
        std::env::temp_dir().join(format!("orangedeck-readonly-{}.json", uuid::Uuid::new_v4()));
    let Ok(client) = orangedeck_infra::CodexClient::spawn(executable, registry, Vec::new()).await
    else {
        println!("Read-only monitor: Codex connection failed");
        return false;
    };
    let readable = if let Ok(threads) = client.refresh_threads().await {
        println!("Monitor threads: {} (contents redacted)", threads.len());
        let observed = threads
            .iter()
            .filter_map(|thread| thread.live_usage.as_ref())
            .collect::<Vec<_>>();
        println!(
            "Session token logs readable: {} (local host only; not account totals)",
            observed.len()
        );
        if let Some(usage) = observed.iter().max_by_key(|usage| usage.updated_at) {
            println!(
                "Latest token record: {} / last model request {} / current turn {} / recorded state {:?}",
                usage.updated_at,
                usage.last_request.total_tokens,
                usage.turn_tokens.as_ref().map_or_else(
                    || "unavailable".to_owned(),
                    |value| value.total_tokens.to_string()
                ),
                usage.status
            );
        }
        if let Some(thread) = threads.first() {
            if let Ok(thread) = client.read_thread(&thread.id).await {
                println!(
                    "Latest user question available: {}",
                    thread
                        .observation
                        .as_ref()
                        .is_some_and(|value| value.latest_user_prompt.is_some())
                );
                true
            } else {
                println!("Read-only history: unavailable");
                false
            }
        } else {
            true
        }
    } else {
        println!("Monitor thread list: unavailable");
        false
    };
    client.shutdown().await;
    readable
}

const fn status(ok: bool) -> &'static str {
    if ok { "OK" } else { "NOT READY" }
}
