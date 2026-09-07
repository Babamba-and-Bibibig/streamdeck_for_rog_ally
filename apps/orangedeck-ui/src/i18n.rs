//! Per-window language selection. Work content and host-provided text are never translated.
use eframe::egui;
pub use orangedeck_domain::UiLanguage as Language;

pub fn language(ctx: &egui::Context) -> Language {
    ctx.data(|data| data.get_temp(egui::Id::new("ui_language")))
        .unwrap_or_default()
}

pub fn set_language(ctx: &egui::Context, language: Language) {
    if self::language(ctx) == language {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ui_language"), language));
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_is_window_local_and_changes_without_restarting() {
        let first = egui::Context::default();
        let second = egui::Context::default();
        set_language(&first, Language::English);
        assert_eq!(language(&first), Language::English);
        assert_eq!(language(&second), Language::Korean);
        set_language(&first, Language::Korean);
        assert_eq!(language(&first), Language::Korean);
    }
}
