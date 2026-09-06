//! A read-only instrument panel. Unknown data is never turned into a zero or a live signal.
use chrono::Utc;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use orangedeck_protocol::{
    CodexConnectionStateDto, CodexThreadDto, CodexThreadStatusDto, LiveTokenUsageDto, SnapshotDto,
};

use crate::theme;

#[derive(Default)]
pub struct MonitorAction {
    pub project_delta: isize,
    pub follow: bool,
    pub detail: bool,
}

pub fn render(
    ui: &mut egui::Ui,
    snapshot: &SnapshotDto,
    selected: usize,
    connected: bool,
    following: bool,
    height: f32,
) -> MonitorAction {
    let mut action = MonitorAction::default();
    let width = (ui.available_width() - 2.0).max(1.0);
    let row_height = (height - 184.0).clamp(196.0, 240.0);
    let dense = row_height < 230.0;
    let (canvas, _) = ui.allocate_exact_size(Vec2::new(width, row_height + 22.0), Sense::hover());
    let p = ui.painter_at(canvas);
    let connected =
        connected && snapshot.codex.connection.state == CodexConnectionStateDto::Connected;
    let now = Utc::now().timestamp();
    text(&p, canvas.min, "CODEX  /  LIVE", 13.0, theme::MUTED);
    p.text(
        Pos2::new(canvas.right(), canvas.top()),
        Align2::RIGHT_TOP,
        if !connected {
            "연결 끊김 · 수신 대기"
        } else if following {
            "현재 프로젝트 · 최신 대화 자동"
        } else {
            "현재 프로젝트 · 대화 고정"
        },
        FontId::proportional(12.0),
        if connected {
            theme::CYAN
        } else {
            theme::YELLOW
        },
    );
    let gap = 12.0;
    let top = canvas.top() + 22.0;
    let hero_width = (width - gap) * 0.62;
    let hero = Rect::from_min_size(
        Pos2::new(canvas.left(), top),
        Vec2::new(hero_width, row_height),
    );
    let workspace = Rect::from_min_max(Pos2::new(hero.right() + gap, top), canvas.max);
    let thread = snapshot.codex.threads.get(selected);
    let usage = thread.and_then(|thread| visible_usage(thread, now));
    if let Some(usage) = usage {
        token_hero(ui, &p, hero, usage, connected, now);
    } else {
        card(&p, hero, true);
        let inner = hero.shrink(16.0);
        text(&p, inner.min, "현재 질의 토큰", 15.0, theme::ORANGE);
        text(&p, inner.min + Vec2::new(0.0, 24.0), "—", 94.0, theme::TEXT);
        fitted(
            &p,
            inner.min + Vec2::new(0.0, if dense { inner.height() - 24.0 } else { 140.0 }),
            "새 토큰 집계 대기",
            20.0,
            theme::MUTED,
            inner.width(),
            1,
        );
        if !dense {
            fitted(
                &p,
                inner.min + Vec2::new(0.0, 177.0),
                "모델 요청이 끝날 때 갱신 · 약 5초 간격 확인",
                12.0,
                theme::MUTED,
                inner.width(),
                2,
            );
        }
        ui.interact(hero, ui.id().with("pending_tokens"), Sense::hover())
            .on_hover_text("모델 요청이 끝나면 약 5초 간격으로 새 토큰 집계를 읽습니다. 새 질의가 시작되면 이전 질의의 수치를 비웁니다.");
    }
    card(&p, workspace, false);
    let inside = workspace.shrink(16.0);
    text(&p, inside.min, "현재 프로젝트", 12.0, theme::CYAN);
    if let Some(thread) = thread {
        let path = crate::selection::project_path(&thread.cwd);
        let folder = path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or("/");
        fitted(
            &p,
            inside.min + Vec2::new(0.0, 19.0),
            folder,
            25.0,
            theme::TEXT,
            inside.width(),
            1,
        );
        fitted(
            &p,
            inside.min + Vec2::new(0.0, if dense { 49.0 } else { 52.0 }),
            &thread.cwd,
            11.0,
            theme::MUTED,
            inside.width(),
            if dense { 1 } else { 2 },
        );
        if !dense {
            fitted(
                &p,
                inside.min + Vec2::new(0.0, 83.0),
                &format!(
                    "{} · 대화 {}개",
                    snapshot.host.name,
                    snapshot.codex.threads.len()
                ),
                11.0,
                theme::MUTED,
                inside.width(),
                1,
            );
        }
    }
    let query = Rect::from_min_max(
        inside.min + Vec2::new(0.0, if dense { 66.0 } else { 104.0 }),
        Pos2::new(inside.right(), inside.bottom() - 42.0),
    );
    text(
        &p,
        query.min,
        "현재 질의 · 눌러서 크게 보기",
        11.0,
        theme::ORANGE,
    );
    let prompt = thread
        .and_then(crate::selection::latest_prompt)
        .unwrap_or("현재 질의 내용을 기다리고 있습니다");
    fitted(
        &p,
        query.min + Vec2::new(0.0, 19.0),
        prompt,
        18.0,
        theme::TEXT,
        query.width(),
        2,
    );
    if ui
        .interact(query, ui.id().with("current_query"), Sense::click())
        .clicked()
    {
        action.detail = true;
    }
    let buttons = [("‹", -1), ("›", 1), ("최근 자동", 0)];
    let button_width = (inside.width() - 12.0) / 3.0;
    for (index, (label, delta)) in buttons.into_iter().enumerate() {
        let x = inside.left() + f32::from(u16::try_from(index).unwrap_or(0)) * (button_width + 6.0);
        let rect = Rect::from_min_size(
            Pos2::new(x, inside.bottom() - 34.0),
            Vec2::new(button_width, 34.0),
        );
        let active = index == 2 && following;
        let label = if active { "자동 ON" } else { label };
        let response = ui
            .interact(
                rect,
                ui.id().with(("project_button", index)),
                Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(match index {
                0 => "이전 프로젝트 선택",
                1 => "다음 프로젝트 선택",
                _ if following => {
                    "자동 모드가 켜져 있습니다. 현재 프로젝트의 최신 대화를 따라갑니다. 이미 최신 대화를 보고 있으면 화면은 그대로입니다. 대화 탭에서 다른 대화를 고르면 고정 모드로 바뀝니다."
                }
                _ => {
                    "현재 프로젝트에서 가장 최근에 활동한 대화로 돌아가고, 이후에도 최신 대화를 자동으로 따라갑니다."
                }
            });
        p.rect_filled(
            rect,
            6,
            if response.is_pointer_button_down_on() {
                theme::CYAN.gamma_multiply(0.6)
            } else if active {
                theme::CYAN
            } else {
                theme::PANEL_RAISED
            },
        );
        if response.hovered() {
            p.rect_stroke(
                rect,
                6,
                Stroke::new(1.0, theme::CYAN),
                egui::StrokeKind::Inside,
            );
        }
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(if index == 2 { 11.0 } else { 22.0 }),
            if active { theme::BG } else { theme::TEXT },
        );
        if response.clicked() {
            if delta == 0 {
                action.follow = true;
            } else {
                action.project_delta = delta;
            }
        }
    }
    crate::usage::render(ui, snapshot, connected);
    action
}

