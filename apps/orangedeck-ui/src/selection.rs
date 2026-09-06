use std::collections::BTreeMap;

use orangedeck_protocol::{CodexThreadDto, CodexThreadStatusDto, SnapshotDto};

#[derive(Clone, Debug)]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub conversations: usize,
    pub active: usize,
    pub waiting: usize,
    pub updated_at: i64,
}

/// Mac paths are identities. Never resolve them against the Ally filesystem or merge by basename.
pub fn project_path(cwd: &str) -> String {
    let parts: Vec<_> = cwd
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    format!(
        "{}{}",
        if cwd.starts_with('/') { "/" } else { "" },
        parts.join("/")
    )
}

pub fn turn_id(thread: &CodexThreadDto) -> Option<&str> {
    thread
        .active_turn_id
        .as_deref()
        .or_else(|| thread.activity.as_ref().map(|value| value.turn_id.as_str()))
        .or_else(|| {
            thread
                .observation
                .as_ref()
                .and_then(|value| value.turn_id.as_deref())
        })
        .or_else(|| {
            thread
                .live_usage
                .as_ref()
                .and_then(|value| value.turn_id.as_deref())
        })
}

pub fn updated_at(thread: &CodexThreadDto) -> i64 {
    thread
        .updated_at
        .max(
            thread
                .live_usage
                .as_ref()
                .map_or(0, |value| value.updated_at.timestamp()),
        )
        .max(
            thread
                .activity
                .as_ref()
                .map_or(0, |value| value.updated_at.timestamp()),
        )
}

/// All tabs use the same current-turn text. A stored name/first preview is never a latest question.
pub fn latest_prompt(thread: &CodexThreadDto) -> Option<&str> {
    let current = turn_id(thread);
    let activity = thread
        .activity
        .as_ref()
        .filter(|value| Some(value.turn_id.as_str()) == current);
    let observation = thread
        .observation
        .as_ref()
        .filter(|value| value.turn_id.as_deref() == current);
    let prompt = activity.and_then(|value| value.latest_user_prompt.as_deref());
    if prompt.is_some()
        && observation.is_none_or(|value| {
            activity.is_some_and(|activity| activity.updated_at > value.observed_at)
        })
    {
        return prompt;
    }
    observation
        .and_then(|value| value.latest_user_prompt.as_deref())
        .or(prompt)
        .filter(|text| !text.trim().is_empty())
}

pub fn status(
    thread: &CodexThreadDto,
    snapshot: &SnapshotDto,
    connected: bool,
    now: i64,
) -> CodexThreadStatusDto {
    if !connected
        || snapshot.codex.connection.state
            != orangedeck_protocol::CodexConnectionStateDto::Connected
    {
        return CodexThreadStatusDto::Unknown;
    }
    if snapshot.codex.pending_approvals.iter().any(|request| {
        request.thread_id.as_deref() == Some(&thread.id)
            && request
                .turn_id
                .as_deref()
                .is_none_or(|id| turn_id(thread).is_none_or(|turn| turn == id))
    }) {
        return CodexThreadStatusDto::WaitingApproval;
    }
    if let Some(activity) = &thread.activity
        && (-5..=20).contains(&(now - activity.observed_at.timestamp()))
        && (-5..=120).contains(&(now - activity.updated_at.timestamp()))
    {
        return activity.status;
    }
    if let Some(usage) = &thread.live_usage
        && (-5..=20).contains(&(now - usage.observed_at.timestamp()))
        && (-5..=120).contains(&(now - usage.updated_at.timestamp()))
    {
        return usage.status;
    }
    if thread.ownership == orangedeck_protocol::ThreadOwnershipDto::OrangeDeck
        && (-5..=120).contains(&(now - thread.updated_at))
    {
        return thread.status;
    }
    CodexThreadStatusDto::Unknown
}

