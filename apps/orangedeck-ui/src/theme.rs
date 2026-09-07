use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Vec2};

pub const BG: Color32 = Color32::from_rgb(12, 14, 21);
pub const PANEL: Color32 = Color32::from_rgb(22, 25, 35);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(30, 34, 47);
pub const BORDER: Color32 = Color32::from_rgb(49, 55, 73);
pub const TEXT: Color32 = Color32::from_rgb(239, 243, 246);
pub const MUTED: Color32 = Color32::from_rgb(145, 156, 166);
pub const ORANGE: Color32 = Color32::from_rgb(255, 126, 79);
pub const CYAN: Color32 = Color32::from_rgb(46, 211, 198);
pub const GREEN: Color32 = Color32::from_rgb(89, 219, 139);
pub const PINK: Color32 = Color32::from_rgb(255, 71, 126);
pub const RED: Color32 = Color32::from_rgb(255, 85, 85);
pub const YELLOW: Color32 = Color32::from_rgb(247, 196, 72);
pub const VIOLET: Color32 = Color32::from_rgb(166, 143, 255);
pub const BLUE: Color32 = Color32::from_rgb(113, 169, 255);

pub fn tint(base: Color32, accent: Color32, amount: f32) -> Color32 {
    egui::lerp(egui::Rgba::from(base)..=egui::Rgba::from(accent), amount).into()
}

pub fn apply(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "korean".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("korean".to_owned());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(16.0, 12.0);
    style.spacing.interact_size = Vec2::new(48.0, 48.0);
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = PANEL;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
    style.visuals.widgets.inactive.weak_bg_fill = PANEL_RAISED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 47, 54);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(40, 47, 54);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5, ORANGE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(55, 44, 37);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(55, 44, 37);
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, ORANGE);
    style.visuals.selection.bg_fill = ORANGE;
    style.visuals.selection.stroke = Stroke::new(1.0, TEXT);
    style.visuals.window_corner_radius = CornerRadius::same(16);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    style.visuals.override_text_color = Some(TEXT);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(16.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style_of(egui::Theme::Dark, style);
}

pub fn panel() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(14.0)
}

pub fn accent_panel(color: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(2.0, color))
        .corner_radius(12)
        .inner_margin(14.0)
}
