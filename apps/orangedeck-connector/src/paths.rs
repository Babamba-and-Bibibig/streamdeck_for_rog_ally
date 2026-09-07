//! Runtime locations selected at the Connector's composition root, never by domain policy.
use std::{
    io,
    path::{Path, PathBuf},
};

pub struct ConnectorPaths {
    pub hook_socket: PathBuf,
    pub owned_threads: PathBuf,
}

impl ConnectorPaths {
    pub fn for_config(config: &Path) -> io::Result<Self> {
        let config = config.canonicalize()?;
        let directory = config
            .parent()
            .ok_or_else(|| io::Error::other("config has no parent"))?;
        Ok(Self {
            hook_socket: directory.join("hooks/codex.sock"),
            owned_threads: directory.join("owned-codex-threads.json"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fresh_custom_config_has_an_isolated_working_hook_directory() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        for directory in [&first, &second] {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join("connector.toml"), "# path fixture").unwrap();
        }
        let paths = ConnectorPaths::for_config(&first.join("connector.toml")).unwrap();
        let other = ConnectorPaths::for_config(&second.join("connector.toml")).unwrap();
        assert_ne!(paths.hook_socket, other.hook_socket);
        assert_ne!(paths.owned_threads, other.owned_threads);
        assert_eq!(
            paths.owned_threads.parent().unwrap(),
            first.canonicalize().unwrap()
        );
        let hub = crate::hooks::HookHub::bind(paths.hook_socket.clone())
            .await
            .unwrap();
        assert!(
            tokio::net::UnixStream::connect(&paths.hook_socket)
                .await
                .is_ok()
        );
        assert!(!other.hook_socket.exists());
        hub.shutdown().await;
    }
}
