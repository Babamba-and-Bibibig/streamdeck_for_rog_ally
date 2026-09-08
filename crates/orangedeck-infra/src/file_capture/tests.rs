use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    time::Instant,
};

fn collected(root: &Path, paths: &[(&str, EventFlags)]) -> TurnChanges {
    let mut events = Events::new(root.to_owned());
    for (path, flags) in paths {
        events.record(&root.join(path), *flags);
    }
    read_changes(&open_root(&root.canonicalize().unwrap()).unwrap(), events)
}

#[test]
fn large_unrelated_subfolder_does_not_hide_root_edits_additions_or_deletions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let downloads = root.join("Downloads");
    fs::create_dir(&downloads).unwrap();
    for index in 0..10_000 {
        fs::write(
            downloads.join(format!("unrelated-{index}.txt")),
            "unchanged",
        )
        .unwrap();
    }
    fs::write(root.join("codex_approval_test.py"), "print('before')\n").unwrap();
    fs::write(root.join("removed.txt"), "removed content\n").unwrap();
    // An independent OS observer detects even opening the unrelated subtree.
    #[cfg(target_os = "linux")]
    let audit = {
        use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
        let audit = Inotify::init(InitFlags::IN_NONBLOCK | InitFlags::IN_CLOEXEC).unwrap();
        audit
            .add_watch(
                &downloads,
                AddWatchFlags::IN_OPEN | AddWatchFlags::IN_ACCESS,
            )
            .unwrap();
        audit
    };
    let start = Instant::now();
    let capture = FileCapture::before(root.to_str().unwrap()).unwrap();
    let pre = start.elapsed();
    fs::write(root.join("codex_approval_test.py"), "print('after')\n").unwrap();
    fs::write(root.join("created.py"), "print('created')\n").unwrap();
    fs::remove_file(root.join("removed.txt")).unwrap();
    let start = Instant::now();
    let changes = capture.finish().unwrap();
    eprintln!(
        "Native event capture: 10000 unrelated files; start={}us, finish={}us",
        pre.as_micros(),
        start.elapsed().as_micros()
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        audit.read_events().unwrap_err(),
        nix::errno::Errno::EAGAIN,
        "An unrelated folder was opened/read"
    );
    assert_eq!(changes.files.len(), 3);
    for (path, kind, content) in [
        (
            "codex_approval_test.py",
            CodeChangeKind::Modified,
            Some("print('after')\n"),
        ),
        (
            "created.py",
            CodeChangeKind::Added,
            Some("print('created')\n"),
        ),
        ("removed.txt", CodeChangeKind::Deleted, None),
    ] {
        let change = changes
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("Missing {path}"));
        assert_eq!(change.kind, kind);
        assert_eq!(change.content.as_deref(), content);
        assert!(
            change.diff.is_empty(),
            "An event does not provide the old contents"
        );
        assert!(!change.truncated);
    }
    // Linux's nonrecursive development adapter always reports its coverage limit.
    assert_eq!(changes.truncated, !cfg!(target_os = "macos"));
}

#[test]
fn nested_event_paths_read_only_the_reported_file_and_ignore_unchanged_dirty_work() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir_all(root.join("project/deep/새 폴더")).unwrap();
    fs::write(
        root.join("project/deep/새 폴더/test.py"),
        "print('changed')\n",
    )
    .unwrap();
    fs::write(root.join("dirty.txt"), "uncommitted user work").unwrap();
    let changes = collected(
        root,
        &[("project/deep/새 폴더/test.py", EventFlags::default())],
    );
    assert_eq!(changes.files.len(), 1);
    assert_eq!(
        changes.files[0].content.as_deref(),
        Some("print('changed')\n")
    );
    assert!(!changes.truncated);
    assert!(collected(root, &[]).files.is_empty());
}

#[test]
fn duplicate_events_are_coalesced_and_dropped_events_do_not_erase_known_paths() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("file.py"), "latest content").unwrap();
    let mut events = Events::new(directory.path().to_owned());
    for _ in 0..300_000 {
        events.record(&directory.path().join("file.py"), EventFlags::default());
    }
    assert_eq!(events.paths.len(), 1);
    assert!(!events.incomplete);
    events.incomplete = true;
    let changes = read_changes(
        &open_root(&directory.path().canonicalize().unwrap()).unwrap(),
        events,
    );
    assert_eq!(changes.files[0].content.as_deref(), Some("latest content"));
    assert!(changes.truncated);
}