/// A recorded count remains useful after completion. Only a different turn invalidates it.
fn visible_usage(thread: &CodexThreadDto, now: i64) -> Option<&LiveTokenUsageDto> {
    let usage = thread.live_usage.as_ref()?;
    (usage
        .turn_id
        .as_deref()
        .is_some_and(|id| crate::selection::turn_id(thread) == Some(id))
        && now - usage.observed_at.timestamp() >= -5
        && now - usage.updated_at.timestamp() >= -5
        && usage.last_request.total_tokens > 0)
        .then_some(usage)
}

fn usage_caption(usage: &LiveTokenUsageDto, connected: bool, now: i64) -> (String, Color32) {
    let (state, color) = if !connected {
        ("연결 끊김 · 기록", theme::MUTED)
    } else if now - usage.observed_at.timestamp() > 20 {
        ("수신 지연 · 기록", theme::YELLOW)
    } else if usage.status == CodexThreadStatusDto::Completed {
        ("완료", theme::GREEN)
    } else if now - usage.updated_at.timestamp() <= 20 {
        ("집계 수신", theme::ORANGE)
    } else {
        ("집계 대기", theme::YELLOW)
    };
    (
        format!(
            "{state} {}",
            usage
                .updated_at
                .with_timezone(&chrono::Local)
                .format("%m/%d %H:%M")
        ),
        color,
    )
}

