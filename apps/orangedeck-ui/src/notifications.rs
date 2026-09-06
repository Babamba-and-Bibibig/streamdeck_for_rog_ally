use chrono::Utc;
use eframe::egui::{self, Color32, RichText};
use orangedeck_protocol::{
    ApprovalDecisionDto, ApprovalDto, CodexThreadDto, CodexThreadStatusDto, NotificationLevelDto,
    SnapshotDto,
};

use crate::{alerts::AlertCenter, theme};

pub fn level_color(level: NotificationLevelDto) -> Color32 {
    match level {
        NotificationLevelDto::Success => theme::GREEN,
        NotificationLevelDto::Warning => theme::YELLOW,
        NotificationLevelDto::Error => theme::RED,
        NotificationLevelDto::Info => theme::CYAN,
    }
}

pub fn approval_panel(
    ui: &mut egui::Ui,
    approval: &ApprovalDto,
    connected: bool,
    sending: bool,
    detailed: bool,
) -> Option<ApprovalDecisionDto> {
    let mut decision = None;
    theme::accent_panel(theme::YELLOW).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new("승인이 필요합니다")
                .size(18.0)
                .strong()
                .color(theme::YELLOW),
        );
        ui.add(egui::Label::new(RichText::new(&approval.summary).size(15.0)).wrap());
        if detailed && !approval.details.is_empty() {
            egui::ScrollArea::vertical()
                .id_salt(("approval_details", approval.id))
                .max_height(130.0)
                .show(ui, |ui| {
                    for detail in &approval.details {
                        ui.add(
                            egui::Label::new(RichText::new(detail).monospace().size(12.0)).wrap(),
                        );
                    }
                });
        }
        ui.add_space(7.0);
        ui.label(
            RichText::new(if !connected {
                "연결이 끊겼습니다. 다시 연결되면 요청 상태를 확인합니다."
            } else if sending {
                "선택 전송 중 · Mac의 처리 결과를 기다립니다"
            } else {
                "표시된 요청 한 건에 적용 · A 승인 / B 거부"
            })
            .size(12.0)
            .color(theme::MUTED),
        );
        ui.add_space(5.0);
        ui.add_enabled_ui(connected && !sending, |ui| {
            ui.horizontal(|ui| {
                let width = ((ui.available_width() - 12.0) / 2.0).min(260.0);
                if ui
                    .add_sized(
                        [width, 52.0],
                        egui::Button::new(
                            RichText::new("A  승인")
                                .size(19.0)
                                .strong()
                                .color(Color32::BLACK),
                        )
                        .fill(theme::GREEN),
                    )
                    .clicked_by(egui::PointerButton::Primary)
                {
                    decision = Some(ApprovalDecisionDto::Approve);
                }
                if ui
                    .add_sized(
                        [width, 52.0],
                        egui::Button::new(
                            RichText::new("B  거부")
                                .size(19.0)
                                .strong()
                                .color(theme::TEXT),
                        )
                        .fill(theme::RED),
                    )
                    .clicked_by(egui::PointerButton::Primary)
                {
                    decision = Some(ApprovalDecisionDto::Reject);
                }
            });
        });
    });
    decision
}