pub fn projects(snapshot: &SnapshotDto, connected: bool, now: i64) -> Vec<ProjectSummary> {
    let mut groups: BTreeMap<String, ProjectSummary> = BTreeMap::new();
    for thread in &snapshot.codex.threads {
        if !thread.cwd.starts_with('/') {
            continue;
        }
        let path = project_path(&thread.cwd);
        let project = groups
            .entry(path.clone())
            .or_insert_with(|| ProjectSummary {
                name: path
                    .rsplit('/')
                    .find(|name| !name.is_empty())
                    .unwrap_or("/")
                    .to_owned(),
                path,
                conversations: 0,
                active: 0,
                waiting: 0,
                updated_at: 0,
            });
        let state = status(thread, snapshot, connected, now);
        project.conversations += 1;
        project.active += usize::from(matches!(
            state,
            CodexThreadStatusDto::Working | CodexThreadStatusDto::WaitingApproval
        ));
        project.waiting += usize::from(state == CodexThreadStatusDto::WaitingApproval);
        project.updated_at = project.updated_at.max(updated_at(thread));
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        (b.waiting > 0, b.active > 0, b.updated_at)
            .cmp(&(a.waiting > 0, a.active > 0, a.updated_at))
            .then_with(|| a.path.cmp(&b.path))
    });
    groups
}

#[derive(Default)]
pub struct Selection {
    pub project: Option<String>,
    pub thread: Option<String>,
    pub follow_latest: bool,
}

