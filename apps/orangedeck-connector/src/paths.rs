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
