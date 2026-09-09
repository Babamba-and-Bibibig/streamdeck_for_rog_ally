//! A small read-only adapter for Codex's local rollout format (not a stable API).
//! Only app-server-listed IDs/paths below the local sessions directory are accepted.
//! Exports bounded counters, lifecycle/questions and successful, turn-bound file edits.
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, Utc};
use orangedeck_domain::{
    CodexEvent, CodexThreadStatus, LiveTokenUsage, ThreadActivity, TokenUsage,
};
use serde_json::Value;

const MAX_READ: u64 = 2 * 1024 * 1024;
const MAX_HEADER: u64 = 256 * 1024;
const MAX_LINE: usize = 256 * 1024;
const MAX_THREADS: usize = 100;
const MAX_SAMPLES: usize = 16;

pub(super) struct UsageLogReader {
    sessions: Option<PathBuf>,
    cursors: HashMap<String, Cursor>,
    events: Vec<CodexEvent>,
}

impl Default for UsageLogReader {
    fn default() -> Self {
        let codex_root = std::env::var_os("CODEX_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")));
        Self {
            sessions: codex_root.map(|root| root.join("sessions")),
            cursors: HashMap::new(),
            events: Vec::new(),
        }
    }
}

impl UsageLogReader {
    pub(super) fn forget(&mut self, id: &str) {
        self.cursors.remove(id);
    }
    pub(super) fn activity(&self, id: &str, now: DateTime<Utc>) -> Option<ThreadActivity> {
        self.cursors.get(id)?.activity(now)
    }

    pub(super) fn changes(
        &self,
        id: &str,
        turn_id: &str,
    ) -> Option<orangedeck_domain::TurnChanges> {
        let cursor = self.cursors.get(id)?;
        if cursor.turn_id.as_deref() != Some(turn_id) {
            return None;
        }
        cursor.edits.changes()
    }

