use crate::{
    i18n::{self, Language},
    icons, theme,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use orangedeck_domain::Shortcut;
use orangedeck_protocol::{ApprovalDecisionDto, ApprovalDto};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeckMode {
    Run,
    Edit,
}

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
    pub slots: [Option<Shortcut>; 8],
    pub mode: DeckMode,
}

#[derive(Default)]
pub struct DeckAction {
    pub decision: Option<ApprovalDecisionDto>,
    pub details: bool,
    pub cycle: isize,
    pub activate: Option<usize>,
    pub edit: Option<usize>,
    pub toggle_edit: bool,
    pub recommended: bool,
}

pub fn shortcut_label(action: Shortcut, lang: Language) -> (&'static str, &'static str) {
    match action {
        Shortcut::Unassigned => (
            lang.text("미지정", "Add action"),
            lang.text("+ 눌러 기능 연결", "Choose a shortcut"),
        ),
        Shortcut::Live => (
            "LIVE",
            lang.text("토큰·남은 한도 보기", "Tokens and remaining quota"),
        ),
        Shortcut::Projects => (
            lang.text("프로젝트", "Projects"),
            lang.text("프로젝트 목록 열기", "Browse your projects"),
        ),
        Shortcut::Conversations => (
            lang.text("대화 목록", "Conversations"),
            lang.text("현재 프로젝트의 대화", "Chats in this project"),
        ),
        Shortcut::Notifications => (
            lang.text("알림 보기", "Notifications"),
            lang.text("현재 질문·응답·요청", "Question, response, requests"),
        ),
        Shortcut::Refresh => (
            lang.text("새로고침", "Refresh"),
            lang.text("메인 PC에서 다시 읽기", "Read fresh data from host"),
        ),
        Shortcut::FollowLatest => (
            lang.text("최근 자동", "Follow latest"),
            lang.text("최신 대화를 자동 추적", "Follow recent activity"),
        ),
        Shortcut::PreviousProject => (
            lang.text("이전 프로젝트", "Previous project"),
            lang.text("앞 프로젝트로 이동", "Switch to previous project"),
        ),
        Shortcut::NextProject => (
            lang.text("다음 프로젝트", "Next project"),
            lang.text("다음 프로젝트로 이동", "Switch to next project"),
        ),
        Shortcut::PreviousConversation => (
            lang.text("이전 대화", "Previous chat"),
            lang.text("앞 대화를 선택·고정", "Select and pin previous chat"),
        ),
        Shortcut::NextConversation => (
            lang.text("다음 대화", "Next chat"),
            lang.text("다음 대화를 선택·고정", "Select and pin next chat"),
        ),
        Shortcut::OpenEditor => (
            lang.text("에디터 열기", "Open editor"),
            lang.text("메인 PC · 등록 프로젝트", "On host · registered project"),
        ),
        Shortcut::OpenTerminal => (
            lang.text("터미널 열기", "Open terminal"),
            lang.text("메인 PC · 등록 프로젝트", "On host · registered project"),
        ),
        Shortcut::OpenProject => (
            lang.text("폴더 열기", "Open folder"),
            lang.text("메인 PC · 등록 프로젝트", "On host · registered project"),
        ),
        Shortcut::OpenBrowser => (
            lang.text("웹페이지 열기", "Open website"),
            lang.text("메인 PC · 설정된 주소", "On host · configured URL"),
        ),
    }
}

pub fn shortcut_color(action: Shortcut) -> Color32 {
    match action {
        Shortcut::Live | Shortcut::Refresh => theme::ORANGE,
        Shortcut::Projects | Shortcut::OpenProject => theme::YELLOW,
        Shortcut::Conversations | Shortcut::PreviousConversation | Shortcut::NextConversation => {
            theme::CYAN
        }
        Shortcut::Notifications => theme::PINK,
        Shortcut::OpenEditor | Shortcut::OpenTerminal => theme::VIOLET,
        Shortcut::Unassigned => theme::MUTED,
        _ => theme::BLUE,
    }
}

