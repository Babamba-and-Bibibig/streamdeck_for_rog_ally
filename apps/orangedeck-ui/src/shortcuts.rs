mod view;
#[cfg(test)]
use crate::theme;
#[cfg(test)]
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
#[cfg(test)]
use orangedeck_protocol::{ApprovalDecisionDto, ApprovalDto};
#[cfg(test)]
use view::key_rects;
pub use view::{DeckAction, DeckMode, DeckView, render, shortcut_color, shortcut_label};

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
            slots: [None; 8],
            mode: DeckMode::Run,
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
                slots: [None; 8],
                mode: DeckMode::Run,
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
