use std::{
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
};

use directories::{BaseDirs, ProjectDirs};
use orangedeck_domain::{Project, ProjectId, ProjectRegistry};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use thiserror::Error;
use url::{Host, Url};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 45_831;
pub const DEMO_TOKEN: &str = "orangedeck-demo-token-localhost-only-0001";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindMode {
    #[default]
    Tailscale,
    Loopback,
    Explicit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectorConfig {
    pub host_name: String,
    pub bind_mode: BindMode,
    pub bind_address: Option<IpAddr>,
    pub port: u16,
    pub token_file: PathBuf,
    pub cargo_binary: PathBuf,
    pub codex_binary: PathBuf,
    pub editor: crate::EditorKind,
    pub log_level: String,
    pub projects: Vec<ProjectConfig>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            host_name: "MACBOOK".to_owned(),
            bind_mode: BindMode::Tailscale,
            bind_address: None,
            port: DEFAULT_PORT,
            token_file: default_config_dir().join("connector.token"),
            cargo_binary: PathBuf::from("cargo"),
            codex_binary: PathBuf::from("codex"),
            editor: crate::EditorKind::default(),
            log_level: "info".to_owned(),
            projects: Vec::new(),
        }
    }
}

impl ConnectorConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        // A pre-0.1.23 file without an explicit token path still uses its old token.
        let text = if path.file_name().is_some_and(|name| name == "agent.toml") {
            let mut document: toml::Value = toml::from_str(&text)?;
            if document.get("token_file").is_none() {
                document["token_file"] = toml::Value::String(
                    path.with_file_name("agent.token")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            toml::to_string(&document)?
        } else {
            text
        };
        let mut config: Self = toml::from_str(&text)?;
        config.token_file = expand_home(&config.token_file)?;
        config.cargo_binary = expand_home(&config.cargo_binary)?;
        config.codex_binary = expand_home(&config.codex_binary)?;
        for project in &mut config.projects {
            project.path = expand_home(&project.path)?;
        }
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::Invalid("port must be non-zero".to_owned()));
        }
        if self.bind_mode == BindMode::Explicit && self.bind_address.is_none() {
            return Err(ConfigError::Invalid(
                "bind_address is required when bind_mode is explicit".to_owned(),
            ));
        }
        if self.projects.is_empty() {
            return Err(ConfigError::Invalid(
                "register at least one project in connector.toml".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn project_registry(&self) -> Result<ProjectRegistry, ConfigError> {
        let projects = self
            .projects
            .iter()
            .map(ProjectConfig::to_domain)
            .collect::<Result<Vec<_>, _>>()?;
        ProjectRegistry::new(projects).map_err(ConfigError::Registry)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub browser_url: Option<String>,
}

impl ProjectConfig {
    pub fn to_domain(&self) -> Result<Project, ConfigError> {
        if !self.path.is_dir() {
            return Err(ConfigError::Invalid(format!(
                "project `{}` directory does not exist: {}",
                self.id,
                self.path.display()
            )));
        }
        let canonical_path = self
            .path
            .canonicalize()
            .map_err(|source| ConfigError::Read {
                path: self.path.clone(),
                source,
            })?;
        if self.name.trim().is_empty() || self.name.chars().count() > 120 {
            return Err(ConfigError::Invalid(format!(
                "project `{}` name must contain 1-120 characters",
                self.id
            )));
        }
        if let Some(browser_url) = &self.browser_url {
            let url = Url::parse(browser_url).map_err(|error| {
                ConfigError::Invalid(format!(
                    "project `{}` has an invalid browser_url: {error}",
                    self.id
                ))
            })?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err(ConfigError::Invalid(format!(
                    "project `{}` browser_url must use http or https",
                    self.id
                )));
            }
        }
        Ok(Project {
            id: ProjectId::new(self.id.clone())?,
            name: self.name.clone(),
            path: canonical_path,
            browser_url: self.browser_url.clone(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    #[serde(alias = "agent_url")]
    pub connector_url: String,
    pub token_file: PathBuf,
    pub host_label: String,
    pub log_level: String,
    pub desktop_notifications: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PairingBundle {
    pub protocol_version: u16,
    #[serde(alias = "agent_url")]
    pub connector_url: String,
    pub host_label: String,
    pub token: String,
}

impl fmt::Debug for PairingBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingBundle")
            .field("protocol_version", &self.protocol_version)
            .field("connector_url", &self.connector_url)
            .field("host_label", &self.host_label)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl PairingBundle {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.protocol_version != orangedeck_protocol::PROTOCOL_VERSION {
            return Err(ConfigError::Invalid(format!(
                "pairing bundle protocol {} is incompatible with protocol {}",
                self.protocol_version,
                orangedeck_protocol::PROTOCOL_VERSION
            )));
        }
        validate_tailscale_connector_url(&self.connector_url)?;
        AuthToken::parse(self.token.clone())?;
        if self.token == DEMO_TOKEN {
            return Err(ConfigError::Invalid(
                "the public demo token cannot pair a real Connector".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        check_secret_permissions(path)?;
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let bundle: Self = toml::from_str(&text)?;
        bundle.validate()?;
        Ok(bundle)
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            connector_url: format!("http://127.0.0.1:{DEFAULT_PORT}"),
            token_file: default_config_dir().join("ui.token"),
            host_label: "MACBOOK".to_owned(),
            log_level: "info".to_owned(),
            desktop_notifications: true,
        }
    }
}

impl UiConfig {
    pub fn demo() -> Self {
        Self {
            token_file: PathBuf::new(),
            host_label: "SIMULATED MAC".to_owned(),
            desktop_notifications: false,
            ..Self::default()
        }
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let mut config: Self = toml::from_str(&text)?;
        config.token_file = expand_home(&config.token_file)?;
        validate_tailscale_connector_url(&config.connector_url)?;
        Ok(config)
    }
}

pub fn is_tailscale_ip(address: IpAddr) -> bool {
    matches!(address, IpAddr::V4(address) if {
        let octets = address.octets();
        octets[0] == 100 && (64..=127).contains(&octets[1])
    })
}

pub fn validate_tailscale_connector_url(value: &str) -> Result<(), ConfigError> {
    let url = Url::parse(value)
        .map_err(|error| ConfigError::Invalid(format!("invalid Connector URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ConfigError::Invalid(
            "Connector URL scheme must be http or https".to_owned(),
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ConfigError::Invalid(
            "Connector URL must not contain credentials".to_owned(),
        ));
    }
    let address = match url.host() {
        Some(Host::Ipv4(address)) => IpAddr::V4(address),
        _ => {
            return Err(ConfigError::Invalid(
                "Connector URL must use a direct Tailscale IPv4 address".to_owned(),
            ));
        }
    };
    if !is_tailscale_ip(address) {
        return Err(ConfigError::Invalid(format!(
            "Connector URL address {address} is outside Tailscale's 100.64.0.0/10 range"
        )));
    }
    if url.port().is_none() {
        return Err(ConfigError::Invalid(
            "Connector URL must include an explicit port".to_owned(),
        ));
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(ConfigError::Invalid(
            "Connector URL must not include a path, query, or fragment".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone)]
pub struct AuthToken(String);

impl AuthToken {
    pub fn generate() -> Self {
        Self(format!(
            "{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        ))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, ConfigError> {
        let value = value.into();
        if value.len() < 32 || value.chars().any(char::is_whitespace) {
            return Err(ConfigError::Invalid(
                "authentication token must contain at least 32 non-whitespace characters"
                    .to_owned(),
            ));
        }
        Ok(Self(value))
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        check_secret_permissions(path)?;
        let value = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let token = Self::parse(value.trim())?;
        if token.expose() == DEMO_TOKEN {
            return Err(ConfigError::Invalid(
                "the public demo token cannot authenticate a real Connector".to_owned(),
            ));
        }
        Ok(token)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn matches(&self, candidate: &str) -> bool {
        let own = self.0.as_bytes();
        let candidate = candidate.as_bytes();
        own.len() == candidate.len() && bool::from(own.ct_eq(candidate))
    }
}

impl fmt::Debug for AuthToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuthToken([REDACTED])")
    }
}

pub fn default_config_dir() -> PathBuf {
    ProjectDirs::from("dev", "OrangeDeck", "OrangeDeck").map_or_else(
        || PathBuf::from(".orangedeck"),
        |dirs| dirs.config_dir().to_path_buf(),
    )
}

pub fn default_connector_config_path() -> PathBuf {
    default_config_dir().join("connector.toml")
}

pub fn default_ui_config_path() -> PathBuf {
    default_config_dir().join("config.toml")
}

/// Check every credential/config destination before initialization changes any file.
pub fn check_setup_destinations(paths: &[&Path], force: bool) -> Result<(), ConfigError> {
    for (index, path) in paths.iter().enumerate() {
        if paths[..index].contains(path) {
            return Err(ConfigError::Invalid(
                "config, token and pairing destinations must be distinct".to_owned(),
            ));
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(ConfigError::Invalid(
                    "setup refuses symlinks and non-regular destination files".to_owned(),
                ));
            }
            Ok(_) if !force => {
                return Err(ConfigError::Invalid(
                    "configuration or credentials already exist; review the existing setup before using --force".to_owned(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(())
}

pub fn write_secure(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let parent = path.parent().ok_or_else(|| {
        ConfigError::Invalid(format!("path has no parent directory: {}", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
        path: parent.to_path_buf(),
        source,
    })?;

    let temporary = parent.join(format!(".orangedeck-{}.tmp", Uuid::new_v4().simple()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|source| ConfigError::Write {
            path: temporary.clone(),
            source,
        })?;
    if let Err(source) = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ConfigError::Write {
            path: temporary,
            source,
        });
    }
    drop(file);
    if let Err(source) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ConfigError::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

pub fn write_toml_secure<T: Serialize>(path: &Path, value: &T) -> Result<(), ConfigError> {
    write_secure(path, &toml::to_string_pretty(value)?)
}

fn expand_home(path: &Path) -> Result<PathBuf, ConfigError> {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") {
        let home = BaseDirs::new()
            .ok_or_else(|| ConfigError::Invalid("cannot determine user home directory".to_owned()))?
            .home_dir()
            .to_path_buf();
        if text == "~" {
            Ok(home)
        } else {
            Ok(home.join(&text[2..]))
        }
    } else {
        Ok(path.to_path_buf())
    }
}

#[cfg(unix)]
fn check_secret_permissions(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(ConfigError::InsecurePermissions {
            path: path.to_path_buf(),
            mode: mode & 0o777,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_secret_permissions(path: &Path) -> Result<(), ConfigError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(ConfigError::Invalid(format!(
            "token file does not exist: {}",
            path.display()
        )))
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    // TOML errors may embed the entire source, including a pairing token. Drop it.
    #[error("invalid TOML configuration; inspect the file locally (contents withheld)")]
    TomlDecode,
    #[error("cannot encode TOML: {0}")]
    TomlEncode(#[from] toml::ser::Error),
    #[error(transparent)]
    ProjectId(#[from] orangedeck_domain::ProjectIdError),
    #[error(transparent)]
    Registry(orangedeck_domain::ProjectRegistryError),
    #[error("insecure permissions on {path}: mode {mode:o}; expected 0600")]
    InsecurePermissions { path: PathBuf, mode: u32 },
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

impl From<toml::de::Error> for ConfigError {
    fn from(_: toml::de::Error) -> Self {
        Self::TomlDecode
    }
}
