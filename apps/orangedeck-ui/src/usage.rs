use chrono::Utc;
use eframe::egui::{self, Color32, RichText};
use orangedeck_protocol::{RateLimitWindowDto, SnapshotDto};

use crate::theme;

pub struct Metric {
    pub scope: Option<String>,
    pub label: String,
    pub value: String,
    pub note: String,
    pub detail: String,
    pub color: Color32,
    pub fraction: Option<f32>,
    pub used_label: Option<String>,
}

pub fn number(value: i64) -> String {
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

fn compact_tokens(value: i64) -> String {
    let (unit, suffix) = if value >= 1_000_000_000_000 {
        (1_000_000_000_000, "조")
    } else if value >= 100_000_000 {
        (100_000_000, "억")
    } else if value >= 10_000 {
        (10_000, "만")
    } else {
        return number(value);
    };
    let hundredths = (i128::from(value) * 100 + unit / 2) / unit;
    format!("약 {}.{:02}{suffix}", hundredths / 100, hundredths % 100)
}

fn window_name(minutes: Option<i64>) -> String {
    match minutes {
        Some(300) => "5시간 한도".to_owned(),
        Some(10_080) => "주간 한도".to_owned(),
        Some(value) if value > 0 && value % 60 == 0 => format!("{}시간 한도", value / 60),
        Some(value) if value > 0 => format!("{value}분 한도"),
        _ => "기간 미제공 한도".to_owned(),
    }
}

fn reset(at: Option<i64>, now: i64) -> String {
    match at.map(|at| at - now) {
        None => "초기화 시각 미제공".to_owned(),
        Some(value) if value <= 0 => "새 한도 수신 대기".to_owned(),
        Some(value) if value >= 86_400 => format!(
            "{}일 {}시간 후 초기화",
            value / 86_400,
            value % 86_400 / 3600
        ),
        Some(value) if value >= 3600 => {
            format!("{}시간 {}분 후 초기화", value / 3600, value % 3600 / 60)
        }
        Some(value) => format!("{}분 후 초기화", (value + 59) / 60),
    }
}

fn quota(scope: &str, window: &RateLimitWindowDto, current: bool, now: i64) -> Metric {
    let valid = current && window.resets_at.is_none_or(|at| at > now);
    Metric {
        scope: Some(scope.to_owned()),
        label: format!("{} · 남음", window_name(window.window_duration_minutes)),
        value: if valid {
            format!("{}%", window.remaining_percent.clamp(0, 100))
        } else {
            "—".to_owned()
        },
        note: if current {
            reset(window.resets_at, now)
        } else {
            "새 수치 수신 대기".to_owned()
        },
        detail: format!(
            "{scope}에 적용되는 OpenAI 공식 사용 한도입니다.\n{}% 남음 · {}\n막대는 남은 비율입니다. 사용 비율을 옆에 표시하며, 토큰 개수로 환산하지 않습니다.",
            window.remaining_percent.clamp(0, 100),
            reset(window.resets_at, now)
        ),
        color: theme::CYAN,
        fraction: valid.then(|| {
            f32::from(u8::try_from(window.remaining_percent.clamp(0, 100)).unwrap_or(0)) / 100.0
        }),
        used_label: valid.then(|| format!("사용 {}%", window.used_percent.clamp(0, 100))),
    }
}

pub fn metrics(snapshot: &SnapshotDto, connected: bool, now: i64) -> Vec<Metric> {
    let limits = snapshot.codex.limits.as_ref();
    let fresh = connected
        && limits
            .and_then(|value| value.updated_at)
            .is_some_and(|at| (-5..=45).contains(&(now - at.timestamp())));
    let mut result = Vec::new();
    if let Some(limits) = limits {
        let scope = limits
            .limit_name
            .as_deref()
            .unwrap_or(match limits.limit_id.as_deref() {
                None | Some("codex") => "Codex",
                Some(id) => id,
            });
        for window in [limits.primary.as_ref(), limits.secondary.as_ref()]
            .into_iter()
            .flatten()
        {
            result.push(quota(scope, window, fresh, now));
        }
        for bucket in &limits.additional {
            for window in [bucket.primary.as_ref(), bucket.secondary.as_ref()]
                .into_iter()
                .flatten()
            {
                result.push(quota(
                    bucket.name.as_deref().unwrap_or(&bucket.id),
                    window,
                    fresh,
                    now,
                ));
            }
        }
    }
    let account = snapshot.codex.account_usage.as_ref();
    let account_fresh = connected
        && account.is_some_and(|value| {
            value.unavailable_reason.is_none()
                && value
                    .updated_at
                    .is_some_and(|at| (-5..=180).contains(&(now - at.timestamp())))
        });
    let mut token_metric = |label: String, value: i64| {
        result.push(Metric {
            scope: None,
            label,
            value: if account_fresh { compact_tokens(value) } else { "—".to_owned() },
            note: if account_fresh { format!("{} 토큰", number(value)) } else { "새 통계 수신 대기".to_owned() },
            detail: account.and_then(|value| value.updated_at).map_or_else(String::new, |at| format!(
                "Codex 서버가 반환한 값 · 조회 {}\n누적 시작일·대상 제품/기기·입력/출력/캐시 내역은 응답에 없습니다.\n일일 최대 기록의 날짜도 별도 제공되지 않습니다.\n이 Mac에서 생성한 답변 토큰만의 통계로 해석할 수 없습니다.\n큰 숫자는 읽기 쉬운 근삿값, 아래 숫자는 받은 정수 그대로입니다.",
                at.format("%Y-%m-%d %H:%M:%S UTC"))),
            color: theme::ORANGE,
            fraction: None,
            used_label: None,
        });
    };
    if let Some(day) = account.and_then(|value| value.daily.first()) {
        token_metric(format!("{} 토큰", day.date), day.tokens);
    }
    if let Some(value) = account.and_then(|value| value.lifetime_tokens) {
        token_metric("누적 토큰 · 서버 제공".to_owned(), value);
    }
    if let Some(value) = account.and_then(|value| value.peak_daily_tokens) {
        token_metric("일일 최대 · 서버 제공".to_owned(), value);
    }
    if let Some(balance) = limits.and_then(|value| value.credits_balance.as_deref()) {
        result.push(Metric {
            scope: None,
            label: "남은 크레딧".to_owned(),
            value: if fresh {
                balance.to_owned()
            } else {
                "—".to_owned()
            },
            note: "OpenAI 공식 잔액".to_owned(),
            detail: "계정 크레딧 잔액입니다. 토큰 수가 아닙니다.".to_owned(),
            color: theme::GREEN,
            fraction: None,
            used_label: None,
        });
    }
    result
}

pub fn render(ui: &mut egui::Ui, snapshot: &SnapshotDto, connected: bool) {
    let entries = metrics(snapshot, connected, Utc::now().timestamp());
    ui.spacing_mut().item_spacing.y = 6.0;
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("공식 남은 한도 · 계정 기준 · % 남음")
                    .size(15.0)
                    .color(theme::CYAN),
            );
            if let Some(at) = snapshot
                .codex
                .limits
                .as_ref()
                .and_then(|limits| limits.updated_at)
            {
                ui.label(
                    RichText::new(format!(
                        "확인 {}",
                        at.with_timezone(&chrono::Local).format("%H:%M:%S")
                    ))
                    .size(11.0)
                    .color(theme::MUTED),
                );
            }
        });
    });
    let quotas: Vec<_> = entries
        .iter()
        .filter(|entry| entry.scope.is_some())
        .collect();
    if quotas.is_empty() {
        ui.label(
            RichText::new("OpenAI에서 제공한 사용 한도를 기다리고 있습니다.").color(theme::MUTED),
        );
    } else {
        grid(ui, &quotas);
        let limits = snapshot.codex.limits.as_ref();
        if limits.is_some_and(|limits| {
            matches!(limits.limit_id.as_deref(), None | Some("codex"))
                && [limits.primary.as_ref(), limits.secondary.as_ref()]
                    .into_iter()
                    .flatten()
                    .all(|window| window.window_duration_minutes != Some(300))
        }) {
            ui.label(RichText::new("Codex 공통 5시간 한도는 현재 공식 응답에 없습니다. 모델 전용 한도는 위 이름을 확인하세요.")
                .size(12.0).color(theme::MUTED));
        }
    }
    let account: Vec<_> = entries
        .iter()
        .filter(|entry| entry.scope.is_none())
        .collect();
    if !account.is_empty() {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Codex 서버 계정 통계 · 상세 집계 기준 미제공")
                .size(15.0)
                .color(theme::ORANGE),
        );
        ui.label(
            RichText::new(
                "누적 시작일·기기/제품 범위·입출력/캐시 구분·최고 기록 날짜는 응답에 없습니다.",
            )
            .size(12.0)
            .color(theme::MUTED),
        );
        grid(ui, &account);
    }
}

