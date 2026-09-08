//! Bounded before/after observation of a local tool's working folder.
//! No Git baseline, shell parsing, subprocesses, symlink traversal, or file writes.
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::{Component, Path},
    sync::Arc,
    time::{Duration, Instant},
};

use nix::{
    dir::Dir,
    fcntl::{OFlag, open, openat},
    sys::stat::Mode,
};
use orangedeck_domain::{CodeChange, CodeChangeKind, TurnChanges};

const MAX_ENTRIES: usize = 8192;
const MAX_FILE: u64 = 262_144;
const MAX_CONTENT: usize = 8 * 1024 * 1024;
const SCAN_TIME: Duration = Duration::from_millis(250);
const FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK)
    .union(OFlag::O_CLOEXEC);

pub fn private_read_flags() -> i32 {
    (OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC).bits()
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp(u64, u64, u64, i64, i64, i64, i64);

impl From<&Metadata> for Stamp {
    fn from(value: &Metadata) -> Self {
        Self(
            value.dev(),
            value.ino(),
            value.len(),
            value.mtime(),
            value.mtime_nsec(),
            value.ctime(),
            value.ctime_nsec(),
        )
    }
}

struct Entry {
    stamp: Stamp,
    text: Option<Arc<str>>,
}

pub struct FileSnapshot {
    root: File,
    files: BTreeMap<String, Entry>,
    incomplete: bool,
}

impl FileSnapshot {
    pub fn before(cwd: &str) -> io::Result<Self> {
        let root = crate::conversation_editor_workspace(cwd).map_err(io::Error::other)?;
        // Open each canonical component relative to its directory descriptor.
        // A swapped ancestor cannot redirect a read through an outside symlink.
        let mut directory = File::from(open(
            Path::new("/"),
            FLAGS | OFlag::O_DIRECTORY,
            Mode::empty(),
        )?);
        for component in root.components() {
            if let Component::Normal(name) = component {
                directory = File::from(openat(
                    &directory,
                    Path::new(name),
                    FLAGS | OFlag::O_DIRECTORY,
                    Mode::empty(),
                )?);
            }
        }
        Self::scan(directory, None)
    }

    fn scan(root: File, previous: Option<&Self>) -> io::Result<Self> {
        let mut result = Self {
            root,
            files: BTreeMap::new(),
            incomplete: false,
        };
        let mut budget = ScanBudget {
            entries: MAX_ENTRIES,
            bytes: MAX_CONTENT,
            deadline: Instant::now() + SCAN_TIME,
        };
        let directory = result.root.try_clone()?;
        result.walk(&directory, "", 0, previous, &mut budget)?;
        Ok(result)
    }

    pub fn finish(self) -> io::Result<TurnChanges> {
        let after = Self::scan(self.root.try_clone()?, Some(&self))?;
        let mut changes = TurnChanges {
            files: Vec::new(),
            truncated: self.incomplete || after.incomplete,
        };
        let mut remaining = 65_536;
        for path in self.files.keys().chain(after.files.keys()) {
            // The second traversal handles only newly created paths.
            if changes.files.iter().any(|change| &change.path == path) {
                continue;
            }
            let old = self.files.get(path);
            let new = after.files.get(path);
            if old
                .zip(new)
                .is_some_and(|(a, b)| a.stamp == b.stamp || a.text.is_some() && a.text == b.text)
                || old.is_none() && self.incomplete
                || new.is_none() && after.incomplete
            {
                continue;
            }
            if changes.files.len() == 64 {
                changes.truncated = true;
                break;
            }
            let kind = if old.is_none() {
                CodeChangeKind::Added
            } else if new.is_none() {
                CodeChangeKind::Deleted
            } else {
                CodeChangeKind::Modified
            };
            let old_text = old.map_or(Some(""), |entry| entry.text.as_deref());
            let new_text = new.map_or(Some(""), |entry| entry.text.as_deref());
            let (diff, line, clipped) = match old_text.zip(new_text) {
                Some((before, after)) => text_diff(before, after, remaining.min(16_384)),
                None => (String::new(), 1, true),
            };
            remaining -= diff.len();
            changes.truncated |= clipped;
            changes.files.push(CodeChange {
                path: path.clone(),
                previous_path: None,
                kind,
                first_line: line,
                diff,
                truncated: clipped,
            });
        }
        Ok(changes)
    }

    fn walk(
        &mut self,
        parent: &File,
        prefix: &str,
        depth: usize,
        previous: Option<&Self>,
        budget: &mut ScanBudget,
    ) -> io::Result<()> {
        if depth > 32 {
            self.incomplete = true;
            return Ok(());
        }
        let mut directory = Dir::from_fd(parent.try_clone()?.into())?;
        for entry in directory.iter() {
            if budget.entries == 0 || Instant::now() >= budget.deadline {
                self.incomplete = true;
                break;
            }
            budget.entries -= 1;
            let entry = entry?;
            let Ok(name) = entry.file_name().to_str() else {
                self.incomplete = true;
                continue;
            };
            if excluded(name) {
                continue;
            }
            let path = format!("{prefix}{name}");
            if path.len() > 4096 || path.chars().any(char::is_control) {
                self.incomplete = true;
                continue;
            }
            let file = match openat(parent, entry.file_name(), FLAGS, Mode::empty()) {
                Ok(fd) => File::from(fd),
                Err(nix::errno::Errno::ELOOP) => continue,
                Err(_) => {
                    self.incomplete = true;
                    continue;
                }
            };
            let metadata = file.metadata()?;
            if metadata.is_dir() {
                if self
                    .walk(&file, &format!("{path}/"), depth + 1, previous, budget)
                    .is_err()
                {
                    self.incomplete = true;
                }
            } else if metadata.is_file() {
                let stamp = Stamp::from(&metadata);
                let cached = previous
                    .and_then(|snapshot| snapshot.files.get(&path))
                    .filter(|entry| entry.stamp == stamp);
                let text = if let Some(cached) = cached {
                    cached.text.clone()
                } else if metadata.len() <= MAX_FILE
                    && usize::try_from(metadata.len()).is_ok_and(|size| size <= budget.bytes)
                {
                    let mut bytes = Vec::new();
                    let read = (&file).take(MAX_FILE + 1).read_to_end(&mut bytes);
                    budget.bytes = budget.bytes.saturating_sub(bytes.len());
                    if read.is_ok()
                        && bytes.len() <= usize::try_from(MAX_FILE).unwrap_or(0)
                        && !bytes.contains(&0)
                        && file
                            .metadata()
                            .is_ok_and(|value| Stamp::from(&value) == stamp)
                    {
                        String::from_utf8(bytes).ok().map(Arc::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.files.insert(path, Entry { stamp, text });
            }
        }
        Ok(())
    }
}

struct ScanBudget {
    entries: usize,
    bytes: usize,
    deadline: Instant,
}

fn excluded(name: &str) -> bool {
    matches!(
        name,
        "." | ".."
            | ".git"
            | ".codex"
            | ".ssh"
            | ".aws"
            | ".gnupg"
            | ".config"
            | ".local"
            | "Library"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".cache"
            | ".next"
            | ".tox"
            | "auth.json"
            | "credentials.json"
            | "hooks.json"
            | "file-changes.json"
            | "known_hosts"
            | "authorized_keys"
    ) || name.starts_with(".env")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || [".token", ".pem", ".key", ".p12", ".pfx", ".local.toml"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

// A single bounded hunk between the common prefix/suffix is linear in file size.
// It preserves all changed text without introducing a quadratic diff algorithm.
fn text_diff(before: &str, after: &str, budget: usize) -> (String, u32, bool) {
    let old: Vec<_> = before.split_inclusive('\n').collect();
    let new: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let line = u32::try_from(prefix + 1).unwrap_or(1);
    let mut diff = format!(
        "@@ -{},{} +{},{} @@\n",
        prefix + 1,
        old.len() - prefix - suffix,
        prefix + 1,
        new.len() - prefix - suffix
    );
    let mut clipped = false;
    for (sign, lines) in [
        ("-", &old[prefix..old.len() - suffix]),
        ("+", &new[prefix..new.len() - suffix]),
    ] {
        for line in lines {
            if diff.len() + sign.len() + line.len() + 1 > budget {
                clipped = true;
                break;
            }
            diff.push_str(sign);
            diff.push_str(line);
            if !line.ends_with('\n') {
                diff.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    if diff.len() > budget {
        let mut end = budget;
        while !diff.is_char_boundary(end) {
            end -= 1;
        }
        diff.truncate(end);
        clipped = true;
    }
    (diff, line, clipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn bounded_scan_profile_with_an_actual_edit_among_1000_files() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..1000 {
            fs::write(
                directory.path().join(format!("file-{index}.rs")),
                "// source\n".repeat(50),
            )
            .unwrap();
        }
        let start = Instant::now();
        let before = FileSnapshot::before(directory.path().to_str().unwrap()).unwrap();
        let pre = start.elapsed();
        fs::write(directory.path().join("file-567.rs"), "// changed\n").unwrap();
        let start = Instant::now();
        let changes = before.finish().unwrap();
        let post = start.elapsed();
        eprintln!(
            "File capture profile: 1000 files, pre={} us, post={} us",
            pre.as_micros(),
            post.as_micros()
        );
        assert_eq!(changes.files.len(), 1);
        assert_eq!(changes.files[0].path, "file-567.rs");
        assert!(!changes.truncated);
    }

    #[test]
    fn real_file_edits_new_files_and_deletions_need_no_git_repository() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(
            root.join("codex_approval_test.py"),
            "# keep\nprint('before')\n# end\n",
        )
        .unwrap();
        fs::write(root.join("already-dirty.txt"), "uncommitted user work").unwrap();
        fs::write(root.join("removed.txt"), "removed content\n").unwrap();
        let before = FileSnapshot::before(root.to_str().unwrap()).unwrap();
        fs::write(
            root.join("codex_approval_test.py"),
            "# keep\nprint('after')\n# end\n",
        )
        .unwrap();
        fs::create_dir(root.join("새 폴더")).unwrap();
        fs::write(root.join("새 폴더/created.txt"), "created content\n").unwrap();
        fs::remove_file(root.join("removed.txt")).unwrap();
        let changes = before.finish().unwrap();
        assert!(!changes.truncated);
        assert_eq!(changes.files.len(), 3);
        let modified = changes
            .files
            .iter()
            .find(|file| file.path == "codex_approval_test.py")
            .unwrap();
        assert_eq!(modified.kind, CodeChangeKind::Modified);
        assert_eq!(modified.first_line, 2);
        assert_eq!(
            modified.diff,
            "@@ -2,1 +2,1 @@\n-print('before')\n+print('after')\n"
        );
        assert!(changes.files.iter().any(
            |file| file.kind == CodeChangeKind::Added && file.diff.contains("+created content")
        ));
        assert!(
            changes
                .files
                .iter()
                .any(|file| file.kind == CodeChangeKind::Deleted
                    && file.diff.contains("-removed content"))
        );
    }

    #[test]
    fn unchanged_dirty_files_are_empty_and_same_length_edits_are_detected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.py");
        fs::write(&path, "before").unwrap();
        let cwd = directory.path().to_str().unwrap();
        let empty = FileSnapshot::before(cwd).unwrap().finish().unwrap();
        assert!(empty.files.is_empty() && !empty.truncated);
        let before = FileSnapshot::before(cwd).unwrap();
        fs::write(path, "after!").unwrap();
        assert_eq!(before.finish().unwrap().files.len(), 1);
    }

    #[test]
    fn symlinks_private_files_and_generated_trees_are_not_read() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::write(directory.path().join("outside.txt"), "private outside").unwrap();
        symlink(
            directory.path().join("outside.txt"),
            root.join("linked.txt"),
        )
        .unwrap();
        symlink(directory.path(), root.join("linked-directory")).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        let before = FileSnapshot::before(root.to_str().unwrap()).unwrap();
        fs::write(directory.path().join("outside.txt"), "outside changed").unwrap();
        fs::write(root.join(".env"), "SECRET=not-collected").unwrap();
        fs::write(root.join("target/generated.txt"), "not-collected").unwrap();
        let changes = before.finish().unwrap();
        assert!(changes.files.is_empty());
        assert!(!changes.truncated);
        assert!(FileSnapshot::before("/").is_err());
    }

    #[test]
    fn replacing_a_directory_with_an_outside_link_cannot_import_its_contents() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir_all(root.join("source")).unwrap();
        fs::write(root.join("source/example.txt"), "inside\n").unwrap();
        let before = FileSnapshot::before(root.to_str().unwrap()).unwrap();
        fs::rename(root.join("source"), directory.path().join("moved")).unwrap();
        symlink(directory.path(), root.join("source")).unwrap();
        fs::write(directory.path().join("secret.txt"), "do not import").unwrap();
        let changes = before.finish().unwrap();
        assert_eq!(changes.files.len(), 1);
        assert_eq!(changes.files[0].kind, CodeChangeKind::Deleted);
        assert!(!changes.files[0].diff.contains("do not import"));
    }

    #[test]
    fn large_and_binary_edits_keep_the_filename_without_copying_the_contents() {
        let directory = tempfile::tempdir().unwrap();
        let before = FileSnapshot::before(directory.path().to_str().unwrap()).unwrap();
        fs::write(
            directory.path().join("large.txt"),
            vec![b'x'; usize::try_from(MAX_FILE + 1).unwrap()],
        )
        .unwrap();
        fs::write(directory.path().join("binary.dat"), b"binary\0bytes").unwrap();
        let changes = before.finish().unwrap();
        assert_eq!(changes.files.len(), 2);
        assert!(changes.truncated);
        assert!(
            changes
                .files
                .iter()
                .all(|file| file.truncated && file.diff.is_empty())
        );
    }

    #[test]
    fn file_count_and_diff_bytes_are_bounded_and_report_incompleteness() {
        let directory = tempfile::tempdir().unwrap();
        let before = FileSnapshot::before(directory.path().to_str().unwrap()).unwrap();
        for index in 0..70 {
            fs::write(
                directory.path().join(format!("file-{index}.txt")),
                "changed\n",
            )
            .unwrap();
        }
        let changes = before.finish().unwrap();
        assert_eq!(changes.files.len(), 64);
        assert!(changes.truncated);
        let (diff, _, clipped) = text_diff("before", &"수정\n".repeat(10_000), 128);
        assert!(diff.len() <= 128 && clipped);
        assert!(std::str::from_utf8(diff.as_bytes()).is_ok());
    }
}
