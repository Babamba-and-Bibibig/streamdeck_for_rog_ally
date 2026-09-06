mod backend;
mod doctor;
mod hook_setup;
mod hooks;
mod mock;
mod paths;
mod server;
mod setup;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use clap::{Parser, Subcommand};
use orangedeck_infra::{
    AgentConfig, AuthToken, BindMode, DEFAULT_PORT, DEMO_TOKEN, default_agent_config_path,
    is_tailscale_ip, resolve_bind_address,
};
use tracing_subscriber::EnvFilter;

use backend::RealBackend;
use mock::MockBackend;
use setup::InitOptions;

#[derive(Parser)]
#[command(
    name = "orangedeck-agent",
    version,
    about = "Secure OrangeDeck host agent"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Codex invokes this hook; only an explicit GUI decision can approve a request.
    CodexHook {
        #[arg(long, default_value_os_t = hooks::default_socket())]
        socket: PathBuf,
    },
    /// Preview the Codex hooks; --install merges them with a private backup.
    CodexHooks {
        #[arg(long)]
        install: bool,
        #[arg(long)]
        hooks_file: Option<PathBuf>,
        /// Codex CLI whose version determines supported hook events.
        #[arg(long, default_value = "codex")]
        codex_binary: PathBuf,
        /// Use the same Agent config as serve, including customized install folders.
        #[arg(long, default_value_os_t = default_agent_config_path())]
        config: PathBuf,
    },
    /// Run the real allow-listed host agent on its Tailscale address.
    Serve {
        #[arg(long, default_value_os_t = default_agent_config_path())]
        config: PathBuf,
    },
    /// Validate local settings and credentials without contacting Codex or Tailscale.
    CheckConfig {
        #[arg(long, default_value_os_t = default_agent_config_path())]
        config: PathBuf,
    },
    /// Run a loopback-only simulated agent for UI development on the Ally.
    Demo {
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
    /// Create a private token, agent config, and transferable pairing bundle.
    Init {
        #[arg(long, default_value_os_t = setup::default_init_config_path())]
        config: PathBuf,
        #[arg(long)]
        project_path: PathBuf,
        #[arg(long, default_value = "main-project")]
        project_id: String,
        #[arg(long, default_value = "Main Project")]
        project_name: String,
        #[arg(long)]
        browser_url: Option<String>,
        #[arg(long, default_value = "MACBOOK")]
        host_name: String,
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        #[arg(long, default_value = "cargo")]
        cargo_binary: PathBuf,
        #[arg(long, default_value = "codex")]
        codex_binary: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Regenerate the 0600 pairing bundle after a Tailscale address change.
    Pairing {
        #[arg(long, default_value_os_t = default_agent_config_path())]
        config: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Diagnose Tailscale, Codex, auth permissions, and registered projects.
    Doctor {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        schema_output: Option<PathBuf>,
        /// Probe read-only thread history and local session token logs without starting a turn.
        #[arg(long)]
        codex_monitor: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::CodexHook { socket } => hooks::run_hook(&socket),
        Command::CodexHooks {
            install,
            hooks_file,
            codex_binary,
            config,
        } => hook_setup::run(install, hooks_file.as_deref(), &codex_binary, &config).await?,
        Command::CheckConfig { config } => {
            let config = AgentConfig::load(&config)?;
            drop(config.project_registry()?);
            drop(AuthToken::load(&config.token_file)?);
            println!(
                "{}",
                serde_json::json!({"valid": true, "codex_binary": config.codex_binary})
            );
        }
        Command::Serve { config } => {
            let paths = paths::AgentPaths::for_config(&config)?;
            let config = AgentConfig::load(&config)?;
            if config.bind_mode != BindMode::Tailscale {
                return Err("real Agent requires bind_mode = \"tailscale\"".into());
            }
            init_tracing(&config.log_level);
            let address = resolve_bind_address(&config).await?;
            if !is_tailscale_ip(address.ip()) {
                return Err(format!(
                    "refusing real agent bind to non-Tailscale address {}; use the demo command for loopback",
                    address.ip()
                )
                .into());
            }
            let token = AuthToken::load(&config.token_file)?;
            let backend = Arc::new(RealBackend::new(config, paths.owned_threads).await?);
            if let Err(error) = backend.enable_hooks(paths.hook_socket).await {
                tracing::warn!(%error, "Codex hook connection unavailable");
            }
            server::serve(address, token, backend).await?;
        }
        Command::Demo { port } => {
            init_tracing("info");
            let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
            let token = AuthToken::parse(DEMO_TOKEN)?;
            let backend = MockBackend::new();
            backend.start_usage_preview();
            server::serve(address, token, Arc::new(backend)).await?;
        }
        Command::Init {
            config,
            project_path,
            project_id,
            project_name,
            browser_url,
            host_name,
            port,
            cargo_binary,
            codex_binary,
            force,
        } => {
            let result = setup::initialize(InitOptions {
                config_path: config,
                project_id,
                project_name,
                project_path,
                browser_url,
                host_name,
                port,
                cargo_binary,
                codex_binary,
                force,
            })
            .await?;
            println!("Agent config: {}", result.config_path.display());
            println!("Tailscale bind: {}:{port}", result.bind_ip);
            println!("Pairing bundle: {}", result.pairing_path.display());
            println!("The pairing bundle contains a secret; transfer it only over Tailscale.");
        }
        Command::Pairing { config, output } => {
            let address = setup::write_pairing(&config, &output).await?;
            println!(
                "Pairing bundle written for Tailscale address {address}: {}",
                output.display()
            );
        }
        Command::Doctor {
            config,
            schema_output,
            codex_monitor,
        } => {
            let healthy =
                doctor::run(config.as_deref(), schema_output.as_deref(), codex_monitor).await;
            if !healthy {
                std::process::exit(2);
            }
        }
    }
    Ok(())
}

fn init_tracing(default_level: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_agent_accepts_only_tailscale_v4() {
        assert!(is_tailscale_ip("100.64.0.2".parse().unwrap()));
        assert!(!is_tailscale_ip("100.1.2.3".parse().unwrap()));
        assert!(!is_tailscale_ip("192.168.0.15".parse().unwrap()));
        assert!(!is_tailscale_ip("0.0.0.0".parse().unwrap()));
    }
}
