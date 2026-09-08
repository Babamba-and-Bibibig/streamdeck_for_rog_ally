//! Bind native file events to local tool hooks and retain their exact turn records.
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Mutex,
};

use chrono::Utc;
use orangedeck_domain::{CodexThread, CodexThreadStatus, ThreadObservation, TurnChanges};
use orangedeck_infra::file_capture::FileCapture;
use serde::{Deserialize, Serialize};

type Key = (String, String, String);
const MAX_TURNS: usize = 16;
const MAX_CACHE: u64 = 3 * 1024 * 1024;

struct Pending {
    cwd: String,
    root: PathBuf,
    before: Option<FileCapture>,
    ambiguous: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct CapturedTurn {
    thread_id: String,
    turn_id: String,
    cwd: String,
    changes: TurnChanges,
    complete: bool,
    #[serde(default)]
    saw_tool: bool,
    #[serde(default)]
    captured_at: chrono::DateTime<Utc>,
    #[serde(skip)]
    started: bool,
}

#[derive(Default)]
struct State {
    pending: HashMap<Key, Pending>,
    turns: VecDeque<CapturedTurn>,
}

pub struct FileHooks {
    state: Mutex<State>,
    path: PathBuf,
}

impl FileHooks {
    pub fn new(path: PathBuf) -> Self {
        let load = || -> io::Result<VecDeque<CapturedTurn>> {
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(orangedeck_infra::file_capture::private_read_flags())
                .open(&path)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.len() > MAX_CACHE {
                return Err(io::Error::other("invalid file-change cache"));
            }
            let mut bytes = Vec::new();
            file.take(MAX_CACHE + 1).read_to_end(&mut bytes)?;
            let turns: VecDeque<CapturedTurn> = serde_json::from_slice(&bytes)?;
            if bytes.len() > usize::try_from(MAX_CACHE).unwrap_or(0)
                || turns.len() > MAX_TURNS
                || turns.iter().any(|turn| {
                    turn.changes.files.len() > 64
                        || turn.changes.files.iter().map(change_bytes).sum::<usize>() > 65_536
                })
            {
                return Err(io::Error::other("oversized file-change cache"));
            }
            Ok(turns)
        };
        Self {
            state: Mutex::new(State {
                turns: load().unwrap_or_default(),
                ..State::default()
            }),
            path,
        }
    }

    pub fn lifecycle(&self, thread: &str, turn: &str, cwd: &str, started: bool) {
        let mut state = self.state.lock().expect("file hook lock");
        if started {
            let record = record(&mut state, thread, turn, cwd);
            record.started = true;
            record.complete = false;
        } else {
            let unfinished = state
                .pending
                .keys()
                .any(|key| key.0 == thread && key.1 == turn);
            state
                .pending
                .retain(|key, _| key.0 != thread || key.1 != turn);
            let record = record(&mut state, thread, turn, cwd);
            record.changes.truncated |= unfinished;
            record.complete = record.started && !record.changes.truncated;
        }
        if !started {
            // Save only at turn end, without source snapshots, commands or tool output.
            // Atomic replacement keeps a previous complete cache on write failure.
            let _ = self.save(&state.turns);
        }
    }

    pub fn before(&self, thread: &str, turn: &str, tool: &str, cwd: &str) {
        let key = (thread.to_owned(), turn.to_owned(), tool.to_owned());
        let root = PathBuf::from(cwd)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(cwd));
        {
            let mut state = self.state.lock().expect("file hook lock");
            let captured = record(&mut state, thread, turn, cwd);
            captured.complete = false;
            captured.saw_tool = true;
            if state.pending.len() >= 4 || state.pending.contains_key(&key) {
                record(&mut state, thread, turn, cwd).changes.truncated = true;
                return;
            }
            let mut ambiguous = false;
            for (other, pending) in &mut state.pending {
                if (pending.root.starts_with(&root) || root.starts_with(&pending.root))
                    && (other.0 != thread || other.1 != turn)
                {
                    pending.ambiguous = true;
                    ambiguous = true;
                }
            }
            state.pending.insert(
                key.clone(),
                Pending {
                    cwd: cwd.to_owned(),
                    root,
                    before: None,
                    ambiguous,
                },
            );
        }
        let before = FileCapture::before(cwd)
            .inspect_err(|error| {
                tracing::warn!(%error, "Could not start file event observation");
            })
            .ok();
        let mut state = self.state.lock().expect("file hook lock");
        if before.is_none() {
            record(&mut state, thread, turn, cwd).changes.truncated = true;
        }
        if let Some(pending) = state.pending.get_mut(&key) {
            pending.before = before;
        }
    }

