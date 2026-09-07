use super::{ControlAction, Language, OrangeDeckApp, UiPreferenceStore, egui, i18n, theme};
use orangedeck_domain::ConversationSlot;

pub(super) struct EditorState {
    key: usize,
    selected: usize,
    label: String,
    just_opened: bool,
    threads: Vec<orangedeck_protocol::CodexThreadDto>,
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
                    self.preference_warning = Some("preferences_unreadable".to_owned());
                }
            }
        }
        i18n::set_language(ctx, self.preferences.language);
    }

    pub(super) fn save_preferences(&mut self) {
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

    pub(super) fn open_key_editor(&mut self, column: usize) {
        if column >= 5 {
            return;
        }
        self.close_deck_modal();
        let slot = &self.preferences.conversations[column];
        let threads = self.picker_threads();
        let selected = threads
            .iter()
            .position(|thread| thread.id == slot.thread_id)
            .unwrap_or(0);
        self.editor = Some(EditorState {
            key: column,
            selected,
            just_opened: true,
            threads,
            label: if slot.label.is_empty() {
                format!("Codex {}", column + 1)
            } else {
                slot.label.clone()
            },
        });
        self.displayed_approval = None;
    }

    fn picker_threads(&self) -> Vec<orangedeck_protocol::CodexThreadDto> {
        let mut threads = self
            .model
            .snapshot
            .as_ref()
            .map_or_else(Vec::new, |snapshot| snapshot.codex.threads.clone());
        threads.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        threads
    }

    fn assign_conversation(&mut self, thread_id: Option<&str>) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        if let Some(id) = thread_id {
            if !self.picker_threads().iter().any(|thread| thread.id == id) {
                self.editor = Some(editor);
                return;
            }
            if self
                .preferences
                .conversations
                .iter()
                .enumerate()
                .any(|(index, slot)| index != editor.key && slot.thread_id == id)
            {
                self.editor = Some(editor);
                return;
            }
            let label = editor
                .label
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(40)
                .collect();
            self.preferences.conversations[editor.key] = ConversationSlot {
                thread_id: id.to_owned(),
                label,
            };
        } else {
            self.preferences.conversations[editor.key] = ConversationSlot::default();
        }
        self.watched_deck = None;
        self.displayed_approval = None;
        self.save_preferences();
    }

    pub(super) fn handle_editor_input(&mut self, action: ControlAction) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let threads = editor.threads.clone();
        match action {
            ControlAction::Back => {
                self.editor = None;
                self.displayed_approval = None;
            }
            ControlAction::Activate => {
                if let Some(thread) = threads.get(editor.selected) {
                    self.assign_conversation(Some(&thread.id));
                }
            }
            ControlAction::NavigateUp => editor.selected = editor.selected.saturating_sub(1),
            ControlAction::NavigateDown => {
                editor.selected = (editor.selected + 1).min(threads.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    pub(super) fn render_key_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        // Keep choices stable while a finger/controller selection is in progress.
        let threads = editor.threads.clone();
        let lang = self.preferences.language;
        let just_opened = editor.just_opened;
        editor.just_opened = false;
        let mut chosen = None;
        let mut clear = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("conversation_picker")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 64.0).clamp(300.0, 700.0));
            ui.heading(if lang == Language::Korean {
                format!("{}번 열에 Codex 대화 연결", editor.key + 1)
            } else {
                format!("Assign Codex conversation to column {}", editor.key + 1)
            });
            ui.label(lang.text(
                "터미널에서 쓰는 대화를 고르세요. 위아래 버튼이 함께 연결됩니다.",
                "Choose the conversation from your terminal. Both keys share this conversation.",
            ));
            ui.horizontal(|ui| {
                ui.label(lang.text("버튼 이름", "Button label"));
                ui.add(egui::TextEdit::singleline(&mut editor.label).char_limit(40));
            });
            ui.add_space(7.0);
            egui::ScrollArea::vertical()
                .id_salt("conversation_choices")
                .max_height((ctx.content_rect().height() - 220.0).clamp(120.0, 470.0))
                .show(ui, |ui| {
                    if threads.is_empty() {
                        ui.label(lang.text(
                            "Mac에서 Codex 대화를 시작한 뒤 새로고침하세요.",
                            "Start a Codex conversation on your Mac, then refresh.",
                        ));
                    }
                    for (index, thread) in threads.iter().enumerate() {
                        let taken = self.preferences.conversations.iter().enumerate().any(
                            |(column, slot)| column != editor.key && slot.thread_id == thread.id,
                        );
                        let suffix = if taken {
                            lang.text(" · 다른 열에 연결됨", " · assigned to another column")
                        } else {
                            ""
                        };
                        let title: String = thread.title.chars().take(90).collect();
                        let label = format!("{title}{suffix}\n{}", thread.cwd);
                        let response = ui.add_enabled(
                            !taken,
                            egui::Button::new(egui::RichText::new(label).size(14.0))
                                .wrap()
                                .min_size(egui::Vec2::new(ui.available_width(), 64.0))
                                .selected(index == editor.selected),
                        );
                        if response.clicked() {
                            chosen = Some(thread.id.clone());
                        }
                        response.on_hover_text(format!("{}\n{}", thread.title, thread.id));
                    }
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button(lang.text("연결 비우기", "Clear assignment"))
                    .clicked()
                {
                    clear = true;
                }
                if ui.button(lang.text("닫기 · B", "Close · B")).clicked() {
                    cancel = true;
                }
                ui.label(
                    egui::RichText::new(
                        lang.text("방향키 선택 · A 연결", "D-pad selects · A assigns"),
                    )
                    .small()
                    .color(theme::MUTED),
                );
            });
        });
        if let Some(id) = chosen {
            self.assign_conversation(Some(&id));
        } else if clear {
            self.assign_conversation(None);
        } else if cancel || (!just_opened && response.should_close()) {
            self.editor = None;
            self.displayed_approval = None;
        }
    }
}
