use crate::i18n;
use eframe::egui::{self, Color32, RichText};
use orangedeck_protocol::{ApprovalDecisionDto, ApprovalDto, NotificationLevelDto};

use crate::theme;

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
    let lang = i18n::language(ui.ctx());
    let mut decision = None;
    theme::accent_panel(theme::YELLOW).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            RichText::new(lang.text("승인이 필요합니다", "Approval required"))
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
                lang.text(
                    "연결이 끊겼습니다. 다시 연결되면 요청 상태를 확인합니다.",
                    "Disconnected. Approval state will be checked after reconnecting.",
                )
            } else if sending {
                lang.text(
                    "선택 전송 중 · Mac의 처리 결과를 기다립니다",
                    "Sending decision · waiting for the host",
                )
            } else {
                lang.text(
                    "표시된 요청 한 건에 적용 · A 승인 / B 거부",
                    "Applies once to this request · A approve / B reject",
                )
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
                            RichText::new(lang.text("A  승인", "A  Approve"))
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
                            RichText::new(lang.text("B  거부", "B  Reject"))
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