impl Selection {
    pub fn threads<'a>(&self, snapshot: &'a SnapshotDto) -> Vec<&'a CodexThreadDto> {
        let mut threads: Vec<_> = snapshot
            .codex
            .threads
            .iter()
            .filter(|thread| self.project.as_deref() == Some(project_path(&thread.cwd).as_str()))
            .collect();
        threads.sort_by(|a, b| {
            updated_at(b)
                .cmp(&updated_at(a))
                .then_with(|| a.id.cmp(&b.id))
        });
        threads
    }

    pub fn selected<'a>(&self, snapshot: &'a SnapshotDto) -> Option<&'a CodexThreadDto> {
        snapshot.codex.threads.iter().find(|thread| {
            self.thread.as_deref() == Some(&thread.id)
                && self.project.as_deref() == Some(project_path(&thread.cwd).as_str())
        })
    }

    pub fn select_project(
        &mut self,
        path: &str,
        snapshot: &SnapshotDto,
        connected: bool,
        now: i64,
    ) {
        self.project = Some(project_path(path));
        self.thread = None;
        self.follow_latest = true;
        self.reconcile(snapshot, connected, now);
    }

    pub fn reconcile(&mut self, snapshot: &SnapshotDto, connected: bool, now: i64) {
        if self.project.is_none() {
            self.project = projects(snapshot, connected, now)
                .first()
                .map(|project| project.path.clone());
            self.follow_latest = true;
        }
        if self.follow_latest || self.selected(snapshot).is_none() {
            self.thread = self
                .threads(snapshot)
                .into_iter()
                .next()
                .map(|thread| thread.id.clone());
        }
    }

    pub fn change_thread(&mut self, delta: isize, snapshot: &SnapshotDto) {
        let threads = self.threads(snapshot);
        if threads.is_empty() {
            return;
        }
        let index = threads
            .iter()
            .position(|thread| self.thread.as_deref() == Some(&thread.id))
            .unwrap_or(0);
        let next = if delta < 0 {
            (index + threads.len() - 1) % threads.len()
        } else {
            (index + 1) % threads.len()
        };
        self.thread = Some(threads[next].id.clone());
        self.follow_latest = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn snapshot() -> SnapshotDto {
        serde_json::from_value(serde_json::json!({
            "host":{"name":"TEST MAC","os":"test","architecture":"test","address":null,"state":"connected","tailscale":false,"latency_ms":null,"last_seen":chrono::Utc::now()},
            "projects":[],"selected_project_id":null,"jobs":[],"git":[],"system":{},"activity":[],
            "codex":{"connection":{"state":"connected","version":null,"compatible":true,"message":null},"threads":[],"pending_approvals":[],"limits":null,"supported_features":[]}
        })).unwrap()
    }

    fn thread(id: &str, cwd: &str, updated: i64) -> CodexThreadDto {
        serde_json::from_value(serde_json::json!({"id":id,"cwd":cwd,"title":id,"preview":"","status":"not_loaded","ownership":"external_read_only","updated_at":updated})).unwrap()
    }

    #[test]
    fn paths_do_not_merge_sibling_names_and_selection_survives_reorder_and_activity_elsewhere() {
        let mut snapshot = snapshot();
        snapshot.codex.threads = vec![
            thread("a-old", "/Users/mac/a/app", 1),
            thread("a-new", "/Users/mac/a/app/", 4),
            thread("b", "/Users/mac/b/app", 3),
        ];
        let mut selection = Selection::default();
        selection.reconcile(&snapshot, true, 10);
        assert_eq!(projects(&snapshot, true, 10).len(), 2);
        assert_eq!(selection.thread.as_deref(), Some("a-new"));
        snapshot.codex.threads.reverse();
        snapshot.codex.threads[0].updated_at = 100;
        selection.reconcile(&snapshot, true, 100);
        assert_eq!(selection.thread.as_deref(), Some("a-new"));
        selection.change_thread(1, &snapshot);
        assert_eq!(selection.thread.as_deref(), Some("a-old"));
        selection.select_project("/Users/mac/b/app", &snapshot, true, 100);
        assert_eq!(selection.threads(&snapshot).len(), 1);
        assert_eq!(selection.thread.as_deref(), Some("b"));
        snapshot.codex.threads.clear();
        selection.reconcile(&snapshot, true, 100);
        assert!(selection.thread.is_none());
    }

    #[test]
    fn all_tabs_use_current_prompt_and_recent_auto_matches_the_first_conversation() {
        let mut snapshot = crate::test_support::snapshot();
        let mut current = snapshot.codex.threads[0].clone();
        assert_eq!(latest_prompt(&current), Some("CURRENT QUESTION"));
        let mut older = current.clone();
        older.id = "older-waiting".to_owned();
        older.live_usage = None;
        older.updated_at -= 10;
        older.ownership = orangedeck_protocol::ThreadOwnershipDto::OrangeDeck;
        older.status = CodexThreadStatusDto::WaitingApproval;
        snapshot.codex.threads = vec![older, current.clone()];
        let mut selection = Selection::default();
        selection.reconcile(&snapshot, true, chrono::Utc::now().timestamp());
        assert_eq!(selection.thread.as_deref(), Some("a"));
        assert_eq!(selection.threads(&snapshot)[0].id, "a");
        current.active_turn_id = Some("next-turn".to_owned());
        assert_eq!(latest_prompt(&current), None);
        current.activity = Some(
            serde_json::from_value(serde_json::json!({
                "turn_id":"next-turn","status":"working","started_at":null,
                "updated_at":chrono::Utc::now(),"observed_at":chrono::Utc::now(),
                "user_input":null,"latest_user_prompt":"NEW QUESTION"
            }))
            .unwrap(),
        );
        assert_eq!(latest_prompt(&current), Some("NEW QUESTION"));
    }

    #[test]
    fn activity_counts_require_recent_observation_and_connection() {
        let mut snapshot = snapshot();
        let mut active = thread("active", "/a", 1);
        active.activity = Some(orangedeck_protocol::ThreadActivityDto {
            turn_id: "turn".to_owned(),
            status: CodexThreadStatusDto::Working,
            started_at: None,
            updated_at: chrono::DateTime::from_timestamp(100, 0).unwrap(),
            observed_at: chrono::DateTime::from_timestamp(100, 0).unwrap(),
            user_input: None,
            latest_user_prompt: None,
        });
        snapshot.codex.threads = vec![thread("recent", "/b", 110), active];
        assert_eq!(projects(&snapshot, true, 110)[0].path, "/a");
        assert_eq!(projects(&snapshot, true, 110)[0].active, 1);
        assert_eq!(
            projects(&snapshot, false, 110)
                .iter()
                .map(|p| p.active)
                .sum::<usize>(),
            0
        );
        assert_eq!(
            projects(&snapshot, true, 125)
                .iter()
                .map(|p| p.active)
                .sum::<usize>(),
            0
        );
    }
}