fn token_hero(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: Rect,
    usage: &LiveTokenUsageDto,
    connected: bool,
    now: i64,
) {
    card(p, rect, true);
    let inner = rect.shrink(16.0);
    let dense = rect.height() < 230.0;
    let seconds = (now - usage.updated_at.timestamp()).max(0);
    let finished = usage.status == CodexThreadStatusDto::Completed;
    let (caption, accent) = usage_caption(usage, connected, now);
    p.rect_filled(
        Rect::from_min_size(inner.min, Vec2::new(3.0, 13.0)),
        2,
        accent,
    );
    text(
        p,
        inner.min + Vec2::new(11.0, -1.0),
        if usage.turn_tokens.is_none() {
            "최근 모델 요청 토큰"
        } else {
            "현재 질의 토큰"
        },
        14.0,
        accent,
    );
    p.text(
        Pos2::new(inner.right(), inner.top()),
        Align2::RIGHT_TOP,
        caption,
        FontId::proportional(11.0),
        accent,
    );
    let counted = usage.turn_tokens.as_ref().unwrap_or(&usage.last_request);
    let scope = if usage.turn_tokens.is_none() {
        "최근 모델 요청 · 1회"
    } else if finished {
        "완료한 질의의 최종 집계"
    } else {
        "이번 작업에서 사용한 토큰"
    };
    if !dense {
        text(
            p,
            inner.min + Vec2::new(0.0, 23.0),
            scope,
            13.0,
            theme::MUTED,
        );
    }
    let number = separated(counted.total_tokens);
    // Fit the entire count at the largest font size; never truncate significant digits.
    let measured = p.layout_no_wrap(number.clone(), FontId::proportional(94.0), theme::TEXT);
    let size = 94.0 * (inner.width() / measured.size().x.max(1.0)).min(1.0);
    text(
        p,
        inner.min + Vec2::new(0.0, if dense { 24.0 } else { 39.0 }),
        &number,
        size,
        theme::TEXT,
    );
    let half = inner.width() * 0.5;
    fitted(
        p,
        inner.min + Vec2::new(0.0, if dense { inner.height() - 24.0 } else { 149.0 }),
        &format!("입력  {}", separated(counted.input_tokens)),
        18.0,
        theme::CYAN,
        half - 8.0,
        1,
    );
    fitted(
        p,
        inner.min + Vec2::new(half, if dense { inner.height() - 24.0 } else { 149.0 }),
        &format!("출력  {}", separated(counted.output_tokens)),
        18.0,
        theme::ORANGE,
        half,
        1,
    );
    // These bars are real model-request totals, not an invented tokens/sec animation.
    if !dense {
        let values: Vec<_> = usage
            .recent_requests
            .iter()
            .rev()
            .take(16)
            .copied()
            .rev()
            .collect();
        let maximum = values.iter().copied().max().unwrap_or(1).max(1);
        let bar_width = (inner.width() - 15.0 * 4.0) / 16.0;
        for (index, value) in values.iter().enumerate() {
            // Integer scaling avoids lossy conversion of unbounded token counters.
            let scaled = i128::from(*value).max(0) * 1000 / i128::from(maximum);
            let ratio = f32::from(u16::try_from(scaled).unwrap_or(1000).min(1000)) / 1000.0;
            let height = (16.0 * ratio).max(2.0);
            let x = inner.left() + f32::from(u16::try_from(index).unwrap_or(0)) * (bar_width + 4.0);
            p.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x, inner.top() + 187.0 - height),
                    Vec2::new(bar_width.max(1.0), height),
                ),
                2,
                if index + 1 == values.len() {
                    accent
                } else {
                    accent.gamma_multiply(0.35)
                },
            );
        }
        fitted(
            p,
            inner.min + Vec2::new(0.0, 193.0),
            &format!("{seconds}초 전 집계 · 모델 요청이 끝날 때 갱신"),
            11.0,
            theme::MUTED,
            inner.width(),
            1,
        );
    }
    ui.interact(rect, ui.id().with("session_token_usage"), Sense::hover()).on_hover_text(format!(
        "출처: 연결된 Mac Agent의 Codex 세션 로그 (읽기 전용)\n{scope}: {number} 토큰\n입력에 캐시 {} 포함 · 출력에 추론 {} 포함\n막대: 최근 모델 요청별 토큰 수 (초당 속도 아님)\n계정 전체 누적량이나 결제 금액이 아닙니다.\n집계 시각: {}\n프로세스 생존 여부는 별도로 확인하지 않습니다.",
        separated(counted.cached_input_tokens), separated(counted.reasoning_output_tokens), usage.updated_at));
}