    pub(super) fn drain_events(&mut self) -> impl Iterator<Item = CodexEvent> + '_ {
        self.events.drain(..)
    }

    pub(super) fn read_listed(
        &mut self,
        listed: &[(String, PathBuf)],
        now: DateTime<Utc>,
    ) -> HashMap<String, LiveTokenUsage> {
        listed
            .iter()
            .take(MAX_THREADS)
            .filter_map(|(id, path)| {
                // Missing, rotated, incompatible or inaccessible logs are absent, never zero.
                let usage = self.read_one(id, path, now);
                if usage.is_err() {
                    self.cursors.remove(id);
                }
                usage.ok().flatten().map(|usage| (id.clone(), usage))
            })
            .collect()
    }

    pub(super) fn read_one(
        &mut self,
        id: &str,
        path: &Path,
        now: DateTime<Utc>,
    ) -> io::Result<Option<LiveTokenUsage>> {
        if uuid::Uuid::parse_str(id).is_err() || !path.is_absolute() {
            return Err(invalid_log());
        }
        let root = self
            .sessions
            .as_ref()
            .ok_or_else(invalid_log)?
            .canonicalize()?;
        let canonical = path.canonicalize()?;
        let name = canonical
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid_log)?;
        if !canonical.starts_with(&root)
            || !name.starts_with("rollout-")
            || !name.ends_with(&format!("-{id}.jsonl"))
        {
            return Err(invalid_log());
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let mut file = options.open(&canonical)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(invalid_log());
        }
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            (metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let identity = (0, 0);
        let modified = metadata.modified()?;
        let length = metadata.len();
        // Keep a few watched older threads alongside the recent catalog page.
        // Otherwise a pinned conversation beyond row 100 would be re-baselined every poll.
        if !self.cursors.contains_key(id)
            && self.cursors.len() >= MAX_THREADS + 8
            && let Some(oldest) = self
                .cursors
                .iter()
                .min_by_key(|(_, cursor)| cursor.last_read_at)
                .map(|(id, _)| id.clone())
        {
            self.cursors.remove(&oldest);
        }
        let cursor = self.cursors.entry(id.to_owned()).or_default();
        if cursor.path != canonical
            || cursor.identity != identity
            || length < cursor.offset
            || (length == cursor.offset && cursor.modified != Some(modified))
            || length.saturating_sub(cursor.offset) > MAX_READ
        {
            *cursor = Cursor::default();
        }
        let was_observed = !cursor.path.as_os_str().is_empty();
        if cursor.path.as_os_str().is_empty() {
            if !header_matches(&mut file, id)? {
                return Err(invalid_log());
            }
            cursor.path = canonical;
            cursor.identity = identity;
            cursor.offset = length.saturating_sub(MAX_READ);
            cursor.skip_partial = cursor.offset > 0;
        }
        if length > cursor.offset {
            file.seek(SeekFrom::Start(cursor.offset))?;
            let mut bytes = Vec::new();
            file.take(MAX_READ).read_to_end(&mut bytes)?;
            let mut consumed = 0;
            for (end, _) in bytes.iter().enumerate().filter(|(_, byte)| **byte == b'\n') {
                let line = &bytes[consumed..end];
                if cursor.skip_partial {
                    cursor.skip_partial = false;
                } else if line.len() <= MAX_LINE {
                    let previous_boundary = cursor.boundary_at;
                    let previous_input = cursor.input_call_id.clone();
                    cursor.consume(line, now);
                    if was_observed
                        && cursor.input_call_id != previous_input
                        && let Some(summary) = &cursor.user_input
                        && cursor.activity_at.is_some_and(|at| {
                            (-5..=120).contains(&now.signed_duration_since(at).num_seconds())
                        })
                        && self.events.len() < MAX_THREADS
                    {
                        self.events.push(CodexEvent::Activity {
                            thread_id: Some(id.to_owned()),
                            turn_id: cursor.turn_id.clone(),
                            kind: "user_input".to_owned(),
                            text: summary.clone(),
                        });
                    }
                    if was_observed
                        && cursor.boundary_at != previous_boundary
                        && cursor
                            .boundary_at
                            .is_some_and(|at| now.signed_duration_since(at).num_seconds() <= 120)
                        && let (Some(turn_id), Some(status)) = (&cursor.turn_id, cursor.status)
                        && matches!(
                            status,
                            CodexThreadStatus::Completed | CodexThreadStatus::Idle
                        )
                        && self.events.len() < MAX_THREADS
                    {
                        self.events.push(CodexEvent::TurnCompleted {
                            thread_id: id.to_owned(),
                            turn_id: turn_id.clone(),
                            status,
                        });
                    }
                }
                consumed = end + 1;
            }
            cursor.offset += u64::try_from(consumed).unwrap_or(MAX_READ);
        }
        cursor.modified = Some(modified);
        cursor.last_read_at = Some(now);
        Ok(cursor.usage(now))
    }
}

fn invalid_log() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "unavailable Codex session usage",
    )
}

fn header_matches(file: &mut File, id: &str) -> io::Result<bool> {
    let mut bytes = Vec::new();
    BufReader::new(file.take(MAX_HEADER)).read_until(b'\n', &mut bytes)?;
    if bytes.last() != Some(&b'\n') {
        return Ok(false);
    }
    Ok(serde_json::from_slice::<Value>(&bytes).is_ok_and(|value| {
        value.get("type").and_then(Value::as_str) == Some("session_meta")
            && value.pointer("/payload/id").and_then(Value::as_str) == Some(id)
    }))
}

#[derive(Default)]
struct Cursor {
    path: PathBuf,
    identity: (u64, u64),
    offset: u64,
    modified: Option<SystemTime>,
    skip_partial: bool,
    total: Option<TokenUsage>,
    baseline: Option<TokenUsage>,
    turn_id: Option<String>,
    status: Option<CodexThreadStatus>,
    last_request: Option<TokenUsage>,
    updated_at: Option<DateTime<Utc>>,
    samples: Vec<i64>,
    boundary_at: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
    activity_at: Option<DateTime<Utc>>,
    input_call_id: Option<String>,
    input_async: bool,
    user_input: Option<String>,
    latest_user_prompt: Option<String>,
    last_read_at: Option<DateTime<Utc>>,
    edits: super::recorded_edits::RecordedEdits,
}

