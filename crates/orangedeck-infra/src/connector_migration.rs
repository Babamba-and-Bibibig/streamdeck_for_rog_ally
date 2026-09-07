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

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy(root: &Path) -> std::path::PathBuf {
        let path = root.join("agent.toml");
        let config = ConnectorConfig {
            token_file: root.join("agent.token"),
            projects: vec![crate::ProjectConfig {
                id: "saved".to_owned(),
                name: "Saved project".to_owned(),
                path: root.to_path_buf(),
                browser_url: Some("https://example.com/".to_owned()),
            }],
            ..ConnectorConfig::default()
        };
        write_toml_secure(&path, &config).unwrap();
        write_secure(&config.token_file, &"migration-test-only-".repeat(4)).unwrap();
        path
    }

    #[test]
    fn migration_preserves_projects_unknown_fields_and_credentials_without_repairing() {
        let root = tempfile::tempdir().unwrap();
        let source = legacy(root.path());
        let original = format!(
            "future_setting = 'keep me'\n{}",
            fs::read_to_string(&source).unwrap()
        );
        write_secure(&source, &original).unwrap();
        let token = fs::read(root.path().join("agent.token")).unwrap();
        let target = root.path().join("connector.toml");
        import_legacy_connector_config(&source, &target).unwrap();
        let result = ConnectorConfig::load(&target).unwrap();
        assert_eq!(result.projects[0].name, "Saved project");
        assert_eq!(
            result.projects[0].browser_url.as_deref(),
            Some("https://example.com/")
        );
        assert_eq!(
            result.token_file,
            root.path().canonicalize().unwrap().join("connector.token")
        );
        assert_eq!(fs::read(result.token_file).unwrap(), token);
        assert_eq!(fs::read(root.path().join("agent.token")).unwrap(), token);
        assert_eq!(fs::read_to_string(&source).unwrap(), original);
        assert!(
            fs::read_to_string(&target)
                .unwrap()
                .contains("future_setting")
        );
        assert!(!root.path().join("orangedeck-pairing.toml").exists());
        assert!(import_legacy_connector_config(&source, &target).is_err());
    }

    #[test]
    fn interrupted_migration_reuses_only_the_same_private_token() {
        let root = tempfile::tempdir().unwrap();
        let source = legacy(root.path());
        let target = root.path().join("connector.toml");
        let token_path = root.path().join("connector.token");
        write_secure(&token_path, &"different-test-only-".repeat(4)).unwrap();
        assert!(import_legacy_connector_config(&source, &target).is_err());
        assert!(!target.exists());
        let original = fs::read_to_string(root.path().join("agent.token")).unwrap();
        write_secure(&token_path, &original).unwrap();
        import_legacy_connector_config(&source, &target).unwrap();
        assert_eq!(fs::read_to_string(token_path).unwrap(), original);
    }

    #[test]
    fn migration_cannot_write_configuration_over_its_new_credential() {
        let root = tempfile::tempdir().unwrap();
        let source = legacy(root.path());
        let target = root.path().join("connector.token");
        let original = fs::read(root.path().join("agent.token")).unwrap();
        assert!(import_legacy_connector_config(&source, &target).is_err());
        assert!(!target.exists());
        assert_eq!(fs::read(root.path().join("agent.token")).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn migration_rejects_symlinks_without_touching_existing_files() {
        let root = tempfile::tempdir().unwrap();
        let source = legacy(root.path());
        let target = root.path().join("connector.toml");
        std::os::unix::fs::symlink(&source, &target).unwrap();
        let before = fs::read(&source).unwrap();
        assert!(import_legacy_connector_config(&source, &target).is_err());
        assert_eq!(fs::read(&source).unwrap(), before);
        assert!(!root.path().join("connector.token").exists());
    }
}