    pub fn after(&self, thread: &str, turn: &str, tool: &str, cwd: &str) {
        let key = (thread.to_owned(), turn.to_owned(), tool.to_owned());
        let before = {
            let mut state = self.state.lock().expect("file hook lock");
            state
                .pending
                .get_mut(&key)
                .filter(|pending| pending.cwd == cwd && !pending.ambiguous)
                .and_then(|pending| pending.before.take())
        };
        let changes = before.and_then(|capture| {
            capture
                .finish()
                .inspect_err(|error| {
                    tracing::warn!(%error, "Could not finish file event observation");
                })
                .ok()
        });
        let mut state = self.state.lock().expect("file hook lock");
        let ambiguous = state
            .pending
            .remove(&key)
            .is_some_and(|pending| pending.ambiguous || pending.cwd != cwd);
        let record = record(&mut state, thread, turn, cwd);
        record.saw_tool = true;
        record.captured_at = Utc::now();
        if let Some(changes) = changes.filter(|_| !ambiguous) {
            merge_changes(&mut record.changes, &changes);
        } else {
            record.changes.truncated = true;
        }
    }

    pub fn enrich(&self, thread: &mut CodexThread) {
        let turn_id = thread
            .active_turn_id
            .as_deref()
            .or_else(|| {
                thread
                    .activity
                    .as_ref()
                    .map(|activity| activity.turn_id.as_str())
            })
            .or_else(|| {
                thread
                    .observation
                    .as_ref()
                    .and_then(|observation| observation.turn_id.as_deref())
            });
        let state = self.state.lock().expect("file hook lock");
        let Some(record) = state.turns.iter().find(|record| {
            record.saw_tool
                && record.thread_id == thread.id
                && Some(record.turn_id.as_str()) == turn_id
                && record.cwd == thread.cwd
        }) else {
            return;
        };
        if thread
            .observation
            .as_ref()
            .is_some_and(|value| value.turn_id.as_deref() != turn_id)
        {
            return;
        }
        let observation = thread.observation.get_or_insert_with(|| ThreadObservation {
            turn_id: Some(record.turn_id.clone()),
            latest_user_prompt: None,
            latest_codex_reply: None,
            changes: None,
            last_turn_status: if record.complete {
                CodexThreadStatus::Completed
            } else {
                thread.status
            },
            model: None,
            observed_at: Utc::now(),
        });
        observation.observed_at = observation.observed_at.max(record.captured_at);
        let changes = observation.changes.get_or_insert_with(TurnChanges::default);
        // Captured changes supersede missing history, but keep unrelated structured edits.
        for file in &record.changes.files {
            if let Some(old) = changes.files.iter_mut().find(|old| {
                let path = std::path::Path::new(&old.path);
                path.strip_prefix(&thread.cwd).unwrap_or(path) == std::path::Path::new(&file.path)
            }) {
                if old.diff.is_empty() {
                    *old = file.clone();
                }
            } else if changes.files.len() < 64 {
                changes.files.push(file.clone());
            }
        }
        changes.truncated = !record.complete
            || record.changes.truncated
            || changes.files.iter().any(|file| file.truncated);
        let mut remaining = 65_536;
        for file in &mut changes.files {
            bound_change(file, &mut remaining);
            changes.truncated |= file.truncated;
        }
        if let Some(activity) = &thread.activity {
            observation.last_turn_status = activity.status;
        }
    }

