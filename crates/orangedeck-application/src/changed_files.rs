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