fn grid(ui: &mut egui::Ui, entries: &[&Metric]) {
    // Every returned window gets a visible row. No horizontal carousel or fixed clipping area.
    for row in entries.chunks(3) {
        ui.columns(3, |columns| {
            for (column, metric) in columns.iter_mut().zip(row) {
                theme::panel()
                    .show(column, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.set_min_height(100.0);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        if let Some(scope) = &metric.scope {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(scope).size(12.0).color(theme::MUTED),
                                )
                                .wrap(),
                            );
                        }
                        ui.add(
                            egui::Label::new(RichText::new(&metric.label).size(15.0)).truncate(),
                        );
                        if let Some(fraction) = metric.fraction {
                            let used = ui.painter().layout_no_wrap(
                                metric.used_label.clone().unwrap_or_default(),
                                egui::FontId::proportional(11.0),
                                theme::TEXT,
                            );
                            let value_width = ui
                                .painter()
                                .layout_no_wrap(
                                    metric.value.clone(),
                                    egui::FontId::proportional(36.0),
                                    theme::TEXT,
                                )
                                .size()
                                .x;
                            let size = 36.0
                                * ((ui.available_width() - used.size().x - 37.0).max(1.0)
                                    / value_width.max(1.0))
                                .min(1.0);
                            let response = ui.add(
                                egui::ProgressBar::new(fraction)
                                    // A rounded egui bar enforces a nonzero minimum fill, even at 0%.
                                    .corner_radius(0)
                                    .animate(false)
                                    .desired_width(ui.available_width())
                                    .desired_height(47.0)
                                    .fill(metric.color.gamma_multiply(0.45))
                                    .text(
                                        RichText::new(&metric.value)
                                            .size(size)
                                            .strong()
                                            .color(theme::TEXT),
                                    ),
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    response.rect.right() - used.size().x - 6.0,
                                    response.rect.center().y - used.size().y * 0.5,
                                ),
                                used,
                                theme::TEXT,
                            );
                        } else {
                            let measured = ui.painter().layout_no_wrap(
                                metric.value.clone(),
                                egui::FontId::proportional(40.0),
                                metric.color,
                            );
                            let size =
                                40.0 * (ui.available_width() / measured.size().x.max(1.0)).min(1.0);
                            ui.add(egui::Label::new(
                                RichText::new(&metric.value).size(size).color(metric.color),
                            ));
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(&metric.note).size(11.0).color(theme::MUTED),
                            )
                            .wrap(),
                        );
                    })
                    .response
                    .on_hover_text(&metric.detail);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_account_counts_keep_exact_integers_beside_readable_units_and_scope_limits() {
        let mut snapshot = crate::test_support::snapshot();
        snapshot.codex.account_usage = Some(
            serde_json::from_value(serde_json::json!({
                "lifetime_tokens":32_123_456_789_i64, "peak_daily_tokens":1_234_567_890_i64,
                "daily":[{"date":"2026-09-05","tokens":234_567_890}],
                "updated_at":Utc::now()
            }))
            .unwrap(),
        );
        let rendered =
            crate::test_support::render_text(676.0, 700.0, |ui| render(ui, &snapshot, true));
        for expected in [
            "약 321.23억",
            "약 12.35억",
            "약 2.35억",
            "32,123,456,789 토큰",
            "1,234,567,890 토큰",
            "234,567,890 토큰",
        ] {
            let label = rendered
                .iter()
                .find(|label| label.text == expected)
                .unwrap_or_else(|| panic!("Missing {expected}"));
            assert!(label.clip.contains_rect(label.rect));
        }
        assert!(
            rendered
                .iter()
                .any(|label| label.text.contains("상세 집계 기준 미제공"))
        );
        let disconnected = metrics(&snapshot, false, Utc::now().timestamp());
        assert!(disconnected.iter().all(|metric| metric.value == "—"));
        assert_eq!(compact_tokens(9999), "9,999");
        assert_eq!(compact_tokens(10000), "약 1.00만");
        assert_eq!(compact_tokens(i64::MAX), "약 9223372.04조");
    }

    #[test]
    fn official_windows_are_not_token_counts_and_expiry_never_invents_a_reset() {
        let window = RateLimitWindowDto {
            used_percent: 31,
            remaining_percent: 69,
            resets_at: Some(1100),
            window_duration_minutes: Some(300),
        };
        assert_eq!(quota("Codex", &window, true, 1000).value, "69%");
        assert_eq!(quota("Codex", &window, true, 1000).fraction, Some(0.69));
        assert_eq!(quota("Codex", &window, true, 1100).fraction, None);
        assert_eq!(quota("Codex", &window, false, 1000).fraction, None);
        assert_eq!(quota("Codex", &window, true, 1100).value, "—");
        assert_eq!(quota("Codex", &window, false, 1000).value, "—");
    }

    #[test]
    fn mac_weekly_and_spark_five_hour_remain_distinct_without_an_invented_common_window() {
        let mut snapshot = crate::test_support::snapshot();
        let now = Utc::now().timestamp();
        let limits = snapshot.codex.limits.as_mut().unwrap();
        limits.primary = limits.secondary.take();
        limits.additional = vec![serde_json::from_value(serde_json::json!({
            "id":"codex_bengalfox","name":"GPT-5.3-Codex-Spark",
            "primary":{"used_percent":0,"remaining_percent":100,"window_duration_minutes":300,"resets_at":now+3600},
            "secondary":{"used_percent":7,"remaining_percent":93,"window_duration_minutes":10080,"resets_at":now+86400}
        })).unwrap()];
        let entries = metrics(&snapshot, true, now);
        let quotas: Vec<_> = entries
            .iter()
            .filter(|entry| entry.scope.is_some())
            .collect();
        assert_eq!(quotas.len(), 3);
        assert_eq!(quotas[0].scope.as_deref(), Some("Codex"));
        assert_eq!(quotas[0].label, "주간 한도 · 남음");
        assert_eq!(quotas[1].scope.as_deref(), Some("GPT-5.3-Codex-Spark"));
        assert_eq!(quotas[1].label, "5시간 한도 · 남음");
        assert_eq!(quotas[1].value, "100%");
        assert_eq!(quotas[1].fraction, Some(1.0));
        assert_eq!(quotas[1].used_label.as_deref(), Some("사용 0%"));
        snapshot.codex.limits = None;
        assert!(
            metrics(&snapshot, true, now)
                .iter()
                .all(|entry| entry.scope.is_none())
        );
    }
}