    fn save(&self, turns: &VecDeque<CapturedTurn>) -> io::Result<()> {
        let bytes = serde_json::to_vec(turns)?;
        if bytes.len() > usize::try_from(MAX_CACHE).unwrap_or(0) {
            return Err(io::Error::other("file cache full"));
        }
        let temp = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp)?;
            file.write_all(&bytes)?;
            fs::rename(&temp, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

fn record<'a>(state: &'a mut State, thread: &str, turn: &str, cwd: &str) -> &'a mut CapturedTurn {
    if let Some(index) = state.turns.iter().position(|record| {
        record.thread_id == thread && record.turn_id == turn && record.cwd == cwd
    }) {
        return &mut state.turns[index];
    }
    if state.turns.len() >= MAX_TURNS {
        state.turns.pop_front();
    }
    state.turns.push_back(CapturedTurn {
        thread_id: thread.to_owned(),
        turn_id: turn.to_owned(),
        cwd: cwd.to_owned(),
        changes: TurnChanges::default(),
        complete: false,
        saw_tool: false,
        captured_at: Utc::now(),
        started: false,
    });
    state.turns.back_mut().expect("inserted turn")
}

fn change_bytes(change: &orangedeck_domain::CodeChange) -> usize {
    change.diff.len() + change.content.as_ref().map_or(0, String::len)
}

fn bound_change(change: &mut orangedeck_domain::CodeChange, remaining: &mut usize) {
    let mut budget = (*remaining).min(16_384);
    for text in std::iter::once(&mut change.diff).chain(change.content.iter_mut()) {
        let mut length = text.len().min(budget);
        while !text.is_char_boundary(length) {
            length -= 1;
        }
        change.truncated |= length < text.len();
        text.truncate(length);
        budget -= length;
        *remaining -= length;
    }
}

