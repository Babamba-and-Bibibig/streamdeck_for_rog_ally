use super::{
    Align, ControlAction, Language, OrangeDeckApp, Page, RichText, Stroke, UiPreferenceStore, egui,
    i18n, shortcuts, theme,
};
use orangedeck_application::{ShortcutEffect, ShortcutUnavailable, resolve_shortcut};
use orangedeck_domain::Shortcut;

pub(super) struct EditorState {
    key: usize,
    selected: usize,
    scroll: bool,
}

impl OrangeDeckApp {
    pub fn load_preferences(&mut self, ctx: &egui::Context, path: Option<std::path::PathBuf>) {
        if let Some(path) = path {
            let store = UiPreferenceStore::new(path);
            match store.load() {
                Ok(preferences) => {
                    self.preferences = preferences;
                    self.preference_store = Some(store);
                }
                Err(_) => {
                    // Keep the original file for recovery; this session stays usable.
                    self.preference_warning = Some("preferences_unreadable".to_owned());
                }
            }
        }
        i18n::set_language(ctx, self.preferences.language);
    }

    fn save_preferences(&mut self) {
        if let Some(store) = &self.preference_store {
            self.preference_warning = store
                .save(&self.preferences)
                .err()
                .map(|_| "preferences_unsaved".to_owned());
        }
    }

    pub fn change_language(&mut self, ctx: &egui::Context, language: Language) {
        self.preferences.language = language;
        self.displayed_approval = None;
        i18n::set_language(ctx, language);
        self.save_preferences();
    }

    pub(super) fn open_key_editor(&mut self, key: usize) {
        if !(2..10).contains(&key) {
            return;
        }
        let selected = self
            .preferences
            .action(key)
            .and_then(|action| Shortcut::ALL.iter().position(|entry| *entry == action))
            .unwrap_or(0);
        self.editor = Some(EditorState {
            key,
            selected,
            scroll: true,
        });
        self.displayed_approval = None;
    }

    pub(super) fn activate_custom_key(&mut self, key: usize) {
        if !(2..10).contains(&key) {
            return;
        }
        let Some(action) = self.preferences.action(key).filter(|_| !self.deck_editing) else {
            self.open_key_editor(key);
            return;
        };
        self.run_shortcut(action);
    }

    fn run_shortcut(&mut self, action: Shortcut) {
        let lang = self.preferences.language;
        match resolve_shortcut(
            action,
            self.model.snapshot.as_ref(),
            self.selection.project.as_deref(),
            self.model.connected,
        ) {
            Ok(ShortcutEffect::Remote(command)) => {
                self.model.command_message = Some(
                    lang.text("메인 PC에 요청을 보냈습니다.", "Request sent to your host.")
                        .to_owned(),
                );
                self.send(command);
            }
            Ok(ShortcutEffect::Local(action)) => match action {
                Shortcut::Live => self.select_page(Page::Dashboard),
                Shortcut::Projects => self.select_page(Page::Projects),
                Shortcut::Conversations => self.select_page(Page::Codex),
                Shortcut::Notifications => self.open_notifications(),
                Shortcut::PreviousProject => self.change_project(-1),
                Shortcut::NextProject => self.change_project(1),
                Shortcut::PreviousConversation => self.change_thread(-1),
                Shortcut::NextConversation => self.change_thread(1),
                Shortcut::FollowLatest => {
                    self.selection.follow_latest = true;
                    self.update_monitor_selection();
                    self.displayed_approval = None;
                }
                _ => {}
            },
            Err(error) => {
                self.model.command_message = Some(
                    match error {
                        ShortcutUnavailable::Disconnected => lang.text(
                            "Agent에 연결한 뒤 이 키를 사용하세요.",
                            "Connect to your Agent to use this key.",
                        ),
                        ShortcutUnavailable::UnregisteredProject => lang.text(
                            "메인 PC 열기 기능은 Agent에 등록한 프로젝트에서 사용할 수 있습니다.",
                            "Host shortcuts require a project registered with your Agent.",
                        ),
                        ShortcutUnavailable::MissingBrowser => lang.text(
                            "이 프로젝트에는 열 웹 주소가 설정되어 있지 않습니다.",
                            "This project has no configured website.",
                        ),
                    }
                    .to_owned(),
                );
            }
        }
    }

