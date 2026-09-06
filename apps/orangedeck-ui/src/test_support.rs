use eframe::egui;
use orangedeck_protocol::SnapshotDto;

pub fn snapshot() -> SnapshotDto {
    let now = chrono::Utc::now();
    serde_json::from_value(serde_json::json!({
        "host":{"name":"TEST MAC","os":"test","architecture":"test","address":null,"state":"connected","tailscale":false,"latency_ms":null,"last_seen":now},
        "projects":[],"selected_project_id":null,"jobs":[],"git":[],"system":{},"activity":[],
        "codex":{"connection":{"state":"connected","version":null,"compatible":true,"message":null},
        "threads":[{"id":"a","cwd":"/Users/mac/project-a","title":"Current conversation","preview":"Old preview","status":"working","ownership":"external_read_only","updated_at":now.timestamp(),"active_turn_id":"new",
            "observation":{"turn_id":"new","latest_user_prompt":"CURRENT QUESTION","latest_agent_message":"CURRENT ANSWER","last_turn_status":"working","observed_at":now},
            "live_usage":{"turn_id":"new","turn_tokens":{"input_tokens":180_000,"output_tokens":12_360,"total_tokens":192_360,"cached_input_tokens":0,"reasoning_output_tokens":0},"last_request":{"input_tokens":180_000,"output_tokens":12_360,"total_tokens":192_360,"cached_input_tokens":0,"reasoning_output_tokens":0},"recent_requests":[1234,192_360],"status":"working","observed_at":now,"updated_at":now}},
            {"id":"b","cwd":"/Users/mac/project-b","title":"Other conversation","preview":"OTHER PROJECT QUESTION","status":"not_loaded","ownership":"external_read_only","updated_at":now.timestamp()-10}],
        "pending_approvals":[],"limits":{"primary":{"used_percent":31,"remaining_percent":69,"window_duration_minutes":300,"resets_at":now.timestamp()+3600},"secondary":{"used_percent":42,"remaining_percent":58,"window_duration_minutes":10_080,"resets_at":now.timestamp()+86_400},"updated_at":now},
        "account_usage":{"lifetime_tokens":1_234_567,"daily":[{"date":"2026-09-06","tokens":654_321}],"updated_at":now},"supported_features":["read_only_monitor"]}
    })).unwrap()
}

pub struct RenderedText {
    pub text: String,
    pub rect: egui::Rect,
    pub clip: egui::Rect,
    pub font_size: f32,
}

pub fn render_text(
    width: f32,
    height: f32,
    mut contents: impl FnMut(&mut egui::Ui),
) -> Vec<RenderedText> {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    // Areas measure their contents before painting. Inspect the settled frame, as on screen.
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            time: Some(0.0),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| contents(ui));
        },
    )
    .drop_without_applying_deltas();
    let output = ctx.run_ui(
        egui::RawInput {
            time: Some(1.0),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| contents(ui));
        },
    );
    let mut labels = Vec::new();
    for shape in &output.shapes {
        let bounds = shape
            .shape
            .visual_bounding_rect()
            .intersect(shape.clip_rect);
        if bounds.is_positive() {
            assert!(bounds.min.is_finite() && bounds.max.is_finite());
            assert!(
                bounds.max.x <= width + 2.0 && bounds.max.y <= height + 2.0,
                "{bounds:?}"
            );
        }
        if let egui::Shape::Text(text) = &shape.shape {
            labels.push(RenderedText {
                text: text.galley.job.text.clone(),
                rect: text.galley.rect.translate(text.pos.to_vec2()),
                clip: shape.clip_rect.intersect(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                font_size: text
                    .galley
                    .job
                    .sections
                    .first()
                    .map_or(0.0, |section| section.format.font_id.size),
            });
        }
    }
    output.drop_without_applying_deltas();
    labels
}

pub fn render(width: f32, height: f32, contents: impl FnMut(&mut egui::Ui)) -> String {
    render_text(width, height, contents)
        .into_iter()
        .filter(|label| label.rect.intersects(label.clip))
        .map(|label| label.text)
        .collect::<Vec<_>>()
        .join("\n")
}