fn merge_changes(existing: &mut TurnChanges, incoming: &TurnChanges) {
    existing.truncated |= incoming.truncated;
    for change in &incoming.files {
        if let Some(old) = existing
            .files
            .iter_mut()
            .find(|file| file.path == change.path)
        {
            // A later event replaces the current preview, not an invented diff.
            let was_added = old.kind == orangedeck_domain::CodeChangeKind::Added;
            *old = change.clone();
            if was_added && old.kind == orangedeck_domain::CodeChangeKind::Modified {
                old.kind = orangedeck_domain::CodeChangeKind::Added;
            }
        } else if existing.files.len() < 64 {
            existing.files.push(change.clone());
        } else {
            existing.truncated = true;
        }
    }
    let mut remaining = 65_536;
    for file in &mut existing.files {
        bound_change(file, &mut remaining);
        existing.truncated |= file.truncated;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn thread(cwd: &str, id: &str, turn: &str) -> CodexThread {
        serde_json::from_value(
            serde_json::json!({"id":id, "cwd":cwd, "title":"Fixture", "preview":"",
            "status":"completed", "ownership":"external_read_only", "updated_at":1,
            "observation":{"turn_id":turn, "last_turn_status":"completed", "observed_at":Utc::now(),
                "changes":{"files":[], "truncated":true}}}),
        )
        .unwrap()
    }

    #[test]
    fn captured_edits_survive_empty_history_refresh_and_connector_restart_for_exact_turn_only() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir(&root).unwrap();
        let cwd = root.to_str().unwrap();
        let path = directory.path().join("file-changes.json");
        let hooks = FileHooks::new(path.clone());
        hooks.lifecycle("s", "t", cwd, true);
        hooks.before("s", "t", "call", cwd);
        fs::write(root.join("codex_approval_test.py"), "print('new')\n").unwrap();
        hooks.after("s", "t", "call", cwd);
        hooks.lifecycle("s", "t", cwd, false);
        for source in [&hooks, &FileHooks::new(path.clone())] {
            for _ in 0..2 {
                let mut thread = thread(cwd, "s", "t");
                source.enrich(&mut thread);
                let changes = thread.observation.unwrap().changes.unwrap();
                assert_eq!(changes.files[0].path, "codex_approval_test.py");
                assert_eq!(changes.files[0].content.as_deref(), Some("print('new')\n"));
                assert_eq!(changes.truncated, !cfg!(target_os = "macos"));
            }
            for (id, turn) in [("s", "later"), ("other", "t")] {
                let mut thread = thread(cwd, id, turn);
                source.enrich(&mut thread);
                assert!(
                    thread
                        .observation
                        .unwrap()
                        .changes
                        .unwrap()
                        .files
                        .is_empty()
                );
            }
        }
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn latest_event_preview_survives_refresh_without_replacing_official_codex_diff() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir(&root).unwrap();
        let cwd = root.to_str().unwrap();
        let hooks = FileHooks::new(directory.path().join("file-changes.json"));
        hooks.lifecycle("s", "t", cwd, true);
        for (call, contents) in [("one", "first contents"), ("two", "latest contents")] {
            hooks.before("s", "t", call, cwd);
            fs::write(root.join("file.py"), contents).unwrap();
            hooks.after("s", "t", call, cwd);
        }
        hooks.lifecycle("s", "t", cwd, false);
        let mut captured = thread(cwd, "s", "t");
        hooks.enrich(&mut captured);
        let changes = captured
            .observation
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap();
        assert_eq!(changes.files.len(), 1);
        assert_eq!(changes.files[0].content.as_deref(), Some("latest contents"));
        assert!(changes.files[0].diff.is_empty());
        let official = serde_json::from_value(serde_json::json!({
            "path": root.join("file.py"), "previous_path":null, "kind":"modified",
            "first_line":7, "diff":"@@ -7 +7 @@\n-old\n+new", "truncated":false
        }))
        .unwrap();
        captured
            .observation
            .as_mut()
            .unwrap()
            .changes
            .as_mut()
            .unwrap()
            .files = vec![official];
        hooks.enrich(&mut captured);
        let file = &captured
            .observation
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .files[0];
        assert_eq!(file.first_line, 7);
        assert!(file.diff.contains("-old\n+new"));
        assert!(file.content.is_none());
    }

    #[test]
    fn parallel_conversations_in_overlapping_roots_do_not_claim_each_others_changes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        let hooks = FileHooks::new(directory.path().join("files.json"));
        for (id, cwd) in [
            ("a", root.to_str().unwrap()),
            ("b", child.to_str().unwrap()),
        ] {
            hooks.lifecycle(id, "t", cwd, true);
            hooks.before(id, "t", "call", cwd);
        }
        fs::write(child.join("ambiguous.txt"), "cannot attribute\n").unwrap();
        for (id, cwd) in [
            ("a", root.to_str().unwrap()),
            ("b", child.to_str().unwrap()),
        ] {
            hooks.after(id, "t", "call", cwd);
            hooks.lifecycle(id, "t", cwd, false);
            let mut thread = thread(cwd, id, "t");
            hooks.enrich(&mut thread);
            let changes = thread.observation.unwrap().changes.unwrap();
            assert!(changes.files.is_empty() && changes.truncated);
        }
    }

    #[test]
    fn missing_before_hook_and_unfinished_or_over_capacity_tools_never_claim_no_changes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir(&root).unwrap();
        let cwd = root.to_str().unwrap();
        for mode in ["missing", "unfinished", "capacity"] {
            let hooks = FileHooks::new(directory.path().join(format!("{mode}.json")));
            hooks.lifecycle("s", "t", cwd, true);
            if mode == "missing" {
                hooks.after("s", "t", "call", cwd);
            } else if mode == "unfinished" {
                hooks.before("s", "t", "call", cwd);
            } else {
                for index in 0..5 {
                    hooks.before("s", "t", &index.to_string(), cwd);
                }
            }
            hooks.lifecycle("s", "t", cwd, false);
            let mut thread = thread(cwd, "s", "t");
            hooks.enrich(&mut thread);
            assert!(
                thread.observation.unwrap().changes.unwrap().truncated,
                "{mode}"
            );
        }
    }

    #[test]
    fn completed_read_only_tool_has_no_changed_files() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().to_str().unwrap();
        fs::write(directory.path().join("dirty.txt"), "existing user change").unwrap();
        let hooks = FileHooks::new(directory.path().join("files.json"));
        hooks.lifecycle("s", "t", cwd, true);
        hooks.before("s", "t", "call", cwd);
        hooks.after("s", "t", "call", cwd);
        hooks.lifecycle("s", "t", cwd, false);
        let mut thread = thread(cwd, "s", "t");
        hooks.enrich(&mut thread);
        let changes = thread.observation.unwrap().changes.unwrap();
        assert!(changes.files.is_empty());
        assert_eq!(changes.truncated, !cfg!(target_os = "macos"));
    }
}