impl Cursor {
    fn consume(&mut self, bytes: &[u8], now: DateTime<Utc>) {
        let Ok(record) = serde_json::from_slice::<Value>(bytes) else {
            return;
        };
        let Some(at) = record
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
        else {
            return;
        };
        if at > now + chrono::Duration::seconds(5) {
            return;
        }
        let Some(payload) = record.get("payload") else {
            return;
        };
        if self.activity_at.is_some_and(|previous| at < previous) {
            return;
        }
        self.activity_at = Some(at);
        if record.get("type").and_then(Value::as_str) == Some("response_item") {
            if self.turn_id.is_some() {
                self.edits.consume(payload);
            }
            self.consume_input(payload);
            return;
        }
        if record.get("type").and_then(Value::as_str) != Some("event_msg") {
            return;
        }
        match payload.get("type").and_then(Value::as_str) {
            Some("task_started") => {
                if self.boundary_at.is_some_and(|previous| at <= previous) {
                    return;
                }
                self.boundary_at = Some(at);
                self.started_at = Some(at);
                self.latest_user_prompt = None;
                self.edits = super::recorded_edits::RecordedEdits::default();
                self.user_input = None;
                self.input_call_id = None;
                self.baseline.clone_from(&self.total);
                self.turn_id = payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.status = Some(CodexThreadStatus::Working);
                self.last_request = None;
                self.updated_at = None;
                self.samples.clear();
            }
            Some("task_complete" | "turn_aborted") => {
                let id = payload.get("turn_id").and_then(Value::as_str);
                if (id.is_none() || self.turn_id.is_none() || id == self.turn_id.as_deref())
                    && self.boundary_at.is_none_or(|previous| at > previous)
                {
                    self.boundary_at = Some(at);
                    if self.turn_id.is_none() {
                        self.turn_id = id.map(str::to_owned);
                    }
                    self.status = Some(if payload["type"] == "task_complete" {
                        CodexThreadStatus::Completed
                    } else {
                        CodexThreadStatus::Idle
                    });
                    self.user_input = None;
                    self.input_call_id = None;
                }
            }
            Some("user_message") => {
                // Store only the explicit user message, never tool output or reasoning.
                self.latest_user_prompt = payload
                    .get("message")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| text.chars().take(4000).collect());
                self.user_input = None;
                self.input_call_id = None;
                if self.status == Some(CodexThreadStatus::WaitingApproval) {
                    self.status = Some(CodexThreadStatus::Working);
                }
            }
            Some("token_count") => self.consume_tokens(payload, at),
            _ => {}
        }
    }

    fn consume_input(&mut self, payload: &Value) {
        if self.turn_id.is_none()
            || !matches!(
                self.status,
                Some(CodexThreadStatus::Working | CodexThreadStatus::WaitingApproval)
            )
        {
            return;
        }
        match payload.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .rsplit('.')
                    .next()
                    .unwrap_or("");
                if !matches!(name, "request_user_input" | "request_user_input_async") {
                    return;
                }
                let Some(call_id) = payload.get("call_id").and_then(Value::as_str) else {
                    return;
                };
                let Some(args) = payload
                    .get("arguments")
                    .and_then(Value::as_str)
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                else {
                    return;
                };
                let Some(questions) = args.get("questions").and_then(Value::as_array) else {
                    return;
                };
                let text = questions
                    .iter()
                    .take(3)
                    .filter_map(|q| {
                        q.get("question")
                            .or_else(|| q.get("title"))
                            .and_then(Value::as_str)
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                if text.trim().is_empty() {
                    return;
                }
                self.user_input = Some(text.chars().take(4000).collect());
                self.input_call_id = Some(call_id.to_owned());
                self.input_async = name == "request_user_input_async";
                self.status = Some(CodexThreadStatus::WaitingApproval);
            }
            Some("function_call_output")
                if !self.input_async
                    && payload.get("call_id").and_then(Value::as_str)
                        == self.input_call_id.as_deref() =>
            {
                self.user_input = None;
                self.input_call_id = None;
                self.status = Some(CodexThreadStatus::Working);
            }
            _ => {}
        }
    }

    fn activity(&self, now: DateTime<Utc>) -> Option<ThreadActivity> {
        Some(ThreadActivity {
            turn_id: self.turn_id.clone()?,
            status: self.status?,
            started_at: self.started_at,
            updated_at: self.activity_at?,
            observed_at: now,
            user_input: self.user_input.clone(),
            latest_user_prompt: self.latest_user_prompt.clone(),
        })
    }

    fn consume_tokens(&mut self, payload: &Value, at: DateTime<Utc>) {
        let Some(info) = payload.get("info") else {
            return;
        };
        let Some(mut total) = info.get("total_token_usage").and_then(parse_usage) else {
            return;
        };
        let Some(mut last) = info.get("last_token_usage").and_then(parse_usage) else {
            return;
        };
        total.model_context_window = info.get("model_context_window").and_then(Value::as_i64);
        last.model_context_window = total.model_context_window;
        // Quota-only repeats and compaction/context estimates are not new consumption.
        if last.total_tokens == 0
            || self.total.as_ref() == Some(&total)
            || self.updated_at.is_some_and(|previous| at <= previous)
        {
            return;
        }
        if self
            .total
            .as_ref()
            .is_some_and(|previous| difference(&total, previous).is_none())
        {
            self.baseline = None;
        }
        self.total = Some(total);
        self.samples.push(last.total_tokens);
        if self.samples.len() > MAX_SAMPLES {
            self.samples.remove(0);
        }
        self.last_request = Some(last);
        self.updated_at = Some(at);
    }

    fn usage(&self, now: DateTime<Utc>) -> Option<LiveTokenUsage> {
        Some(LiveTokenUsage {
            turn_id: self.turn_id.clone(),
            turn_tokens: self
                .total
                .as_ref()
                .zip(self.baseline.as_ref())
                .and_then(|(total, base)| difference(total, base)),
            last_request: self.last_request.clone()?,
            recent_requests: self.samples.clone(),
            status: self.status.unwrap_or(CodexThreadStatus::Unknown),
            updated_at: self.updated_at?,
            observed_at: now,
        })
    }
}