fn separated(value: i64) -> String {
    let digits = value.max(0).to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

fn card(p: &egui::Painter, rect: Rect, warm: bool) {
    p.rect_filled(
        rect,
        16,
        if warm {
            Color32::from_rgb(31, 25, 23)
        } else {
            theme::PANEL
        },
    );
    p.rect_stroke(
        rect,
        16,
        Stroke::new(
            1.0,
            if warm {
                Color32::from_rgb(70, 44, 32)
            } else {
                theme::BORDER
            },
        ),
        egui::StrokeKind::Inside,
    );
}

fn text(p: &egui::Painter, at: Pos2, text: &str, size: f32, color: Color32) {
    p.text(
        at,
        Align2::LEFT_TOP,
        text,
        FontId::proportional(size),
        color,
    );
}

fn fitted(
    p: &egui::Painter,
    at: Pos2,
    value: &str,
    size: f32,
    color: Color32,
    width: f32,
    lines: usize,
) {
    let mut job = egui::text::LayoutJob::simple(
        value.to_owned(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    job.wrap.max_rows = lines;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    p.galley(at, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn mac_spark_window_and_large_full_token_count_are_visible_without_horizontal_scrolling() {
        let mut snapshot = test_support::snapshot();
        let now = Utc::now().timestamp();
        let limits = snapshot.codex.limits.as_mut().unwrap();
        limits.primary = limits.secondary.take();
        limits.additional = vec![serde_json::from_value(serde_json::json!({
            "id":"spark", "name":"GPT-5.3-Codex-Spark",
            "primary":{"used_percent":0,"remaining_percent":100,"window_duration_minutes":300,"resets_at":now+3600},
            "secondary":{"used_percent":7,"remaining_percent":93,"window_duration_minutes":10080,"resets_at":now+86400}
        })).unwrap()];
        for (width, height) in [(676.0, 442.0), (894.0, 442.0), (894.0, 380.0)] {
            let text = test_support::render_text(width, height, |ui| {
                render(ui, &snapshot, 0, true, true, height);
            });
            for value in [
                "192,360",
                "5시간 한도 · 남음",
                "100%",
                "93%",
                "58%",
                "사용 0%",
                "사용 7%",
                "사용 42%",
                "GPT-5.3-Codex-Spark",
            ] {
                let label = text
                    .iter()
                    .find(|label| label.text == value)
                    .unwrap_or_else(|| panic!("Missing {value}"));
                assert!(
                    label.clip.expand(1.0).contains_rect(label.rect),
                    "{width}: {value}: {:?} vs {:?}",
                    label.rect,
                    label.clip
                );
                if value == "192,360" {
                    assert!(
                        label.font_size >= 80.0,
                        "main count too small: {}",
                        label.font_size
                    );
                }
            }
            let reset_notes: Vec<_> = text
                .iter()
                .filter(|label| label.text.ends_with("후 초기화"))
                .collect();
            assert_eq!(reset_notes.len(), 3);
            for label in reset_notes {
                assert!(label.clip.expand(1.0).contains_rect(label.rect));
            }
            let remaining = text.iter().find(|label| label.text == "100%").unwrap();
            let used = text.iter().find(|label| label.text == "사용 0%").unwrap();
            assert!(remaining.rect.right() < used.rect.left());
        }
    }

    #[test]
    fn live_keeps_recorded_tokens_but_does_not_present_disconnected_quotas_as_current() {
        let snapshot = test_support::snapshot();
        for width in [600.0, 930.0] {
            let current = test_support::render(width, 432.0, |ui| {
                render(ui, &snapshot, 0, true, true, 432.0);
            });
            for expected in ["5시간 한도", "주간 한도", "192,360", "project-a"] {
                assert!(current.contains(expected), "{expected}: {current}");
            }
            let stale = test_support::render(width, 432.0, |ui| {
                render(ui, &snapshot, 0, false, true, 432.0);
            });
            assert!(stale.contains("192,360"));
            assert!(stale.contains("연결 끊김 · 기록"));
            assert!(!stale.contains("31%"));
        }
    }
    #[test]
    fn completed_or_slow_counts_survive_until_a_new_turn_and_keep_their_timestamp() {
        let mut snapshot = test_support::snapshot();
        let thread = &mut snapshot.codex.threads[0];
        let now = Utc::now().timestamp();
        let usage = thread.live_usage.as_mut().unwrap();
        usage.updated_at -= chrono::Duration::minutes(30);
        usage.status = CodexThreadStatusDto::Completed;
        assert!(usage_caption(usage, true, now).0.starts_with("완료 "));
        assert!(
            usage_caption(usage, false, now)
                .0
                .starts_with("연결 끊김 · 기록 ")
        );
        assert!(
            usage_caption(usage, true, now).0.contains(
                &usage
                    .updated_at
                    .with_timezone(&chrono::Local)
                    .format("%m/%d %H:%M")
                    .to_string()
            )
        );
        assert!(visible_usage(thread, now).is_some());
        let usage = thread.live_usage.as_mut().unwrap();
        usage.status = CodexThreadStatusDto::Working;
        assert!(usage_caption(usage, true, now).0.starts_with("집계 대기 "));
        usage.observed_at -= chrono::Duration::minutes(2);
        assert!(
            usage_caption(usage, true, now)
                .0
                .starts_with("수신 지연 · 기록 ")
        );
        assert!(visible_usage(thread, now).is_some());
        thread.active_turn_id = Some("next".to_owned());
        assert!(visible_usage(thread, now).is_none());
        thread.active_turn_id = Some("new".to_owned());
        thread.live_usage.as_mut().unwrap().updated_at = Utc::now() + chrono::Duration::minutes(1);
        assert!(visible_usage(thread, now).is_none());
    }
}