    pub(super) fn use_recommended_keys(&mut self) {
        let recommended = [
            Shortcut::Refresh,
            Shortcut::FollowLatest,
            Shortcut::Conversations,
            Shortcut::Notifications,
            Shortcut::PreviousConversation,
            Shortcut::NextConversation,
            Shortcut::OpenEditor,
            Shortcut::OpenTerminal,
        ];
        for (slot, action) in self.preferences.shortcuts.iter_mut().zip(recommended) {
            if *slot == Shortcut::Unassigned {
                *slot = action;
            }
        }
        self.save_preferences();
    }

    fn set_edited_key(&mut self, action: Option<Shortcut>) {
        if let Some(editor) = self.editor.take() {
            self.preferences.assign(editor.key, action);
            self.focus_index = editor.key;
            self.save_preferences();
        }
        self.displayed_approval = None;
    }

    pub(super) fn handle_editor_input(&mut self, action: ControlAction) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        match action {
            ControlAction::Back => {
                self.editor = None;
                self.displayed_approval = None;
            }
            ControlAction::Activate => {
                let selected = Shortcut::ALL[editor.selected];
                self.set_edited_key(Some(selected));
            }
            ControlAction::NavigateLeft
            | ControlAction::NavigateUp
            | ControlAction::NavigateRight
            | ControlAction::NavigateDown => {
                let delta = match action {
                    ControlAction::NavigateLeft => -1,
                    ControlAction::NavigateRight => 1,
                    ControlAction::NavigateUp => -2,
                    _ => 2,
                };
                editor.selected = editor
                    .selected
                    .saturating_add_signed(delta)
                    .min(Shortcut::ALL.len() - 1);
                editor.scroll = true;
            }
            _ => {}
        }
    }

    pub(super) fn render_key_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let lang = self.preferences.language;
        let mut chosen = None;
        let mut clear = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("key_editor")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).clamp(300.0, 600.0));
            ui.heading(if lang == Language::Korean {
                format!("{:02}번 키에 기능 연결", editor.key + 1)
            } else {
                format!("Assign key {:02}", editor.key + 1)
            });
            ui.label(lang.text(
                "기능을 고르면 저장됩니다. 실행은 키를 누를 때만 합니다.",
                "Choose to save an action. It runs only when you press its key.",
            ));
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 220.0).clamp(140.0, 420.0))
                .show(ui, |ui| {
                    for (row, actions) in Shortcut::ALL.chunks(2).enumerate() {
                        ui.columns(2, |columns| {
                            for (column, action) in columns.iter_mut().zip(actions) {
                                let index = Shortcut::ALL
                                    .iter()
                                    .position(|entry| entry == action)
                                    .unwrap_or(row * 2);
                                let (title, hint) = shortcuts::shortcut_label(*action, lang);
                                let button = egui::Button::new(
                                    RichText::new(format!("{title}\n{hint}")).size(13.0),
                                )
                                .fill(theme::PANEL_RAISED)
                                .stroke(Stroke::new(
                                    if editor.selected == index { 2.0 } else { 1.0 },
                                    if editor.selected == index {
                                        shortcuts::shortcut_color(*action)
                                    } else {
                                        theme::BORDER
                                    },
                                ));
                                let response =
                                    column.add_sized([column.available_width(), 62.0], button);
                                if editor.selected == index && editor.scroll {
                                    response.scroll_to_me(Some(Align::Center));
                                }
                                if response.clicked() {
                                    chosen = Some(*action);
                                }
                            }
                        });
                    }
                });
            editor.scroll = false;
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(lang.text("키 비우기", "Clear key")).clicked() {
                    clear = true;
                }
                if ui.button(lang.text("취소 · B", "Cancel · B")).clicked() {
                    cancel = true;
                }
                ui.label(
                    RichText::new(lang.text("방향키 선택 · A 저장", "D-pad selects · A saves"))
                        .small()
                        .color(theme::MUTED),
                );
            });
        });
        if let Some(action) = chosen {
            self.set_edited_key(Some(action));
        } else if clear {
            self.set_edited_key(None);
        } else if cancel || response.should_close() {
            self.editor = None;
            self.displayed_approval = None;
        }
    }
}
