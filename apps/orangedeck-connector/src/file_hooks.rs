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
            let mut ambiguous = false;
            for (other, pending) in &mut state.pending {
                if (pending.root.starts_with(&root) || root.starts_with(&pending.root))
                    && (other.0 != thread || other.1 != turn)
                {
                    pending.ambiguous = true;
                    ambiguous = true;
                }
            }
            // Even an unobserved tool can invalidate another conversation's capture.
            if state.pending.len() >= 4 || state.pending.contains_key(&key) {
                if let Some(pending) = state.pending.get_mut(&key) {
                    pending.ambiguous = true;
                }
                record(&mut state, thread, turn, cwd).changes.truncated = true;
                return;
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
        // Stop/Interrupt may have removed the tool while its event flush was running.
        let ambiguous = state
            .pending
            .remove(&key)
            .is_none_or(|pending| pending.ambiguous || pending.cwd != cwd);
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
                } else {
                    // Keep the Codex diff and its line, alongside the last observed text.
                    old.content.clone_from(&file.content);
                    old.truncated |= file.truncated;
                    if file.kind == orangedeck_domain::CodeChangeKind::Deleted
                        || old.kind == orangedeck_domain::CodeChangeKind::Deleted
                    {
                        old.kind = file.kind;
                    }
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