fn parse_usage(value: &Value) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: value.get("input_tokens")?.as_i64()?,
        cached_input_tokens: value.get("cached_input_tokens")?.as_i64()?,
        output_tokens: value.get("output_tokens")?.as_i64()?,
        reasoning_output_tokens: value.get("reasoning_output_tokens")?.as_i64()?,
        total_tokens: value.get("total_tokens")?.as_i64()?,
        model_context_window: None,
    };
    valid_usage(&usage).then_some(usage)
}

fn valid_usage(usage: &TokenUsage) -> bool {
    usage.input_tokens >= 0
        && usage.output_tokens >= 0
        && usage.cached_input_tokens >= 0
        && usage.reasoning_output_tokens >= 0
        && usage.cached_input_tokens <= usage.input_tokens
        && usage.reasoning_output_tokens <= usage.output_tokens
        && usage.input_tokens.checked_add(usage.output_tokens) == Some(usage.total_tokens)
}

fn difference(total: &TokenUsage, base: &TokenUsage) -> Option<TokenUsage> {
    let usage = TokenUsage {
        input_tokens: total.input_tokens.checked_sub(base.input_tokens)?,
        cached_input_tokens: total
            .cached_input_tokens
            .checked_sub(base.cached_input_tokens)?,
        output_tokens: total.output_tokens.checked_sub(base.output_tokens)?,
        reasoning_output_tokens: total
            .reasoning_output_tokens
            .checked_sub(base.reasoning_output_tokens)?,
        total_tokens: total.total_tokens.checked_sub(base.total_tokens)?,
        model_context_window: total.model_context_window,
    };
    valid_usage(&usage).then_some(usage)
}
