//! Five conversation columns: responses above, this turn's files below.
use crate::{
    i18n::{self, Language},
    icons, theme,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use orangedeck_domain::Shortcut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
    Loading,
    None,
    Changes(usize),
    Unavailable,
}

pub struct PairView {
    pub identity: String,
    pub label: String,
    pub status: String,
    pub assigned: bool,
    pub available: bool,
    pub attention: bool,
    pub files: FileState,
}

pub struct DeckView<'a> {
    pub pairs: &'a [PairView; 5],
    pub focused: usize,
    pub pulse: f32,
    pub editing: bool,
    pub sound: bool,
    pub connected: bool,
}

#[derive(Default)]
pub struct DeckAction {
    pub activate: Option<usize>,
    pub edit: Option<usize>,
    pub toggle_edit: bool,
    pub toggle_sound: bool,
}

pub fn key_rects(bounds: Rect) -> [Rect; 10] {
    let gap = 12.0;
    let grid = Rect::from_min_max(
        bounds.min + Vec2::new(5.0, 60.0),
        bounds.max - Vec2::new(5.0, 22.0),
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
    p.galley(rect.min, p.layout_job(job), color);
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
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        if selected { theme::BG } else { theme::TEXT },
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

pub fn render(ui: &mut egui::Ui, view: &DeckView<'_>) -> DeckAction {
    let lang = i18n::language(ui.ctx());
    let mut action = DeckAction::default();
    let (bounds, _) =
        ui.allocate_exact_size(ui.available_size().max(Vec2::splat(1.0)), Sense::hover());
    let p = ui.painter_at(bounds);
    text(
        &p,
        Rect::from_min_size(bounds.min, Vec2::new(bounds.width() - 210.0, 26.0)),
        lang.text("다섯 개의 Codex", "FIVE CODEX CONVERSATIONS"),
        21.0,
        theme::TEXT,
    );
    let edit = Rect::from_min_size(
        Pos2::new(bounds.right() - 90.0, bounds.top()),
        Vec2::new(90.0, 32.0),
    );
    action.toggle_edit = control(
        ui,
        &p,
        edit,
        "edit_pairs",
        lang.text(
            if view.editing {
                "설정 끝"
            } else {
                "대화 연결"
            },
            if view.editing { "Done" } else { "Assign chats" },
        ),
        view.editing,
    );
    let sound = Rect::from_min_size(edit.min - Vec2::new(106.0, 0.0), Vec2::new(98.0, 32.0));
    action.toggle_sound = control(
        ui,
        &p,
        sound,
        "pair_sound",
        lang.text(
            if view.sound {
                "소리 켜짐"
            } else {
                "소리 꺼짐"
            },
            if view.sound { "Sound on" } else { "Sound off" },
        ),
        false,
    );
    text(
        &p,
        Rect::from_min_size(
            bounds.min + Vec2::new(0.0, 35.0),
            Vec2::new(bounds.width(), 20.0),
        ),
        lang.text(
            if view.editing {
                "위 행에서 연결할 대화를 고르세요 · 위아래가 한 쌍입니다"
            } else {
                "위: 응답·승인   /   아래: 수정 파일 목록"
            },
            if view.editing {
                "Choose a conversation for each column · both keys stay paired"
            } else {
                "Top: response & approval   /   Bottom: changed file list"
            },
        ),
        12.0,
        theme::MUTED,
    );
    let keys = key_rects(bounds);
    let colors = [
        theme::ORANGE,
        theme::CYAN,
        theme::VIOLET,
        theme::BLUE,
        theme::PINK,
    ];
    for (index, key) in keys.into_iter().enumerate() {
        let column = index % 5;
        let top = index < 5;
        let pair = &view.pairs[column];
        let enabled = if view.editing {
            top
        } else if top {
            !pair.assigned || pair.available
        } else {
            pair.available && pair.files != FileState::None
        };
        let response = ui.interact(
            key,
            ui.id().with(("pair_key", index, &pair.identity)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let accent = colors[column];
        let pulse = if pair.attention { view.pulse } else { 0.0 };
        let fill = theme::tint(
            theme::PANEL_RAISED,
            accent,
            if pair.attention {
                0.10 + pulse * 0.20
            } else {
                0.035
            },
        );
        p.rect_filled(
            key,
            16,
            if enabled && response.is_pointer_button_down_on() {
                theme::tint(fill, accent, 0.22)
            } else {
                fill
            },
        );
        p.rect_stroke(
            key,
            16,
            Stroke::new(
                if pair.attention { 2.0 + pulse } else { 1.0 },
                if pair.attention || (enabled && response.hovered()) {
                    accent
                } else {
                    theme::BORDER
                },
            ),
            egui::StrokeKind::Inside,
        );
        p.rect_filled(
            Rect::from_min_size(key.min + Vec2::new(13.0, 10.0), Vec2::new(22.0, 3.0)),
            2,
            accent,
        );
        let label_color = if enabled || pair.attention {
            theme::TEXT
        } else {
            theme::MUTED
        };
        let role = lang.text(
            if top { "응답" } else { "파일" },
            if top { "REPLY" } else { "FILES" },
        );
        text(
            &p,
            Rect::from_min_size(
                key.min + Vec2::new(12.0, 19.0),
                Vec2::new(key.width() - 24.0, 17.0),
            ),
            &format!("{:02} · {role}", column + 1),
            11.0,
            accent,
        );
        icons::draw(
            &p,
            Pos2::new(key.center().x, key.top() + key.height() * 0.48),
            (key.width() * 0.23).min(36.0),
            label_color,
            if !pair.assigned && top {
                Shortcut::Unassigned
            } else if top {
                Shortcut::Conversations
            } else {
                Shortcut::OpenEditor
            },
        );
        let main = if top {
            if pair.assigned {
                pair.label.clone()
            } else {
                lang.text("대화 연결", "Assign chat").to_owned()
            }
        } else {
            match pair.files {
                FileState::Loading => lang.text("확인 중", "Checking files").to_owned(),
                FileState::None => lang.text("파일 수정 없음", "No file changes").to_owned(),
                FileState::Unavailable => {
                    lang.text("파일 정보 확인", "Check file records").to_owned()
                }
                FileState::Changes(count) => {
                    if lang == Language::Korean {
                        format!("수정 파일 {count}개")
                    } else {
                        format!("{count} changed files")
                    }
                }
            }
        };
        text(
            &p,
            Rect::from_min_size(
                Pos2::new(key.left() + 10.0, key.bottom() - 42.0),
                Vec2::new(key.width() - 20.0, 20.0),
            ),
            &main,
            14.0,
            label_color,
        );
        let hint = if top {
            pair.status.as_str()
        } else if !pair.assigned {
            lang.text("위 버튼과 연결", "Paired with top key")
        } else if !view.connected {
            lang.text("마지막 기록 보기", "Review cached files")
        } else if matches!(pair.files, FileState::Changes(_)) {
            lang.text("파일 목록 보기", "View file list")
        } else if matches!(pair.files, FileState::Loading | FileState::Unavailable) {
            lang.text("눌러서 다시 확인", "Tap to check again")
        } else {
            ""
        };
        text(
            &p,
            Rect::from_min_size(
                Pos2::new(key.left() + 10.0, key.bottom() - 21.0),
                Vec2::new(key.width() - 20.0, 16.0),
            ),
            hint,
            10.0,
            theme::MUTED,
        );
        if view.focused == index && enabled {
            p.circle_filled(key.right_top() + Vec2::new(-15.0, 15.0), 8.0, accent);
            p.text(
                key.right_top() + Vec2::new(-15.0, 15.0),
                Align2::CENTER_CENTER,
                "A",
                FontId::proportional(10.0),
                theme::BG,
            );
        }
        if enabled {
            let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            if top
                && (response.secondary_clicked()
                    || response.clicked() && (view.editing || !pair.assigned))
            {
                action.edit = Some(column);
            } else if response.clicked() {
                action.activate = Some(index);
            }
        }
    }
    action
}
