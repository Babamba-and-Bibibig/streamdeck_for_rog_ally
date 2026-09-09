//! Local installation migration; never called by a remote command.
use std::{
    fs::{self, OpenOptions},
    io::Read,
    path::Path,
};

use crate::{
    AuthToken, ConfigError, ConnectorConfig, check_setup_destinations, write_secure,
    write_toml_secure,
};

fn read_regular(path: &Path, limit: u64) -> Result<String, ConfigError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(ConfigError::Invalid(
            "migration requires bounded regular configuration and credential files".to_owned(),
        ));
    }
    let mut text = String::new();
    file.take(limit + 1)
        .read_to_string(&mut text)
        .map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    if text.len() as u64 > limit {
        return Err(ConfigError::Invalid(
            "migration file is too large".to_owned(),
        ));
    }
    Ok(text)
}

/// Copy an existing setup to the Connector filenames, preserving credentials and
/// unknown configuration fields. The old files and pairing bundle are untouched.
pub fn import_legacy_connector_config(source: &Path, target: &Path) -> Result<(), ConfigError> {
    let source_parent = source.parent().ok_or_else(|| {
        ConfigError::Invalid("migration source needs a parent directory".to_owned())
    })?;
    let target_parent = target.parent().ok_or_else(|| {
        ConfigError::Invalid("migration target needs a parent directory".to_owned())
    })?;
    let canonical = |path: &Path| {
        path.canonicalize().map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })
    };
    let directory = canonical(source_parent)?;
    if directory != canonical(target_parent)? || source == target {
        return Err(ConfigError::Invalid(
            "migration must keep the existing settings directory and use a new filename".to_owned(),
        ));
    }
    if target
        .file_name()
        .is_some_and(|name| name == "connector.token")
    {
        return Err(ConfigError::Invalid(
            "the configuration and credential must use different filenames".to_owned(),
        ));
    }
    check_setup_destinations(&[target], false)?;
    let text = read_regular(source, 1_048_576)?;
    let mut document: toml::Value = toml::from_str(&text)?;
    let config = ConnectorConfig::load(source)?;
    drop(config.project_registry()?);
    // Bound and reject symlinks before using the normal private-token validation.
    drop(read_regular(&config.token_file, 16_384)?);
    let token = AuthToken::load(&config.token_file)?;
    let token_path = directory.join("connector.token");
    let token_exists = fs::symlink_metadata(&token_path).is_ok();
    check_setup_destinations(&[&token_path], token_exists)?;
    if token_exists {
        drop(read_regular(&token_path, 16_384)?);
        let existing = AuthToken::load(&token_path)?;
        if !existing.matches(token.expose()) {
            return Err(ConfigError::Invalid(
                "a different Connector credential exists; review the two setups before updating"
                    .to_owned(),
            ));
        }
    }
    // Keeping the original TOML document preserves settings newer than this code.
    document["token_file"] = toml::Value::String(token_path.to_string_lossy().into_owned());
    if !token_exists {
        write_secure(&token_path, token.expose())?;
    }
    // A failed final write can be retried only with the same existing credential.
    write_toml_secure(target, &document)
}
