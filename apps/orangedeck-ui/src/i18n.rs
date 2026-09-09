//! Per-window language selection. Work content and unknown host text stay unchanged.
use eframe::egui;
pub use orangedeck_domain::UiLanguage as Language;
use std::borrow::Cow;

pub fn language(ctx: &egui::Context) -> Language {
    ctx.data(|data| data.get_temp(egui::Id::new("ui_language")))
        .unwrap_or_default()
}

pub fn set_language(ctx: &egui::Context, language: Language) {
    if self::language(ctx) == language {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ui_language"), language));
    ctx.request_repaint();
}

/// Translate only known OrangeDeck status labels, including older Connectors.
/// Unknown errors and all work content keep their original text and details.
pub fn status_message(language: Language, message: &str) -> Cow<'_, str> {
    let (prefix, body) = message.split_once(": ").map_or(("", message), |(_, body)| {
        (&message[..message.len() - body.len()], body)
    });
    let (korean, english) = match body {
        "Connecting to OrangeDeck Connector" => ("Mac 통신 모듈에 연결 중", body),
        "선택을 전송했습니다. Mac의 처리 결과를 기다립니다." => (
            body,
            "Decision sent. Waiting for the Mac to finish processing.",
        ),
        "대화 기록 읽음 · 외부 세션 제어 없음" => (
            body,
            "Conversation history refreshed; external session unchanged.",
        ),
        "선택한 대화를 확인합니다 / Watching selected conversations" => (
            "선택한 대화를 확인합니다",
            "Watching selected conversations",
        ),
        "선택을 Mac Codex에 전송했습니다" => {
            (body, "Decision sent to Codex on your Mac.")
        }
        "이 승인 요청은 이미 종료되었습니다" => {
            (body, "This approval request has already ended.")
        }
        "통신이 끊겨 요청을 보내지 못했습니다. 다시 연결한 뒤 재시도하세요 / Connector is disconnected; command was not sent"
        | "Connector is disconnected; command was not sent" => (
            "통신이 끊겨 요청을 보내지 못했습니다. 다시 연결한 뒤 재시도하세요.",
            "Connector is disconnected; command was not sent. Reconnect and try again.",
        ),
        "OrangeDeck network worker has stopped" => (
            "통신 처리가 중지되었습니다. 앱을 다시 실행하세요.",
            "OrangeDeck network worker has stopped. Restart the app.",
        ),
        "Mac Codex 승인 요청" => (body, "Mac Codex approval request"),
        "Codex 판단 요청" => (body, "Codex needs your input"),
        "Codex 응답 완료" => (body, "Codex reply completed"),
        "Codex 작업 오류" => (body, "Codex task failed"),
        "Codex 작업 중단" => (body, "Codex task stopped"),
        "승인 요청 종료" => (body, "Approval request ended"),
        _ => return Cow::Borrowed(message),
    };
    let translated = language.text(korean, english);
    if prefix.is_empty() {
        Cow::Borrowed(translated)
    } else {
        Cow::Owned(format!("{prefix}{translated}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_is_window_local_and_changes_without_restarting() {
        let first = egui::Context::default();
        let second = egui::Context::default();
        set_language(&first, Language::English);
        assert_eq!(language(&first), Language::English);
        assert_eq!(language(&second), Language::Korean);
        set_language(&first, Language::Korean);
        assert_eq!(language(&first), Language::Korean);
    }

    #[test]
    fn known_connector_statuses_translate_without_stripping_unknown_error_details() {
        assert_eq!(
            status_message(
                Language::English,
                "approval_not_pending: 이 승인 요청은 이미 종료되었습니다"
            ),
            "approval_not_pending: This approval request has already ended."
        );
        for text in [
            "custom_error: 경로 / 내용: 원문을 유지하세요",
            "Run this exact command: printf '작업 폴더: /tmp/example / details'",
            "unrecognized failure: line 42\nfull error details",
        ] {
            assert_eq!(status_message(Language::English, text), text);
            assert_eq!(status_message(Language::Korean, text), text);
        }
    }
}