pub fn render(
    ui: &mut egui::Ui,
    snapshot: &SnapshotDto,
    thread: Option<&CodexThreadDto>,
    alerts: &mut AlertCenter,
    connected: bool,
) {
    let Some(thread) = thread else {
        theme::panel().show(ui, |ui| {
            ui.label(RichText::new("현재 질의를 기다리고 있습니다").size(25.0));
            ui.label("프로젝트들 탭에서 확인할 프로젝트를 선택하세요.");
        });
        return;
    };
    let now = Utc::now().timestamp();
    let state = crate::selection::status(thread, snapshot, connected, now);
    let unread = alerts.current_unread(thread);
    let latest = alerts
        .entries
        .iter()
        .find(|entry| crate::alerts::is_current(entry, thread))
        .cloned();
    let color = if state == CodexThreadStatusDto::WaitingApproval {
        theme::YELLOW
    } else {
        latest
            .as_ref()
            .map_or(theme::CYAN, |entry| level_color(entry.notification.level))
    };
    let label = if !connected {
        "연결 끊김 · 마지막 기록"
    } else if state == CodexThreadStatusDto::WaitingApproval {
        "사용자 판단 / 승인 대기"
    } else if let Some(entry) = &latest {
        entry.notification.title.as_str()
    } else {
        match state {
            CodexThreadStatusDto::Working => "Codex 작업 중",
            CodexThreadStatusDto::Completed => "Codex 응답 완료",
            CodexThreadStatusDto::Error => "Codex 작업 오류",
            CodexThreadStatusDto::Idle => "작업 중단 / 대기",
            _ => "현재 실행 상태 확인 중",
        }
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(25.0).strong().color(color));
        if unread && ui.button("확인했어요").clicked() {
            alerts.mark_current_read(thread);
        }
    });
    ui.add(
        egui::Label::new(RichText::new(&thread.title).size(14.0).color(theme::MUTED)).truncate(),
    );
    let observation = thread.observation.as_ref().filter(|observation| {
        match (
            observation.turn_id.as_deref(),
            crate::selection::turn_id(thread),
        ) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
    });
    theme::accent_panel(color).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new("현재 질의 · 1건")
                .size(14.0)
                .strong()
                .color(theme::ORANGE),
        );
        ui.add(
            egui::Label::new(
                RichText::new(
                    crate::selection::latest_prompt(thread)
                        .unwrap_or("현재 질의의 본문을 수신하고 있습니다"),
                )
                .size(23.0),
            )
            .wrap(),
        );
        if let Some(activity) = &thread.activity
            && activity.user_input.is_some()
            && state == CodexThreadStatusDto::WaitingApproval
        {
            ui.add_space(12.0);
            ui.separator();
            ui.label(
                RichText::new("판단이 필요합니다")
                    .size(19.0)
                    .strong()
                    .color(theme::YELLOW),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(activity.user_input.as_deref().unwrap_or_default()).size(22.0),
                )
                .wrap(),
            );
            ui.label(
                RichText::new("이 질문의 답변은 Mac Codex에서 선택하세요.")
                    .size(13.0)
                    .color(theme::MUTED),
            );
        }
        ui.add_space(14.0);
        ui.separator();
        ui.label(
            RichText::new("이 질의에 대한 Codex 응답")
                .size(14.0)
                .strong()
                .color(theme::CYAN),
        );
        ui.add(
            egui::Label::new(
                RichText::new(
                    observation
                        .and_then(|value| value.latest_agent_message.as_deref())
                        .unwrap_or("응답을 기다리고 있습니다"),
                )
                .size(22.0),
            )
            .wrap(),
        );
        if let Some(observation) = observation {
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "{} 조회",
                    observation
                        .observed_at
                        .with_timezone(&chrono::Local)
                        .format("%H:%M:%S")
                ))
                .size(11.0)
                .color(theme::MUTED),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_query_view_never_displays_another_project_or_a_previous_turn_reply() {
        let mut snapshot = crate::test_support::snapshot();
        let mut alerts = AlertCenter::default();
        let current = crate::test_support::render(930.0, 600.0, |ui| {
            render(
                ui,
                &snapshot,
                snapshot.codex.threads.first(),
                &mut alerts,
                true,
            );
        });
        assert!(current.contains("CURRENT QUESTION"));
        assert!(current.contains("CURRENT ANSWER"));
        assert!(!current.contains("OTHER PROJECT QUESTION"));
        snapshot.codex.threads[0]
            .observation
            .as_mut()
            .unwrap()
            .turn_id = Some("old".to_owned());
        let old = crate::test_support::render(930.0, 600.0, |ui| {
            render(
                ui,
                &snapshot,
                snapshot.codex.threads.first(),
                &mut alerts,
                true,
            );
        });
        assert!(!old.contains("CURRENT ANSWER"));
        assert!(!old.contains("CURRENT QUESTION"));
    }
}