#[test]
fn links_replaced_ancestors_private_paths_and_outside_paths_cannot_copy_contents() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("project");
    fs::create_dir_all(root.join("source")).unwrap();
    let secret = directory.path().join("secret.txt");
    fs::write(&secret, "never copy this value").unwrap();
    symlink(&secret, root.join("link.txt")).unwrap();
    fs::hard_link(&secret, root.join("hardlink.txt")).unwrap();
    fs::rename(root.join("source"), directory.path().join("moved")).unwrap();
    symlink(directory.path(), root.join("source")).unwrap();
    fs::create_dir(root.join("target")).unwrap();
    fs::write(root.join(".env"), "not collected").unwrap();
    fs::write(root.join("target/generated.txt"), "not collected").unwrap();
    let changes = collected(
        &root,
        &[
            ("../secret.txt", EventFlags::default()),
            ("link.txt", EventFlags::default()),
            ("hardlink.txt", EventFlags::default()),
            ("source/secret.txt", EventFlags::default()),
            (".env", EventFlags::default()),
            ("target/generated.txt", EventFlags::default()),
        ],
    );
    assert!(changes.truncated);
    assert!(
        changes
            .files
            .iter()
            .all(|change| change.content.is_none() && change.diff.is_empty())
    );
    assert!(
        changes
            .files
            .iter()
            .all(|change| change.path == "hardlink.txt")
    );
    assert!(FileCapture::before("/").is_err());
}

#[test]
fn changed_file_and_content_limits_keep_filenames_and_valid_utf8() {
    let directory = tempfile::tempdir().unwrap();
    let mut events = Events::new(directory.path().to_owned());
    for index in 0..70 {
        let path = directory.path().join(format!("file-{index:02}.txt"));
        fs::write(&path, "수정\n".repeat(10_000)).unwrap();
        events.record(
            &path,
            EventFlags {
                created: true,
                ..EventFlags::default()
            },
        );
    }
    let changes = read_changes(
        &open_root(&directory.path().canonicalize().unwrap()).unwrap(),
        events,
    );
    assert_eq!(changes.files.len(), 64);
    assert!(changes.truncated);
    assert!(changes.files.iter().all(|file| {
        file.content
            .as_ref()
            .is_none_or(|text| text.len() <= MAX_FILE)
    }));
    assert!(
        changes
            .files
            .iter()
            .map(|file| file.content.as_ref().map_or(0, String::len))
            .sum::<usize>()
            <= MAX_CONTENT
    );
    fs::write(directory.path().join("binary.dat"), b"binary\0bytes").unwrap();
    let binary = collected(directory.path(), &[("binary.dat", EventFlags::default())]);
    assert_eq!(binary.files[0].path, "binary.dat");
    assert!(binary.files[0].content.is_none() && binary.files[0].truncated);
}

#[test]
fn transient_files_and_unreadable_deletions_are_not_invented() {
    let directory = tempfile::tempdir().unwrap();
    let changes = collected(
        directory.path(),
        &[
            (
                "temporary.txt",
                EventFlags {
                    created: true,
                    removed: true,
                    ..EventFlags::default()
                },
            ),
            ("unobserved.txt", EventFlags::default()),
            (
                "deleted.txt",
                EventFlags {
                    removed: true,
                    ..EventFlags::default()
                },
            ),
        ],
    );
    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.files[0].path, "deleted.txt");
    assert_eq!(changes.files[0].kind, CodeChangeKind::Deleted);
    assert!(changes.truncated);
    // Replacing the watched root cannot redirect the later reader to a new root.
    let root = directory.path().join("root");
    fs::create_dir(&root).unwrap();
    let capture = FileCapture::before(root.to_str().unwrap()).unwrap();
    fs::rename(&root, directory.path().join("previous")).unwrap();
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(capture.finish().is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_new_nested_folders_atomic_saves_and_immediate_finish_use_real_fsevents() {
    // Run natively on the work Mac with --test-threads=1; no Codex/network/editor.
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    for index in 0..10 {
        let capture = FileCapture::before(root.to_str().unwrap()).unwrap();
        let child = root.join(format!("new-{index}/deep"));
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join("temporary"), "created content\n").unwrap();
        fs::rename(child.join("temporary"), child.join("test.py")).unwrap();
        let changes = capture.finish().unwrap();
        let file = changes
            .files
            .iter()
            .find(|file| file.path.ends_with("/test.py"))
            .expect("Immediate PostToolUse must flush the nested file event");
        assert_eq!(file.content.as_deref(), Some("created content\n"));
        assert!(!file.truncated);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn macos_cancelled_tools_release_capacity_for_the_next_turn() {
    let directory = tempfile::tempdir().unwrap();
    let cwd = directory.path().to_str().unwrap();
    let captures: Vec<_> = (0..4).map(|_| FileCapture::before(cwd).unwrap()).collect();
    drop(captures);
    let next = FileCapture::before(cwd).expect("Cancelled tools must release their observer slots");
    fs::write(directory.path().join("next.py"), "next turn\n").unwrap();
    assert!(
        next.finish()
            .unwrap()
            .files
            .iter()
            .any(|file| file.path == "next.py")
    );
}
