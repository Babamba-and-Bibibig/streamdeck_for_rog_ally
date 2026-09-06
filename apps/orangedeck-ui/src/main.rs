mod alerts;
mod app;
mod controller;
mod doctor;
mod input_backend;
mod model;
mod monitor;
mod network;
mod notifications;
mod selection;
mod shortcuts;
#[cfg(test)]
mod test_support;
mod theme;
mod usage;
mod verify;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use eframe::egui;
use orangedeck_infra::{
    AuthToken, PairingBundle, UiConfig, check_setup_destinations, default_ui_config_path,
    validate_tailscale_agent_url, write_secure, write_toml_secure,
};
use tracing_subscriber::EnvFilter;

use app::OrangeDeckApp;

#[derive(Parser)]
#[command(
    name = "orangedeck-ui",
    version,
    about = "Native OrangeDeck console for ROG Ally"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate local settings and credentials without opening a window or network.
    CheckConfig {
        #[arg(long, default_value_os_t = default_ui_config_path())]
        config: PathBuf,
    },
    /// Connect to a paired Mac Agent over Tailscale.
    Run {
        #[arg(long, default_value_os_t = default_ui_config_path())]
        config: PathBuf,
        #[arg(long)]
        notifications: bool,
    },
    /// Connect to a loopback Mock Agent.
    Demo {
        #[arg(long)]
        notifications: bool,
    },
    /// Import a private pairing bundle created on the Mac.
    Pair {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value_os_t = default_ui_config_path())]
        config: PathBuf,
        #[arg(long)]
        force: bool,
    },
    /// Inspect display, controller, Tailscale, config, and Agent reachability.
    Doctor {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        demo: bool,
        /// List workspace paths reported by the paired Agent, without conversation text.
        #[arg(long)]
        workspaces: bool,
    },
    /// Verify real HTTP/WebSocket state; actions require explicit flags and a project.
    Verify {
        #[arg(long, default_value_os_t = default_ui_config_path())]
        config: PathBuf,
        #[arg(long)]
        project_id: Option<String>,
        /// Run the fixed cargo check command in the registered project.
        #[arg(long, requires = "project_id")]
        cargo_check: bool,
        /// Create an owned thread and ask only for ORANGEDECK_OK, without tools.
        #[arg(long, requires = "project_id")]
        codex_prompt: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::CheckConfig { config } => {
            let config = UiConfig::load(&config)?;
            drop(AuthToken::load(&config.token_file)?);
            println!("{}", serde_json::json!({"valid": true}));
        }
        Command::Run {
            config,
            notifications,
        } => {
            input_backend::prepare()?;
            let config = UiConfig::load(&config)?;
            require_tailscale_url(&config.agent_url)?;
            let token = AuthToken::load(&config.token_file)?;
            init_tracing(&config.log_level);
            run_ui(config, token, false, notifications)?;
        }
        Command::Demo { notifications } => {
            input_backend::prepare()?;
            let config = UiConfig::demo();
            let token = AuthToken::parse(orangedeck_infra::DEMO_TOKEN)?;
            init_tracing("info");
            run_ui(config, token, true, notifications)?;
        }
        Command::Pair {
            bundle,
            config,
            force,
        } => {
            let bundle = PairingBundle::load(&bundle)?;
            require_tailscale_url(&bundle.agent_url)?;
            let parent = config
                .parent()
                .ok_or_else(|| format!("config path has no parent: {}", config.display()))?;
            let token_path = parent.join("ui.token");
            check_setup_destinations(&[&config, &token_path], force)?;
            write_secure(&token_path, &bundle.token)?;
            let ui_config = UiConfig {
                agent_url: bundle.agent_url,
                token_file: token_path,
                host_label: bundle.host_label,
                log_level: "info".to_owned(),
                desktop_notifications: true,
            };
            write_toml_secure(&config, &ui_config)?;
            println!("OrangeDeck pairing imported: {}", config.display());
            println!("Authentication token stored separately with private permissions.");
        }
        Command::Doctor {
            config,
            demo,
            workspaces,
        } => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let healthy = runtime.block_on(doctor::run(config.as_deref(), demo, workspaces));
            if !healthy {
                std::process::exit(2);
            }
        }
        Command::Verify {
            config,
            project_id,
            cargo_check,
            codex_prompt,
        } => {
            let config = UiConfig::load(&config)?;
            let token = AuthToken::load(&config.token_file)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(verify::run(
                config,
                token,
                project_id,
                cargo_check,
                codex_prompt,
            ))?;
        }
    }
    Ok(())
}

fn run_ui(
    config: UiConfig,
    token: AuthToken,
    demo_mode: bool,
    notifications: bool,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OrangeDeck")
            .with_inner_size([1_038.0, 584.0])
            .with_min_inner_size([820.0, 480.0])
            .with_maximized(true),
        ..eframe::NativeOptions::default()
    };
    eframe::run_native(
        "OrangeDeck",
        options,
        Box::new(move |context| {
            OrangeDeckApp::new(context, config, token, demo_mode)
                .map(|mut app| {
                    if notifications {
                        app.open_notifications();
                    }
                    Box::new(app) as Box<dyn eframe::App>
                })
                .map_err(|error| {
                    Box::new(std::io::Error::other(error))
                        as Box<dyn std::error::Error + Send + Sync>
                })
        }),
    )
}

fn require_tailscale_url(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    validate_tailscale_agent_url(url).map_err(Into::into)
}

fn init_tracing(default_level: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_ui_rejects_lan_and_public_urls() {
        assert!(require_tailscale_url("http://100.64.0.2:45831").is_ok());
        assert!(require_tailscale_url("http://100.1.2.3:45831").is_err());
        assert!(require_tailscale_url("http://192.168.0.2:45831").is_err());
        assert!(require_tailscale_url("http://0.0.0.0:45831").is_err());
    }
}
