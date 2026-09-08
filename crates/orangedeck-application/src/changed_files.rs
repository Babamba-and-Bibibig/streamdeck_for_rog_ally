//! Authorize editor navigation from a recorded change, never a client-provided arbitrary file.
use orangedeck_domain::{CodeChange, CodeChangeKind, CodexThread, DashboardState};

pub fn recorded_change<'a>(
    state: &'a DashboardState,
    thread_id: &str,
    turn_id: &str,
    path: &str,
) -> Result<(&'a CodexThread, &'a CodeChange), &'static str> {
    let thread = state
        .codex
        .threads
        .iter()
        .find(|thread| thread.id == thread_id)
        .ok_or("대화를 찾을 수 없습니다 / Conversation is unavailable")?;
    let observation = thread.observation.as_ref().filter(|observation| observation.turn_id.as_deref() == Some(turn_id))
        .ok_or("대화가 갱신되었습니다. 버튼을 다시 열어 주세요 / Conversation changed; reopen the button")?;
    let change = observation
        .changes
        .as_ref()
        .and_then(|changes| changes.files.iter().find(|change| change.path == path))
        .ok_or(
            "이 질의의 수정 파일 목록에 없는 파일입니다 / File is not a recorded edit in this turn",
        )?;
    if change.kind == CodeChangeKind::Deleted {
        return Err(
            "삭제한 파일은 목록에서 변경 내용을 확인하세요 / Deleted files can be reviewed in the change list",
        );
    }
    // Filesystem containment is checked separately against this conversation's canonical cwd.
    Ok((thread, change))
}

#[cfg(test)]
mod tests {
    use super::*;
    use orangedeck_domain::{
        CodexThreadStatus, Project, ProjectId, ThreadObservation, ThreadOwnership, TurnChanges,
    };

    #[test]
    fn navigation_requires_the_exact_thread_turn_and_recorded_file() {
        let project = Project {
            id: ProjectId::new("demo").unwrap(),
            name: "Demo".to_owned(),
            path: "/demo/project".into(),
            browser_url: None,
        };
        let mut state = DashboardState::new("Demo", vec![project]);
        state.codex.threads.push(CodexThread {
            id: "terminal-a".to_owned(),
            project_id: None,
            cwd: "/demo/project".to_owned(),
            title: "Demo".to_owned(),
            preview: String::new(),
            status: CodexThreadStatus::Completed,
            ownership: ThreadOwnership::ExternalReadOnly,
            updated_at: 1,
            active_turn_id: None,
            token_usage: None,
            live_usage: None,
            activity: None,
            observation: Some(ThreadObservation {
                turn_id: Some("turn-now".to_owned()),
                latest_user_prompt: None,
                latest_codex_reply: None,
                last_turn_status: CodexThreadStatus::Completed,
                model: None,
                observed_at: chrono::Utc::now(),
                changes: Some(TurnChanges {
                    files: vec![CodeChange {
                        path: "src/main.rs".to_owned(),
                        previous_path: None,
                        kind: CodeChangeKind::Modified,
                        first_line: 42,
                        diff: String::new(),
                        truncated: false,
                    }],
                    truncated: false,
                }),
            }),
        });
        assert_eq!(
            recorded_change(&state, "terminal-a", "turn-now", "src/main.rs")
                .unwrap()
                .1
                .first_line,
            42
        );
        for (thread, turn, path) in [
            ("other-terminal", "turn-now", "src/main.rs"),
            ("terminal-a", "turn-old", "src/main.rs"),
            ("terminal-a", "turn-now", "../private.txt"),
            ("terminal-a", "turn-now", "src/other.rs"),
        ] {
            assert!(recorded_change(&state, thread, turn, path).is_err());
        }
        state.codex.threads[0]
            .observation
            .as_mut()
            .unwrap()
            .changes
            .as_mut()
            .unwrap()
            .files[0]
            .kind = CodeChangeKind::Deleted;
        assert!(recorded_change(&state, "terminal-a", "turn-now", "src/main.rs").is_err());
    }
}
