use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};

use orangedeck_infra::{
    AuthToken, BindMode, ConnectorConfig, PairingBundle, ProjectConfig, check_setup_destinations,
    default_connector_config_path, is_tailscale_ip, tailscale_ip, write_secure, write_toml_secure,
};
use orangedeck_protocol::PROTOCOL_VERSION;
use thiserror::Error;

pub struct InitOptions {
    pub config_path: PathBuf,
    pub project_id: String,
    pub project_name: String,
    pub project_path: PathBuf,
    pub browser_url: Option<String>,
    pub host_name: String,
    pub port: u16,
    pub cargo_binary: PathBuf,
    pub codex_binary: PathBuf,
    pub editor: orangedeck_infra::EditorKind,
    pub force: bool,
}

pub struct InitResult {
    pub config_path: PathBuf,
    pub pairing_path: PathBuf,
    pub bind_ip: IpAddr,
}

pub async fn initialize(options: InitOptions) -> Result<InitResult, SetupError> {
    let project_path =
        options
            .project_path
            .canonicalize()
            .map_err(|source| SetupError::ProjectPath {
                path: options.project_path,
                source,
            })?;
    if !project_path.is_dir() {
        return Err(SetupError::NotDirectory(project_path));
    }
    let config_dir = options
        .config_path
        .parent()
        .ok_or_else(|| SetupError::NoParent(options.config_path.clone()))?;
    let token_path = config_dir.join("connector.token");
    let pairing_path = config_dir.join("orangedeck-pairing.toml");
    if !options.force
        && ["agent.toml", "agent.token"]
            .iter()
            .any(|name| std::fs::symlink_metadata(config_dir.join(name)).is_ok())
    {
        return Err(orangedeck_infra::ConfigError::Invalid(
            "an older setup exists; run Setup to import it without changing credentials".to_owned(),
        )
        .into());
    }
    check_setup_destinations(
        &[&options.config_path, &token_path, &pairing_path],
        options.force,
    )?;
    let bind_ip = tailscale_ip().await.map_err(SetupError::Tailscale)?;
    if !is_tailscale_ip(bind_ip) {
        return Err(SetupError::NotTailscale(bind_ip));
    }

    let token = AuthToken::generate();
    let config = ConnectorConfig {
        host_name: options.host_name.clone(),
        bind_mode: BindMode::Tailscale,
        bind_address: None,
        port: options.port,
        token_file: token_path.clone(),
        cargo_binary: options.cargo_binary,
        codex_binary: options.codex_binary,
        editor: options.editor,
        log_level: "info".to_owned(),
        projects: vec![ProjectConfig {
            id: options.project_id,
            name: options.project_name,
            path: project_path,
            browser_url: options.browser_url,
        }],
    };
    config.validate()?;
    drop(config.project_registry()?);
    write_secure(&token_path, token.expose())?;
    write_toml_secure(&options.config_path, &config)?;
    let bundle = PairingBundle {
        protocol_version: PROTOCOL_VERSION,
        connector_url: format!("http://{bind_ip}:{}", options.port),
        host_label: options.host_name,
        token: token.expose().to_owned(),
    };
    write_toml_secure(&pairing_path, &bundle)?;
    Ok(InitResult {
        config_path: options.config_path,
        pairing_path,
        bind_ip,
    })
}

pub async fn write_pairing(config_path: &Path, output: &Path) -> Result<IpAddr, SetupError> {
    let config = ConnectorConfig::load(config_path)?;
    if config.bind_mode != BindMode::Tailscale {
        return Err(SetupError::NotTailscaleMode);
    }
    let token = AuthToken::load(&config.token_file)?;
    let bind_ip = tailscale_ip().await.map_err(SetupError::Tailscale)?;
    if !is_tailscale_ip(bind_ip) {
        return Err(SetupError::NotTailscale(bind_ip));
    }
    let bundle = PairingBundle {
        protocol_version: PROTOCOL_VERSION,
        connector_url: format!("http://{bind_ip}:{}", config.port),
        host_label: config.host_name,
        token: token.expose().to_owned(),
    };
    write_toml_secure(output, &bundle)?;
    Ok(bind_ip)
}

pub fn default_init_config_path() -> PathBuf {
    default_connector_config_path()
}

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("cannot resolve project path {path}: {source}")]
    ProjectPath {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("project path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("path has no parent directory: {0}")]
    NoParent(PathBuf),
    #[error("Tailscale is required and no usable address was found: {0}")]
    Tailscale(orangedeck_infra::SystemError),
    #[error("address {0} is not a Tailscale IPv4 address")]
    NotTailscale(IpAddr),
    #[error("pairing is only available when bind_mode = \"tailscale\"")]
    NotTailscaleMode,
    #[error(transparent)]
    Config(#[from] orangedeck_infra::ConfigError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_cgnat_tailscale_addresses() {
        assert!(is_tailscale_ip("100.64.0.1".parse().unwrap()));
        assert!(is_tailscale_ip("100.127.255.254".parse().unwrap()));
        assert!(!is_tailscale_ip("100.1.2.3".parse().unwrap()));
        assert!(!is_tailscale_ip("192.168.0.2".parse().unwrap()));
        assert!(!is_tailscale_ip("127.0.0.1".parse().unwrap()));
    }
}
