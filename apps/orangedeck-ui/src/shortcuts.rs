use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use orangedeck_protocol::{ApprovalDecisionDto, ApprovalDto};

use crate::theme;

pub struct DeckView<'a> {
    pub approval: Option<&'a ApprovalDto>,
    pub position: usize,
    pub pending: usize,
    pub ready: bool,
    pub armed: bool,
    pub sending: bool,
    pub focused: usize,
    pub pulse: f32,
    pub message: Option<&'a str>,
}

#[derive(Default)]
pub struct DeckAction {
    pub decision: Option<ApprovalDecisionDto>,
    pub details: bool,
    pub cycle: isize,
}

/// The same square geometry is used for painting, pointer input and compact-screen checks.
fn key_rects(bounds: Rect) -> [Rect; 10] {
    let gap = 12.0;
    let grid = Rect::from_min_max(
        bounds.min + Vec2::new(5.0, 92.0),
        bounds.max - Vec2::new(5.0, 30.0),
    );
    let side = ((grid.width() - 4.0 * gap) / 5.0)
        .min((grid.height() - gap) / 2.0)
        .max(1.0);
    let origin = grid.center() - Vec2::new(5.0 * side + 4.0 * gap, 2.0 * side + gap) / 2.0;
    std::array::from_fn(|index| {
        let column = f32::from(u8::try_from(index % 5).unwrap_or(0));
        let row = f32::from(u8::try_from(index / 5).unwrap_or(0));
        Rect::from_min_size(
            origin + Vec2::new(column, row) * (side + gap),
            Vec2::splat(side),
        )
    })
}

