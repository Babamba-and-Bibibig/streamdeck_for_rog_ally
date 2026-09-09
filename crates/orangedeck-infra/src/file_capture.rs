//! Observe a tool's filesystem events, then check only the reported paths' metadata.
//! No directory traversal, Git baseline, command parsing or filesystem writes.
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

use nix::{
    fcntl::{OFlag, open, openat},
    sys::stat::Mode,
};
use orangedeck_domain::{
    CodeChange, CodeChangeKind, TurnChanges, excluded_change_component as excluded,
};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)] // The native callback and stream lifetime are isolated here.
mod macos;
#[cfg(target_os = "macos")]
use macos::Watcher;
#[cfg(not(target_os = "macos"))]
mod other;
#[cfg(not(target_os = "macos"))]
use other::Watcher;

const MAX_PATHS: usize = 64;
const FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_CLOEXEC);

pub fn private_read_flags() -> i32 {
    (OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).bits()
}

#[derive(Clone, Copy, Default)]
struct EventFlags {
    created: bool,
    removed: bool,
    renamed: bool,
}

struct Events {
    root: PathBuf,
    paths: BTreeMap<String, EventFlags>,
    incomplete: bool,
}

impl Events {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            paths: BTreeMap::new(),
            incomplete: false,
        }
    }

    #[cfg(target_os = "macos")]
    fn accepts(&self, path: &Path) -> bool {
        path.strip_prefix(&self.root).is_ok_and(|relative| {
            relative.components().all(|component| match component {
                Component::Normal(name) => name.to_str().is_some_and(|name| !excluded(name)),
                _ => false,
            })
        })
    }

    fn record(&mut self, path: &Path, flags: EventFlags) {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return;
        };
        let Some(relative) = relative.to_str().filter(|path| !path.is_empty()) else {
            self.incomplete = true;
            return;
        };
        if relative.len() > 4096 || relative.chars().any(char::is_control) {
            self.incomplete = true;
            return;
        }
        if Path::new(relative)
            .components()
            .any(|component| match component {
                Component::Normal(name) => name.to_str().is_none_or(excluded),
                _ => true,
            })
        {
            return;
        }
        if self.paths.len() >= MAX_PATHS && !self.paths.contains_key(relative) {
            self.incomplete = true;
            return;
        }
        let entry = self.paths.entry(relative.to_owned()).or_default();
        entry.created |= flags.created;
        entry.removed |= flags.removed;
        entry.renamed |= flags.renamed;
    }
}

pub struct FileCapture {
    root: File,
    path: PathBuf,
    watcher: Watcher,
}

impl FileCapture {
    pub fn before(cwd: &str) -> io::Result<Self> {
        let path = crate::conversation_editor_workspace(cwd).map_err(io::Error::other)?;
        // Each component is opened relative to its parent descriptor, without links.
        let root = open_root(&path)?;
        let watcher = Watcher::start(&path)?;
        Ok(Self {
            root,
            path,
            watcher,
        })
    }

    pub fn finish(self) -> io::Result<TurnChanges> {
        let events = self.watcher.finish()?;
        let current = open_root(&self.path)?.metadata()?;
        let anchored = self.root.metadata()?;
        if current.dev() != anchored.dev() || current.ino() != anchored.ino() {
            return Err(io::Error::other(
                "working folder changed during file observation",
            ));
        }
        Ok(read_changes(&self.root, events))
    }
}

fn open_root(path: &Path) -> io::Result<File> {
    let mut directory = File::from(open(
        Path::new("/"),
        FLAGS | OFlag::O_DIRECTORY,
        Mode::empty(),
    )?);
    for component in path.components() {
        if let Component::Normal(name) = component {
            directory = File::from(openat(
                &directory,
                Path::new(name),
                FLAGS | OFlag::O_DIRECTORY,
                Mode::empty(),
            )?);
        }
    }
    Ok(directory)
}

fn open_file(root: &File, path: &str) -> io::Result<File> {
    let mut parent = root.try_clone()?;
    let mut components = Path::new(path).components().peekable();
    while let Some(Component::Normal(name)) = components.next() {
        let flags = if components.peek().is_some() {
            FLAGS | OFlag::O_DIRECTORY
        } else {
            FLAGS
        };
        parent = File::from(openat(&parent, Path::new(name), flags, Mode::empty())?);
    }
    Ok(parent)
}

fn read_changes(root: &File, events: Events) -> TurnChanges {
    let mut changes = TurnChanges {
        files: Vec::new(),
        truncated: events.incomplete,
    };
    for (path, flags) in events.paths {
        let mut change = CodeChange {
            path,
            previous_path: None,
            kind: if flags.created && !flags.removed && !flags.renamed {
                CodeChangeKind::Added
            } else {
                CodeChangeKind::Modified
            },
            first_line: 1,
            truncated: false,
        };
        match open_file(root, &change.path) {
            Ok(file) => {
                let Ok(metadata) = file.metadata() else {
                    changes.truncated = true;
                    continue;
                };
                if !metadata.is_file() {
                    continue;
                }
                // Keep the hard-link ambiguity marker without reading any file bytes.
                change.truncated = metadata.nlink() > 1;
            }
            Err(error)
                if error.kind() == io::ErrorKind::NotFound && (flags.removed || flags.renamed) =>
            {
                // A temporary file created and removed within this tool has no final file.
                if flags.created {
                    continue;
                }
                change.kind = CodeChangeKind::Deleted;
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                change.truncated = true;
            }
            Err(_) => {
                // Links, replaced ancestors and vanished/unreadable files are not followed.
                changes.truncated = true;
                continue;
            }
        }
        changes.truncated |= change.truncated;
        changes.files.push(change);
    }
    changes
}