/// Painting, hit testing and compact-window checks share these square key bounds.
pub(super) fn key_rects(bounds: Rect) -> [Rect; 10] {
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
        Rect::from_min_size(
            origin
                + Vec2::new(
                    f32::from(u8::try_from(index % 5).unwrap_or(0)),
                    f32::from(u8::try_from(index / 5).unwrap_or(0)),
                ) * (side + gap),
            Vec2::splat(side),
        )
    })
}

fn text(p: &egui::Painter, rect: Rect, value: &str, size: f32, color: Color32) {
    let mut job = egui::text::LayoutJob::simple(
        value.to_owned(),
        FontId::proportional(size),
        color,
        rect.width().max(1.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    p.galley(rect.min, galley, color);
}

fn control(
    ui: &egui::Ui,
    p: &egui::Painter,
    rect: Rect,
    id: &str,
    label: &str,
    selected: bool,
) -> bool {
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    p.rect_filled(
        rect,
        8,
        if selected {
            theme::ORANGE
        } else {
            theme::PANEL_RAISED
        },
    );
    if response.hovered() {
        p.rect_stroke(
            rect,
            8,
            Stroke::new(1.0, theme::ORANGE),
            egui::StrokeKind::Inside,
        );
    }
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if selected { theme::BG } else { theme::TEXT },
    );
    response.clicked()
}

pub fn render(ui: &mut egui::Ui, view: &DeckView<'_>) -> DeckAction {
    let lang = i18n::language(ui.ctx());
    let mut action = DeckAction::default();
    let (bounds, _) =
        ui.allocate_exact_size(ui.available_size().max(Vec2::splat(1.0)), Sense::hover());
    let p = ui.painter_at(bounds);
    p.text(
        bounds.min + Vec2::new(5.0, 2.0),
        Align2::LEFT_TOP,
        lang.text("나만의 컨트롤 덱", "YOUR CONTROL DECK"),
        FontId::proportional(22.0),
        theme::TEXT,
    );
    let edit = Rect::from_min_size(
        Pos2::new(bounds.right() - 80.0, bounds.top()),
        Vec2::new(75.0, 30.0),
    );
    let preset = Rect::from_min_size(edit.min - Vec2::new(106.0, 0.0), Vec2::new(98.0, 30.0));
    action.toggle_edit = control(
        ui,
        &p,
        edit,
        "deck_edit",
        lang.text(
            if view.mode == DeckMode::Edit {
                "편집 끝"
            } else {
                "키 편집"
            },
            if view.mode == DeckMode::Edit {
                "Done"
            } else {
                "Edit keys"
            },
        ),
        view.mode == DeckMode::Edit,
    );
    action.recommended = control(
        ui,
        &p,
        preset,
        "deck_preset",
        lang.text("추천 구성", "Quick setup"),
        false,
    );
    let banner = Rect::from_min_max(
        bounds.min + Vec2::new(5.0, 36.0),
        Pos2::new(bounds.right() - 5.0, bounds.top() + 80.0),
    );
    let waiting = view.approval.is_some();
    let accent = if waiting { theme::YELLOW } else { theme::CYAN };
    p.rect_filled(banner, 10, theme::PANEL);
    p.rect_filled(
        Rect::from_min_size(
            banner.min + Vec2::new(0.0, 8.0),
            Vec2::new(3.0, banner.height() - 16.0),
        ),
        2,
        accent,
    );
    let heading = if view.mode == DeckMode::Edit {
        lang.text("편집 모드 · 바꿀 키를 누르세요", "EDIT MODE · choose a key")
            .to_owned()
    } else if view.sending {
        lang.text(
            "선택 전송 중 · 처리 결과 대기",
            "Sending decision · awaiting host",
        )
        .to_owned()
    } else if waiting {
        if lang == Language::Korean {
            format!("승인 요청  {} / {}", view.position + 1, view.pending)
        } else {
            format!("APPROVAL  {} / {}", view.position + 1, view.pending)
        }
    } else if !view.ready {
        lang.text("연결 확인 중 · 실행 대기", "Checking connection")
            .to_owned()
    } else {
        lang.text(
            "준비 완료 · 원하는 기능을 연결하세요",
            "READY · make this deck yours",
        )
        .to_owned()
    };
    let controls = if waiting {
        if view.pending > 1 { 166.0 } else { 82.0 }
    } else {
        0.0
    };
    text(
        &p,
        Rect::from_min_size(
            banner.min + Vec2::new(12.0, 4.0),
            Vec2::new(banner.width() - controls - 24.0, 15.0),
        ),
        &heading,
        11.0,
        accent,
    );
    let hint = view.approval.map_or(
        lang.text(
            "빈 + 키를 눌러 추가 · 추천 구성으로 빠르게 시작",
            "Tap an empty + key, or try Quick setup",
        ),
        |request| request.summary.as_str(),
    );
    text(
        &p,
        Rect::from_min_size(
            banner.min + Vec2::new(12.0, 20.0),
            Vec2::new(banner.width() - controls - 24.0, 22.0),
        ),
        hint,
        14.0,
        theme::MUTED,
    );
    if let Some(request) = view.approval {
        let details = Rect::from_min_size(
            Pos2::new(banner.right() - 78.0, banner.top() + 4.0),
            Vec2::new(74.0, 36.0),
        );
        let response = ui
            .interact(
                details,
                ui.id().with(("deck_details", request.id)),
                Sense::click(),
            )
            .on_hover_text(request.details.join("\n"));
        p.rect_filled(details, 7, theme::PANEL_RAISED);
        p.text(
            details.center(),
            Align2::CENTER_CENTER,
            lang.text("요청 상세 ›", "Details ›"),
            FontId::proportional(12.0),
            theme::TEXT,
        );
        action.details = response.clicked();
        if view.pending > 1 {
            for (index, label) in ["‹", "›"].into_iter().enumerate() {
                let offset = f32::from(u8::try_from(index).unwrap_or(0)) * 38.0;
                let rect = Rect::from_min_size(
                    Pos2::new(details.left() - 80.0 + offset, details.top()),
                    Vec2::new(34.0, 36.0),
                );
                if control(
                    ui,
                    &p,
                    rect,
                    if index == 0 {
                        "previous_approval"
                    } else {
                        "next_approval"
                    },
                    label,
                    false,
                ) {
                    action.cycle = if index == 0 { -1 } else { 1 };
                }
            }
        }
    }
    let enabled =
        waiting && view.ready && view.armed && !view.sending && view.mode != DeckMode::Edit;
    let status = if !view.ready {
        lang.text("연결 대기", "Offline")
    } else if !waiting {
        lang.text("요청 없음", "No request")
    } else if view.sending {
        lang.text("전송 중", "Sending")
    } else if !view.armed || view.mode == DeckMode::Edit {
        lang.text("준비 중", "Not armed")
    } else {
        lang.text("승인 대기", "Awaiting decision")
    };
    for (index, rect) in key_rects(bounds).into_iter().enumerate() {
        let fixed = index < 2;
        let assigned = index.checked_sub(2).and_then(|slot| view.slots[slot]);
        let icon = assigned.unwrap_or(Shortcut::Unassigned);
        let active = if fixed { enabled } else { true };
        let color = if fixed {
            if enabled {
                if index == 0 {
                    theme::GREEN
                } else {
                    theme::PINK
                }
            } else {
                theme::MUTED
            }
        } else {
            shortcut_color(icon)
        };
        let response = ui
            .interact(
                rect,
                ui.id().with((
                    "shortcut_key",
                    index,
                    view.approval.map(|request| request.id),
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
            });
        let hover = active && response.hovered();
        let pressed = active && response.is_pointer_button_down_on();
        let filled = assigned.is_some() || (fixed && enabled);
        let fill = if filled {
            theme::tint(theme::PANEL, color, if hover { 0.26 } else { 0.12 })
        } else {
            theme::PANEL
        };
        p.rect_filled(
            rect.translate(Vec2::new(0.0, 4.0)),
            16,
            Color32::from_black_alpha(100),
        );
        p.rect_filled(rect, 16, fill);
        p.rect_stroke(
            rect,
            16,
            Stroke::new(
                if hover || pressed { 2.0 } else { 1.0 },
                if pressed {
                    theme::TEXT
                } else if hover {
                    color
                } else {
                    theme::BORDER
                },
            ),
            egui::StrokeKind::Inside,
        );
        p.rect_stroke(
            rect.shrink(3.0),
            13,
            Stroke::new(1.0, theme::tint(fill, theme::TEXT, 0.04)),
            egui::StrokeKind::Inside,
        );
        if fixed && enabled {
            p.rect_stroke(
                rect.shrink(1.0),
                16,
                Stroke::new(
                    1.5 + view.pulse,
                    color.gamma_multiply(0.55 + 0.35 * view.pulse),
                ),
                egui::StrokeKind::Inside,
            );
        }
        p.text(
            rect.min + Vec2::splat(11.0),
            Align2::LEFT_TOP,
            format!("{:02}", index + 1),
            FontId::monospace(10.0),
            color,
        );
        let center = rect.center() - Vec2::new(0.0, rect.height() * 0.06);
        let icon_size = (rect.width() * 0.28).clamp(24.0, 44.0);
        if filled {
            p.circle_filled(center, icon_size * 0.8, color.gamma_multiply(0.10));
        }
        if fixed {
            let stroke = Stroke::new((icon_size / 11.0).max(2.5), color);
            let n = icon_size * 0.36;
            if index == 0 {
                p.add(egui::Shape::line(
                    vec![
                        center + Vec2::new(-n, 0.0),
                        center + Vec2::new(-n * 0.15, n * 0.8),
                        center + Vec2::new(n, -n),
                    ],
                    stroke,
                ));
            } else {
                p.line_segment([center - Vec2::splat(n), center + Vec2::splat(n)], stroke);
                p.line_segment(
                    [center + Vec2::new(-n, n), center + Vec2::new(n, -n)],
                    stroke,
                );
            }
        } else {
            icons::draw(&p, center, icon_size, color, icon);
        }
        let (title, hint) = if fixed {
            (
                if index == 0 {
                    lang.text("승인", "Approve")
                } else {
                    lang.text("거절", "Reject")
                },
                status,
            )
        } else {
            shortcut_label(icon, lang)
        };
        let font_size = (rect.width() * 0.095).clamp(12.0, 20.0);
        let title_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 44.0),
            Vec2::new(rect.width() - 24.0, 24.0),
        );
        text(
            &p,
            title_rect,
            title,
            font_size,
            if fixed && !enabled {
                theme::MUTED
            } else {
                theme::TEXT
            },
        );
        let bottom = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 21.0),
            Vec2::new(rect.width() - 24.0, 16.0),
        );
        text(
            &p,
            bottom,
            if !fixed && view.mode == DeckMode::Edit {
                lang.text("눌러서 변경", "Tap to edit")
            } else if !fixed && assigned.is_some() {
                ""
            } else {
                hint
            },
            10.0,
            color,
        );
        if view.focused == index && active {
            p.text(
                rect.right_top() + Vec2::new(-11.0, 11.0),
                Align2::RIGHT_TOP,
                if view.mode == DeckMode::Edit && !fixed {
                    "✎"
                } else {
                    "A"
                },
                FontId::monospace(10.0),
                theme::TEXT,
            );
        }
        if fixed {
            if enabled && response.clicked_by(egui::PointerButton::Primary) {
                action.decision = Some(if index == 0 {
                    ApprovalDecisionDto::Approve
                } else {
                    ApprovalDecisionDto::Reject
                });
            }
        } else if response.secondary_clicked()
            || (response.clicked() && (view.mode == DeckMode::Edit || assigned.is_none()))
        {
            action.edit = Some(index);
        } else if response.clicked() {
            action.activate = Some(index);
        }
        response.on_hover_text(hint);
    }
    let hint = view.message.unwrap_or(lang.text(
        "+ 기능 추가 · 키 편집으로 변경 · 방향키 선택 / A 실행 · B 거절",
        "+ add action · Edit keys to change · D-pad selects / A runs · B rejects",
    ));
    text(
        &p,
        Rect::from_min_size(
            Pos2::new(bounds.left() + 5.0, bounds.bottom() - 19.0),
            Vec2::new(bounds.width() - 10.0, 19.0),
        ),
        hint,
        11.0,
        if view.message.is_some() {
            theme::YELLOW
        } else {
            theme::MUTED
        },
    );
    action
}