fn line(p: &egui::Painter, at: Pos2, value: &str, size: f32, color: Color32, width: f32) {
    let mut job = egui::text::LayoutJob::simple(
        value.to_owned(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    p.galley(at, galley, color);
}

pub fn render(ui: &mut egui::Ui, view: &DeckView<'_>) -> DeckAction {
    let mut action = DeckAction::default();
    let size = ui.available_size().max(Vec2::new(1.0, 1.0));
    let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(bounds);
    let top = bounds.min + Vec2::new(5.0, 0.0);
    p.text(
        top,
        Align2::LEFT_TOP,
        "QUICK DECK",
        FontId::proportional(22.0),
        theme::TEXT,
    );
    p.text(
        Pos2::new(bounds.right() - 5.0, bounds.top() + 5.0),
        Align2::RIGHT_TOP,
        "10 KEYS  /  5 × 2",
        FontId::monospace(11.0),
        theme::MUTED,
    );

    let banner = Rect::from_min_max(
        top + Vec2::new(0.0, 34.0),
        Pos2::new(bounds.right() - 5.0, bounds.top() + 80.0),
    );
    let waiting = view.approval.is_some();
    let accent = if !view.ready {
        theme::YELLOW
    } else if waiting {
        theme::ORANGE
    } else {
        theme::CYAN
    };
    p.rect_filled(banner, 8, theme::PANEL);
    p.rect_filled(
        Rect::from_min_size(banner.min, Vec2::new(3.0, banner.height())),
        2,
        accent,
    );
    let heading = if !view.ready {
        "연결 확인 중 · 실행 대기".to_owned()
    } else if view.sending {
        "선택 전송 중 · 처리 결과 대기".to_owned()
    } else if waiting {
        format!("승인 요청  {} / {}", view.position + 1, view.pending)
    } else {
        "승인 요청 없음".to_owned()
    };
    line(
        &p,
        banner.min + Vec2::new(13.0, 4.0),
        &heading,
        11.0,
        accent,
        banner.width() - 180.0,
    );
    let description = view.approval.map_or(
        "요청이 오면 승인·거절 키가 켜집니다.",
        |approval| approval.summary.as_str(),
    );
    let controls = if waiting {
        if view.pending > 1 { 164.0 } else { 86.0 }
    } else {
        0.0
    };
    line(
        &p,
        banner.min + Vec2::new(13.0, 21.0),
        description,
        15.0,
        if waiting { theme::TEXT } else { theme::MUTED },
        banner.width() - controls - 26.0,
    );
    if let Some(approval) = view.approval {
        let detail_rect = Rect::from_min_size(
            Pos2::new(banner.right() - 83.0, banner.top() + 4.0),
            Vec2::new(78.0, 38.0),
        );
        let response = ui
            .interact(
                detail_rect,
                ui.id().with(("deck_details", approval.id)),
                Sense::click(),
            )
            .on_hover_text(format!(
                "{}\n{}\n{}",
                approval.title,
                approval.summary,
                approval.details.join("\n")
            ));
        p.rect_filled(
            detail_rect,
            6,
            if response.hovered() {
                theme::BORDER
            } else {
                theme::PANEL_RAISED
            },
        );
        p.text(
            detail_rect.center(),
            Align2::CENTER_CENTER,
            "요청 상세 ›",
            FontId::proportional(12.0),
            theme::TEXT,
        );
        action.details = response.clicked();
        if view.pending > 1 {
            for (index, label) in ["‹", "›"].into_iter().enumerate() {
                let offset = f32::from(u8::try_from(index).unwrap_or(0)) * 37.0;
                let rect = Rect::from_min_size(
                    Pos2::new(detail_rect.left() - 78.0 + offset, detail_rect.top()),
                    Vec2::new(33.0, 38.0),
                );
                let response =
                    ui.interact(rect, ui.id().with(("deck_cycle", index)), Sense::click());
                p.rect_filled(rect, 6, theme::PANEL_RAISED);
                p.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(20.0),
                    theme::TEXT,
                );
                if response.clicked() {
                    action.cycle = if index == 0 { -1 } else { 1 };
                }
            }
        }
    }

    let enabled = waiting && view.ready && view.armed && !view.sending;
    let status = if !view.ready {
        "연결 대기"
    } else if !waiting {
        "요청 없음"
    } else if view.sending {
        "전송 중"
    } else if !view.armed {
        "준비 중"
    } else {
        "승인 대기"
    };
    for (index, rect) in key_rects(bounds).into_iter().enumerate() {
        let assigned = index < 2;
        let active = assigned && enabled;
        let color = match (index, active) {
            (0, true) => theme::GREEN,
            (1, true) => theme::PINK,
            _ => theme::MUTED,
        };
        let response = ui
            .interact(
                rect,
                // A release begun on a previous request must never decide its replacement.
                ui.id().with((
                    "shortcut_key",
                    index,
                    view.approval.map(|approval| approval.id),
                )),
                if active {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            )
            .on_hover_cursor(if active {
                egui::CursorIcon::PointingHand
            } else {
                egui::CursorIcon::Default
            })
            .on_hover_text(if !assigned {
                "아직 기능을 지정하지 않은 키입니다."
            } else if !view.ready {
                "연결과 최신 요청을 확인한 뒤 사용할 수 있습니다."
            } else if !waiting {
                "현재 질의에 승인 요청이 없습니다. 요청이 오면 키가 켜집니다."
            } else if view.sending {
                "선택을 전송했습니다. 처리 결과를 기다리고 있습니다."
            } else if !view.armed {
                "새 요청을 표시하고 있습니다. 잠시 후 사용할 수 있습니다."
            } else if index == 0 {
                "위에 표시된 승인 요청 한 건만 허용합니다."
            } else {
                "위에 표시된 승인 요청 한 건을 거절합니다."
            });
        let hovered = active && response.hovered();
        let pressed = active && response.is_pointer_button_down_on();
        if active {
            for spread in [3.0, 6.0] {
                p.rect_stroke(
                    rect.expand(spread),
                    15,
                    Stroke::new(
                        2.0,
                        color.gamma_multiply(
                            (if hovered { 0.28 } else { 0.10 } + view.pulse * 0.18)
                                / (spread / 3.0),
                        ),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
        }
        p.rect_filled(rect.translate(Vec2::new(0.0, 4.0)), 13, Color32::BLACK);
        p.rect_filled(
            rect,
            13,
            if pressed {
                color.gamma_multiply(0.36)
            } else if hovered {
                color.gamma_multiply(0.24)
            } else {
                theme::PANEL_RAISED
            },
        );
        let inner = rect.shrink(4.0);
        p.rect_filled(
            inner,
            10,
            if active {
                color.gamma_multiply(if pressed {
                    0.40
                } else if hovered {
                    0.28 + view.pulse * 0.08
                } else {
                    0.08 + view.pulse * 0.16
                })
            } else {
                theme::PANEL
            },
        );
        // Controller focus is a separate A badge; it must never light an idle key.
        let border = if pressed {
            theme::TEXT
        } else if hovered {
            color
        } else if active {
            color.gamma_multiply(0.4 + 0.6 * view.pulse)
        } else {
            theme::BORDER
        };
        p.rect_stroke(
            rect,
            13,
            Stroke::new(
                if hovered || pressed {
                    3.5
                } else if active {
                    2.0 + 1.5 * view.pulse
                } else {
                    1.0
                },
                border,
            ),
            egui::StrokeKind::Inside,
        );
        line(
            &p,
            rect.min + Vec2::new(11.0, 8.0),
            &format!("{:02}", index + 1),
            10.0,
            color,
            30.0,
        );
        if enabled && view.focused == index {
            let badge = Rect::from_min_size(
                Pos2::new(rect.right() - 28.0, rect.top() + 8.0),
                Vec2::new(18.0, 16.0),
            );
            p.rect_filled(badge, 4, theme::BORDER);
            p.text(
                badge.center(),
                Align2::CENTER_CENTER,
                "A",
                FontId::monospace(10.0),
                theme::TEXT,
            );
        }
        let center = rect.min + Vec2::new(rect.width() * 0.5, rect.height() * 0.40);
        let radius = (rect.width() * 0.16).clamp(12.0, 26.0);
        let icon = if hovered || pressed {
            theme::TEXT
        } else if active {
            color
        } else {
            color.gamma_multiply(0.5)
        };
        let stroke = Stroke::new(if assigned { 3.5 } else { 1.5 }, icon);
        if assigned {
            p.circle_filled(
                center,
                radius + 7.0,
                if active {
                    color.gamma_multiply(0.07)
                } else {
                    theme::PANEL_RAISED
                },
            );
        }
        match index {
            0 => {
                p.line_segment(
                    [
                        center + Vec2::new(-radius * 0.65, 0.0),
                        center + Vec2::new(-radius * 0.15, radius * 0.5),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        center + Vec2::new(-radius * 0.15, radius * 0.5),
                        center + Vec2::new(radius * 0.7, -radius * 0.55),
                    ],
                    stroke,
                );
            }
            1 => {
                p.line_segment(
                    [
                        center - Vec2::splat(radius * 0.55),
                        center + Vec2::splat(radius * 0.55),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        center + Vec2::new(-radius * 0.55, radius * 0.55),
                        center + Vec2::new(radius * 0.55, -radius * 0.55),
                    ],
                    stroke,
                );
            }
            _ => {
                p.line_segment(
                    [
                        center - Vec2::new(radius * 0.5, 0.0),
                        center + Vec2::new(radius * 0.5, 0.0),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        center - Vec2::new(0.0, radius * 0.5),
                        center + Vec2::new(0.0, radius * 0.5),
                    ],
                    stroke,
                );
            }
        }
        let label = match index {
            0 => "승인",
            1 => "거절",
            _ => "미지정",
        };
        p.text(
            Pos2::new(
                rect.center().x,
                rect.bottom() - if assigned { 29.0 } else { 23.0 },
            ),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(if assigned {
                (rect.width() * 0.18).clamp(17.0, 25.0)
            } else {
                13.0
            }),
            if active { theme::TEXT } else { theme::MUTED },
        );
        if assigned {
            p.text(
                Pos2::new(rect.center().x, rect.bottom() - 10.0),
                Align2::CENTER_CENTER,
                status,
                FontId::proportional(10.0),
                color,
            );
        }
        if active && response.clicked_by(egui::PointerButton::Primary) {
            action.decision = Some(if index == 0 {
                ApprovalDecisionDto::Approve
            } else {
                ApprovalDecisionDto::Reject
            });
        }
    }
    line(
        &p,
        Pos2::new(bounds.left() + 5.0, bounds.bottom() - 19.0),
        if let Some(message) = view.message {
            message
        } else if !view.ready {
            "연결과 최신 요청을 확인하고 있습니다. 승인·거절 키는 잠시 잠깁니다."
        } else if view.sending {
            "선택을 전송했습니다. 처리 결과가 올 때까지 두 키가 잠깁니다."
        } else if waiting && !view.armed {
            "새 승인 요청을 표시하고 있습니다. 내용을 확인해 주세요."
        } else if enabled {
            "터치로 실행 · A 선택 / B 거절 · Y 요청 상세 · 표시된 요청 1건에 적용"
        } else {
            "03—10  미지정 · 남은 8개 키의 기능은 다음에 설정합니다."
        },
        11.0,
        theme::MUTED,
        bounds.width() - 10.0,
    );
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DeckFrame {
        keys: [Vec<egui::epaint::RectShape>; 2],
        labels: Vec<(String, Color32)>,
        cursor: egui::CursorIcon,
        action: DeckAction,
    }

    fn frame(ctx: &egui::Context, view: &DeckView<'_>, events: Vec<egui::Event>) -> DeckFrame {
        let mut action = DeckAction::default();
        let mut keys = [Rect::NOTHING; 10];
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(676.0, 332.0))),
                events,
                time: Some(0.0),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        keys = key_rects(ui.available_rect_before_wrap());
                        action = render(ui, view);
                    });
            },
        );
        let mut painted = DeckFrame {
            keys: std::array::from_fn(|_| Vec::new()),
            labels: Vec::new(),
            cursor: output.platform_output.cursor_icon,
            action,
        };
        for shape in &output.shapes {
            match &shape.shape {
                egui::Shape::Rect(rect) => {
                    for (index, key) in keys[..2].iter().enumerate() {
                        // Compare the visible key surfaces, excluding the small A focus badge.
                        if key.expand(6.0).contains_rect(rect.rect)
                            && rect.rect.width() >= key.width() - 8.0
                        {
                            let mut local = rect.clone();
                            local.rect = local.rect.translate(-key.min.to_vec2());
                            painted.keys[index].push(local);
                        }
                    }
                }
                egui::Shape::Text(text) => {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    let value = &text.galley.job.text;
                    if keys[..2].iter().any(|key| key.contains(rect.center())) {
                        assert!(
                            keys[..2].iter().any(|key| key.contains_rect(rect)),
                            "{value}"
                        );
                        assert!(shape.clip_rect.contains_rect(rect), "{value}");
                        painted
                            .labels
                            .push((value.clone(), text.galley.job.sections[0].format.color));
                    }
                }
                _ => {}
            }
        }
        output.drop_without_applying_deltas();
        assert!(!painted.keys[0].is_empty() && !painted.keys[1].is_empty());
        painted
    }

    fn request() -> ApprovalDto {
        ApprovalDto {
            id: uuid::Uuid::new_v4(),
            thread_id: Some("a".to_owned()),
            turn_id: Some("new".to_owned()),
            kind: orangedeck_protocol::ApprovalKindDto::CommandExecution,
            title: "모의 승인 요청".to_owned(),
            summary: "cargo check --offline".to_owned(),
            details: Vec::new(),
            requested_at: chrono::Utc::now(),
        }
    }

    fn view(approval: Option<&ApprovalDto>) -> DeckView<'_> {
        DeckView {
            approval,
            position: 0,
            pending: usize::from(approval.is_some()),
            ready: true,
            armed: true,
            sending: false,
            focused: 0,
            pulse: 0.5,
            message: None,
        }
    }

    #[test]
    fn disabled_keys_stay_neutral_under_focus_hover_and_click_in_every_unavailable_state() {
        let request = request();
        for (approval, ready, armed, sending, status) in [
            (None, true, true, false, "요청 없음"),
            (Some(&request), false, true, false, "연결 대기"),
            (Some(&request), true, true, true, "전송 중"),
            (Some(&request), true, false, false, "준비 중"),
        ] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let mut view = DeckView {
                ready,
                armed,
                sending,
                ..view(approval)
            };
            frame(&ctx, &view, vec![]);
            let baseline = frame(&ctx, &view, vec![]);
            assert_eq!(baseline.keys[0], baseline.keys[1], "{status}");
            assert_eq!(
                baseline
                    .labels
                    .iter()
                    .filter(|(label, _)| label == status)
                    .count(),
                2
            );
            assert!(
                baseline
                    .labels
                    .iter()
                    .all(|(_, color)| *color == theme::MUTED)
            );
            for (index, key) in key_rects(Rect::from_min_size(Pos2::ZERO, Vec2::new(676.0, 332.0)))
                [..2]
                .iter()
                .enumerate()
            {
                view.focused = index;
                view.pulse = 1.0;
                for event in [
                    egui::Event::PointerMoved(key.center()),
                    egui::Event::PointerButton {
                        pos: key.center(),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: key.center(),
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ] {
                    let hovered = frame(&ctx, &view, vec![event]);
                    assert_eq!(hovered.keys, baseline.keys, "{status} key {index}");
                    assert_eq!(hovered.cursor, egui::CursorIcon::Default);
                    assert!(hovered.action.decision.is_none());
                }
            }
        }
    }

    #[test]
    fn both_pending_keys_pulse_and_have_equivalent_hover_and_press_feedback() {
        let request = request();
        for (index, decision) in [
            (0, ApprovalDecisionDto::Approve),
            (1, ApprovalDecisionDto::Reject),
        ] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let mut view = view(Some(&request));
            frame(&ctx, &view, vec![]);
            let baseline = frame(&ctx, &view, vec![]);
            view.focused = 1;
            let focused = frame(&ctx, &view, vec![]);
            assert_eq!(focused.keys, baseline.keys, "focus must not mimic hover");
            view.pulse = 1.0;
            let pulsed = frame(&ctx, &view, vec![]);
            for key in 0..2 {
                assert_ne!(pulsed.keys[key], baseline.keys[key]);
            }
            view.pulse = 0.5;
            let pos =
                key_rects(Rect::from_min_size(Pos2::ZERO, Vec2::new(676.0, 332.0)))[index].center();
            let hovered = frame(&ctx, &view, vec![egui::Event::PointerMoved(pos)]);
            assert_ne!(hovered.keys[index], baseline.keys[index]);
            assert_eq!(hovered.keys[1 - index], baseline.keys[1 - index]);
            assert_eq!(hovered.cursor, egui::CursorIcon::PointingHand);
            assert!(hovered.action.decision.is_none());
            let pressed = frame(
                &ctx,
                &view,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert_ne!(pressed.keys[index], hovered.keys[index]);
            assert!(pressed.action.decision.is_none());
            let released = frame(
                &ctx,
                &view,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert_eq!(released.action.decision, Some(decision));
            assert_eq!(
                released
                    .labels
                    .iter()
                    .filter(|(label, _)| label == "승인 대기")
                    .count(),
                2
            );
        }
    }

    #[test]
    fn ten_square_keys_fit_two_rows_at_ally_sizes() {
        for (width, height) in [
            (676.0, 332.0),
            (700.0, 348.0),
            (894.0, 442.0),
            (914.0, 460.0),
        ] {
            let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            let keys = key_rects(bounds);
            for (index, key) in keys.iter().enumerate() {
                assert!((key.width() - key.height()).abs() < 0.01);
                assert!(key.width() >= 98.0);
                assert!(bounds.contains_rect(*key));
                assert!((key.top() - keys[(index / 5) * 5].top()).abs() < 0.01);
                for other in keys.iter().skip(index + 1) {
                    assert!(!key.intersects(*other));
                }
            }
        }
    }
}
