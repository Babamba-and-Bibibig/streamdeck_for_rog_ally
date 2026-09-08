use crate::i18n::{self, Language};
mod deck_settings;
mod paired_deck;
use orangedeck_domain::UiPreferences;
use orangedeck_infra::UiPreferenceStore;
use std::{process::Command, time::Duration};

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use orangedeck_infra::{AuthToken, UiConfig};
use orangedeck_protocol::{
    ApprovalDecisionDto, ClientCommand, CodexThreadDto, CodexThreadStatusDto, SnapshotDto,
};

use crate::{
    controller::{ControlAction, ControllerInput},
    model::UiModel,
    monitor,
    network::NetworkHandle,
    notifications,
    selection::{self, Selection},
    theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Dashboard,
    Agents,
    Projects,
    Codex,
}

impl Page {
    const ALL: [Self; 4] = [Self::Dashboard, Self::Agents, Self::Projects, Self::Codex];

    #[cfg(test)]
    const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "LIVE",
            Self::Agents => "에이전트들",
            Self::Projects => "프로젝트들",
            Self::Codex => "대화",
        }
    }

    const fn translated(self, lang: Language) -> &'static str {
        match self {
            Self::Dashboard => "LIVE",
            Self::Agents => lang.text("에이전트들", "Agents"),
            Self::Projects => lang.text("프로젝트들", "Projects"),
            Self::Codex => lang.text("대화", "Conversations"),
        }
    }

    const fn short(self) -> &'static str {
        match self {
            Self::Dashboard => "01",
            Self::Agents => "02",
            Self::Projects => "03",
            Self::Codex => "04",
        }
    }
}

pub struct OrangeDeckApp {
    preferences: UiPreferences,
    preference_store: Option<UiPreferenceStore>,
    preference_warning: Option<String>,
    editor: Option<deck_settings::EditorState>,
    deck_editing: bool,
    config: UiConfig,
    network: NetworkHandle,
    model: UiModel,
    controller: ControllerInput,
    page: Page,
    focus_index: usize,
    scroll_focus: bool,
    selection: Selection,
    observed_selection: Option<String>,
    demo_mode: bool,
    selected_approval: Option<uuid::Uuid>,
    displayed_approval: Option<uuid::Uuid>,
    deck_modal: Option<paired_deck::DeckModal>,
    watched_deck: Option<Vec<String>>,
    file_request: Option<uuid::Uuid>,
}

impl OrangeDeckApp {
    pub fn new(
        context: &eframe::CreationContext<'_>,
        config: UiConfig,
        token: AuthToken,
        demo_mode: bool,
    ) -> Result<Self, String> {
        theme::apply(&context.egui_ctx);
        let network = NetworkHandle::start(
            config.connector_url.clone(),
            token,
            if demo_mode { "mock" } else { "real" },
            context.egui_ctx.clone(),
        )?;
        Ok(Self {
            preferences: if demo_mode {
                UiPreferences {
                    conversations: std::array::from_fn(|index| {
                        orangedeck_domain::ConversationSlot {
                            thread_id: [
                                "demo-build",
                                "demo-review",
                                "external-tui",
                                "demo-website",
                                "demo-notes",
                            ][index]
                                .to_owned(),
                            label: format!("Codex {}", index + 1),
                        }
                    }),
                    ..UiPreferences::default()
                }
            } else {
                UiPreferences::default()
            },
            preference_store: None,
            preference_warning: None,
            editor: None,
            deck_editing: false,
            config,
            network,
            model: UiModel::default(),
            controller: ControllerInput::new(),
            page: Page::Dashboard,
            focus_index: 0,
            scroll_focus: false,
            selection: Selection::default(),
            observed_selection: None,
            demo_mode,
            selected_approval: None,
            displayed_approval: None,
            deck_modal: None,
            watched_deck: None,
            file_request: None,
        })
    }

    pub fn open_agents(&mut self) {
        self.page = Page::Agents;
        self.focus_index = 0;
        self.displayed_approval = None;
    }

    fn send(&mut self, command: ClientCommand) {
        if !self.model.connected {
            self.model.command_message =
                Some("Connector is disconnected; command was not sent".to_owned());
            return;
        }
        if let Err(error) = self.network.send(command) {
            self.model.command_message = Some(error);
        }
    }

    fn refresh_current(&mut self) {
        self.send(ClientCommand::CodexRefreshThreads);
    }

    fn handle_action(&mut self, action: ControlAction) {
        if self.editor.is_some() {
            self.handle_editor_input(action);
            return;
        }
        if self.deck_modal.is_some() {
            self.handle_modal_input(action);
            return;
        }
        if self.page == Page::Agents && self.deck_editing && action == ControlAction::Back {
            self.deck_editing = false;
            return;
        }
        match action {
            ControlAction::PreviousPage => self.change_page(-1),
            ControlAction::NextPage => self.change_page(1),
            ControlAction::Back => {
                self.page = Page::Dashboard;
                self.focus_index = 0;
                self.displayed_approval = None;
            }
            ControlAction::Context => self.refresh_current(),
            ControlAction::Detail => self.open_agents(),
            ControlAction::PreviousThread | ControlAction::NextThread => {
                let delta = if action == ControlAction::PreviousThread {
                    -1
                } else {
                    1
                };
                if matches!(self.page, Page::Dashboard | Page::Agents | Page::Projects) {
                    self.change_project(delta);
                } else {
                    self.change_thread(delta);
                }
            }
            ControlAction::NavigateLeft => self.navigate(-1, 0),
            ControlAction::NavigateRight => self.navigate(1, 0),
            ControlAction::NavigateUp => self.navigate(0, -1),
            ControlAction::NavigateDown => self.navigate(0, 1),
            ControlAction::Activate => self.activate_focus(),
        }
    }

    fn change_page(&mut self, delta: isize) {
        let current = Page::ALL
            .iter()
            .position(|page| *page == self.page)
            .unwrap_or(0);
        let len = Page::ALL.len();
        let next = if delta.is_negative() {
            current.checked_sub(delta.unsigned_abs()).unwrap_or(len - 1)
        } else {
            (current + delta.unsigned_abs()) % len
        };
        self.select_page(Page::ALL[next]);
    }

    fn select_page(&mut self, page: Page) {
        self.page = page;
        self.displayed_approval = None;
        self.focus_index = 0;
        self.scroll_focus = true;
        if page == Page::Codex
            && let Some(snapshot) = &self.model.snapshot
        {
            self.focus_index = self
                .selection
                .threads(snapshot)
                .iter()
                .position(|thread| self.selection.thread.as_deref() == Some(&thread.id))
                .unwrap_or(0);
        }
    }

    fn select_project(&mut self, path: &str) {
        if let Some(snapshot) = &self.model.snapshot {
            self.selection.select_project(
                path,
                snapshot,
                self.model.connected,
                chrono::Utc::now().timestamp(),
            );
        }
        self.observed_selection = None;
        self.displayed_approval = None;
        self.selected_approval = None;
        self.focus_index = 0;
        self.scroll_focus = true;
    }

    fn change_project(&mut self, delta: isize) {
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        let projects = selection::projects(
            snapshot,
            self.model.connected,
            chrono::Utc::now().timestamp(),
        );
        if projects.is_empty() {
            return;
        }
        let index = projects
            .iter()
            .position(|project| self.selection.project.as_deref() == Some(&project.path))
            .unwrap_or(0);
        let next = if delta < 0 {
            (index + projects.len() - 1) % projects.len()
        } else {
            (index + 1) % projects.len()
        };
        self.select_project(&projects[next].path);
    }

    fn change_thread(&mut self, delta: isize) {
        if let Some(snapshot) = &self.model.snapshot {
            self.selection.change_thread(delta, snapshot);
            self.focus_index = self
                .selection
                .threads(snapshot)
                .iter()
                .position(|thread| self.selection.thread.as_deref() == Some(&thread.id))
                .unwrap_or(0);
        }
        self.displayed_approval = None;
        self.scroll_focus = true;
    }

    fn navigate(&mut self, horizontal: isize, vertical: isize) {
        if self.page == Page::Agents {
            let column = (self.focus_index % 5)
                .saturating_add_signed(horizontal)
                .min(4);
            let row = (self.focus_index / 5)
                .saturating_add_signed(vertical)
                .min(1);
            self.focus_index = row * 5 + column;
            return;
        }
        let max = self.focus_count();
        if max == 0 {
            return;
        }
        let delta = if horizontal != 0 {
            horizontal
        } else {
            vertical
        };
        self.focus_index = self.focus_index.saturating_add_signed(delta).min(max - 1);
        self.scroll_focus = true;
    }

    fn focus_count(&self) -> usize {
        match self.page {
            Page::Dashboard => 1,
            Page::Agents => 10,
            Page::Projects => self.model.snapshot.as_ref().map_or(0, |snapshot| {
                selection::projects(
                    snapshot,
                    self.model.connected,
                    chrono::Utc::now().timestamp(),
                )
                .len()
            }),
            Page::Codex => self
                .model
                .snapshot
                .as_ref()
                .map_or(0, |snapshot| self.selection.threads(snapshot).len()),
        }
    }

    fn activate_focus(&mut self) {
        if self.page == Page::Agents {
            self.activate_pair(self.focus_index);
            return;
        }
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        match self.page {
            Page::Dashboard => self.open_agents(),
            // Approval keys are handled above, with the displayed-request guard.
            Page::Agents => {}
            Page::Projects => {
                let projects = selection::projects(
                    snapshot,
                    self.model.connected,
                    chrono::Utc::now().timestamp(),
                );
                if let Some(project) = projects.get(self.focus_index) {
                    self.select_project(&project.path);
                }
            }
            Page::Codex => {
                if let Some(thread) = self.selection.threads(snapshot).get(self.focus_index) {
                    self.selection.thread = Some(thread.id.clone());
                    self.selection.follow_latest = false;
                    self.displayed_approval = None;
                }
            }
        }
    }

    fn process_network(&mut self) {
        for event in self.network.drain() {
            let close_modal = self.process_deck_network(&event);
            self.model.apply_network(event);
            if close_modal {
                self.close_deck_modal();
            }
        }
    }

    fn update_monitor_selection(&mut self) {
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        self.selection.reconcile(
            snapshot,
            self.model.connected,
            chrono::Utc::now().timestamp(),
        );
        if !self.model.connected {
            self.observed_selection = None;
            return;
        }
        if snapshot
            .codex
            .supported_features
            .iter()
            .any(|feature| feature == "read_only_monitor")
            && let Some(thread) = self.selection.selected(snapshot)
        {
            let key = format!("{}:{}", thread.id, selection::turn_id(thread).unwrap_or(""));
            if self.observed_selection.as_deref() != Some(&key) {
                let id = thread.id.clone();
                self.observed_selection = Some(key);
                self.send(ClientCommand::CodexReadThread { thread_id: id });
            }
        }
    }

    fn current_unread(&self) -> bool {
        self.model
            .snapshot
            .as_ref()
            .and_then(|snapshot| self.selection.selected(snapshot))
            .is_some_and(|thread| self.model.alerts.current_unread(thread))
    }

    fn attention_color(&self) -> Option<Color32> {
        let snapshot = self.model.snapshot.as_ref()?;
        let thread = self.selection.selected(snapshot)?;
        if selection::status(
            thread,
            snapshot,
            self.model.connected,
            chrono::Utc::now().timestamp(),
        ) == CodexThreadStatusDto::WaitingApproval
        {
            Some(theme::YELLOW)
        } else if self.current_unread() {
            self.model
                .alerts
                .entries
                .iter()
                .find(|alert| !alert.read && crate::alerts::is_current(alert, thread))
                .map(|alert| notifications::level_color(alert.notification.level))
        } else {
            None
        }
    }

    fn render_header(&mut self, root: &mut egui::Ui) {
        let lang = self.preferences.language;
        let ctx = root.ctx().clone();
        egui::Panel::top("header")
            .exact_size(56.0)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(10.0))
            .show(root, |ui| {
                ui.spacing_mut().interact_size = Vec2::new(44.0, 32.0);
                ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("ORANGE")
                            .size(19.0)
                            .strong()
                            .color(theme::ORANGE),
                    );
                    ui.label(RichText::new("DECK").size(19.0).strong());
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(self.page.translated(lang))
                            .size(13.0)
                            .color(theme::MUTED),
                    )
                    .on_hover_text(self.controller.detected().join(" · "));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        for (language, label, width) in [
                            (Language::English, "EN", 44.0),
                            (Language::Korean, "한국어", 62.0),
                        ] {
                            let selected = lang == language;
                            if ui
                                .add_sized(
                                    [width, 32.0],
                                    egui::Button::new(
                                        RichText::new(label)
                                            .size(13.0)
                                            .strong()
                                            .color(if selected { theme::BG } else { theme::MUTED }),
                                    )
                                    .fill(if selected {
                                        theme::ORANGE
                                    } else {
                                        theme::PANEL_RAISED
                                    }),
                                )
                                .on_hover_text(language.text("한국어로 보기", "Switch to English"))
                                .clicked()
                            {
                                self.change_language(&ctx, language);
                            }
                        }
                        ui.add_space(8.0);
                        let (label, color) = if self.model.connected {
                            (lang.text("연결됨", "CONNECTED"), theme::GREEN)
                        } else if self.model.connecting {
                            (lang.text("연결 중", "CONNECTING"), theme::YELLOW)
                        } else {
                            (lang.text("연결 끊김", "OFFLINE"), theme::RED)
                        };
                        ui.label(RichText::new(format!("● {label}")).size(12.0).color(color));
                        ui.label(
                            RichText::new(if self.demo_mode {
                                "LOCAL MOCK"
                            } else {
                                "TAILSCALE"
                            })
                            .size(11.0)
                            .color(theme::CYAN),
                        );
                        if ctx.content_rect().width() > 1_100.0 {
                            ui.label(
                                RichText::new(&self.config.host_label)
                                    .size(12.0)
                                    .color(theme::MUTED),
                            );
                        }
                    });
                });
            });
    }

    fn attention_pulse(ctx: &egui::Context) -> f32 {
        let bright = ctx.input(|input| (input.time * std::f64::consts::TAU / 1.8).sin() > 0.0);
        ctx.animate_bool_with_time(egui::Id::new("notification_glow"), bright, 0.65)
    }

    fn render_attention(&self, ctx: &egui::Context) {
        if let Some(color) = self.attention_color() {
            let pulse = Self::attention_pulse(ctx);
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("attention_border"),
            ))
            .rect_stroke(
                ctx.content_rect().shrink(3.0),
                10,
                Stroke::new(5.0 + 3.0 * pulse, color.gamma_multiply(0.7 + 0.3 * pulse)),
                egui::StrokeKind::Inside,
            );
        }
    }

    fn render_nav(&mut self, root: &mut egui::Ui) {
        let lang = self.preferences.language;
        egui::Panel::left("navigation")
            .exact_size(96.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(8.0))
            .show(root, |ui| {
                ui.add_space(4.0);
                let height = ((ui.available_height() - 40.0) / 4.0).clamp(48.0, 90.0);
                for page in Page::ALL {
                    let selected = self.page == page;
                    let attention = if page == Page::Agents {
                        let has_attention = self.model.snapshot.as_ref().is_some_and(|snapshot| {
                            self.pair_views(snapshot).iter().any(|pair| pair.attention)
                        });
                        has_attention.then_some(theme::YELLOW)
                    } else {
                        None
                    };
                    let color = attention.unwrap_or(if selected {
                        theme::ORANGE
                    } else {
                        theme::MUTED
                    });
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(78.0, height), egui::Sense::click());
                    let p = ui.painter();
                    p.rect_filled(
                        rect,
                        12,
                        if selected || attention.is_some() {
                            theme::tint(theme::PANEL, color, 0.12)
                        } else if response.hovered() {
                            theme::PANEL_RAISED
                        } else {
                            theme::BG
                        },
                    );
                    if selected || attention.is_some() {
                        p.rect_stroke(rect, 12, Stroke::new(1.2, color), egui::StrokeKind::Inside);
                    }
                    let icon = match page {
                        Page::Dashboard => orangedeck_domain::Shortcut::Live,
                        Page::Agents => orangedeck_domain::Shortcut::OpenTerminal,
                        Page::Projects => orangedeck_domain::Shortcut::Projects,
                        Page::Codex => orangedeck_domain::Shortcut::Conversations,
                    };
                    let label = match page {
                        Page::Codex => lang.text("대화", "Chats"),
                        Page::Agents => lang.text("에이전트들", "Agents"),
                        _ => page.translated(lang),
                    };
                    if height >= 62.0 {
                        crate::icons::draw(
                            p,
                            rect.center() - Vec2::new(0.0, 10.0),
                            21.0,
                            color,
                            icon,
                        );
                    }
                    p.text(
                        egui::pos2(rect.center().x, rect.bottom() - 12.0),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        egui::FontId::proportional(12.0),
                        color,
                    );
                    p.text(
                        rect.right_top() + Vec2::new(-7.0, 7.0),
                        egui::Align2::RIGHT_TOP,
                        page.short(),
                        egui::FontId::monospace(9.0),
                        theme::MUTED,
                    );
                    if attention.is_some() {
                        p.circle_filled(rect.min + Vec2::splat(9.0), 3.0, color);
                    }
                    if response.on_hover_text(page.translated(lang)).clicked() {
                        self.select_page(page);
                    }
                }
            });
    }

    fn current_approval_id(&self) -> Option<uuid::Uuid> {
        if self.editor.is_some() {
            return None;
        }
        let pending = self.pending_for_modal();
        self.selected_approval
            .filter(|id| pending.iter().any(|approval| approval.id == *id))
            .or_else(|| pending.first().map(|approval| approval.id))
    }

    fn submit_approval(&mut self, id: uuid::Uuid, decision: ApprovalDecisionDto) {
        if self.model.begin_approval(id) {
            if let Some(modal) = &mut self.deck_modal {
                modal.submitted = Some(id);
            }
            self.model.alerts.mark_read(id);
            if let Err(error) = self.network.send(ClientCommand::CodexApprovalResponse {
                approval_id: id,
                decision,
            }) {
                if let Some(modal) = &mut self.deck_modal {
                    modal.submitted = None;
                }
                self.model.approvals_in_flight.remove(&id);
                self.model.approval_error = Some((id, error.clone()));
                self.model.command_message = Some(error);
            }
        }
    }

    fn render_project_picker(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let lang = self.preferences.language;
        let projects = selection::projects(
            snapshot,
            self.model.connected,
            chrono::Utc::now().timestamp(),
        );
        let mut chosen = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new(lang.text("프로젝트", "Project")).size(12.0).color(theme::MUTED));
            egui::ComboBox::from_id_salt("active_project").width((ui.available_width() - 180.0).max(150.0))
                .selected_text(self.selection.project.as_deref().unwrap_or(lang.text("프로젝트 선택", "Choose a project")))
                .show_ui(ui, |ui| {
                    for project in &projects {
                        if ui.selectable_label(self.selection.project.as_deref() == Some(&project.path), &project.path).clicked() { chosen = Some(project.path.clone()); }
                    }
                });
            ui.label(RichText::new(if lang == Language::English { format!("{} projects · {} active", projects.len(), projects.iter().filter(|project| project.active > 0).count()) } else { format!("전체 {} · 활성 {}", projects.len(), projects.iter().filter(|project| project.active > 0).count()) }).size(12.0).color(theme::CYAN))
                .on_hover_text(lang.text("Mac Codex에 기록된 프로젝트 · 활성은 최근 2분 이내 작업 활동 또는 현재 승인 대기 기준입니다.", "Projects found in Codex history. Active means work within two minutes or a pending approval."));
        });
        if let Some(path) = chosen {
            self.select_project(&path);
        }
        ui.add_space(4.0);
    }

    fn render_dashboard(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let mut scoped = snapshot.clone();
        scoped.codex.threads = self
            .selection
            .threads(snapshot)
            .into_iter()
            .cloned()
            .collect();
        let selected = scoped
            .codex
            .threads
            .iter()
            .position(|thread| self.selection.thread.as_deref() == Some(&thread.id))
            .unwrap_or(0);
        let height = ui.available_height();
        egui::ScrollArea::vertical()
            .id_salt("monitor")
            .show(ui, |ui| {
                let action = monitor::render(
                    ui,
                    &scoped,
                    selected,
                    self.model.connected,
                    self.selection.follow_latest,
                    height,
                );
                if action.project_delta != 0 {
                    self.change_project(action.project_delta);
                }
                if action.follow {
                    self.selection.follow_latest = true;
                }
                if action.detail {
                    self.open_agents();
                }
            });
    }

    fn render_projects(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let lang = self.preferences.language;
        let projects = selection::projects(
            snapshot,
            self.model.connected,
            chrono::Utc::now().timestamp(),
        );
        section_title(
            ui,
            lang.text("작업 공간 · 프로젝트들", "WORKSPACE · PROJECTS"),
            theme::ORANGE,
        );
        ui.label(
            RichText::new(lang.text(
                "판단 대기 · 작업 중인 프로젝트 우선, 최근 활동 순",
                "Approvals and active projects first, then recent activity",
            ))
            .size(13.0)
            .color(theme::MUTED),
        );
        for (index, project) in projects.iter().enumerate() {
            let selected = self.selection.project.as_deref() == Some(&project.path);
            let color = if project.waiting > 0 {
                theme::YELLOW
            } else if project.active > 0 {
                theme::CYAN
            } else {
                theme::MUTED
            };
            let frame = if selected || index == self.focus_index {
                theme::accent_panel(color)
            } else {
                theme::panel()
            };
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&project.name).size(23.0).strong());
                        if selected {
                            ui.label(
                                RichText::new(lang.text("선택됨", "Selected"))
                                    .size(12.0)
                                    .color(theme::ORANGE),
                            );
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(if lang == Language::English {
                                    format!(
                                        "{} chats · {} active · {} waiting",
                                        project.conversations, project.active, project.waiting
                                    )
                                } else {
                                    format!(
                                        "대화 {} · 활성 {} · 판단 대기 {}",
                                        project.conversations, project.active, project.waiting
                                    )
                                })
                                .size(13.0)
                                .color(color),
                            );
                        });
                    });
                    ui.add(
                        egui::Label::new(
                            RichText::new(&project.path).size(13.0).color(theme::MUTED),
                        )
                        .truncate(),
                    );
                    ui.label(
                        RichText::new(if lang == Language::English {
                            format!("Last active {}", activity_time(lang, project.updated_at))
                        } else {
                            format!("최근 활동 {}", activity_time(lang, project.updated_at))
                        })
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                })
                .response
                .interact(egui::Sense::click());
            if self.scroll_focus && index == self.focus_index {
                response.scroll_to_me(Some(Align::Center));
            }
            if response.clicked() {
                self.select_project(&project.path);
                self.focus_index = index;
            }
        }
        if projects.is_empty() {
            ui.label(lang.text(
                "Mac Codex의 프로젝트 목록을 기다리고 있습니다.",
                "Waiting for projects from Codex on your host.",
            ));
        }
        self.scroll_focus = false;
    }

    fn render_codex(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let lang = self.preferences.language;
        ui.horizontal(|ui| {
            section_title(ui, lang.text("현재 프로젝트 · 대화", "PROJECT · CONVERSATIONS"), theme::ORANGE);
            if small_action(
                ui,
                if self.selection.follow_latest { lang.text("자동 ON", "AUTO ON") } else { lang.text("최근 자동", "Follow latest") },
                self.selection.follow_latest,
            )
            .on_hover_text(lang.text("현재 프로젝트의 최신 대화를 자동으로 따라갑니다. 이미 최신 대화면 선택을 유지합니다. 다른 대화를 고르면 고정 모드로 바뀝니다.", "Follow the newest conversation in this project. Selecting another conversation pins it."))
            .clicked()
            {
                self.selection.follow_latest = true;
            }
        });
        ui.label(
            RichText::new(lang.text(
                "최신 질문 · 최근 활동 순 · 현재 LIVE와 같은 대화가 선택되어 있습니다.",
                "Recent questions, newest first. Selection is shared with LIVE.",
            ))
            .size(13.0)
            .color(theme::MUTED),
        );
        let threads = self.selection.threads(snapshot);
        for (index, thread) in threads.iter().enumerate() {
            let response = thread_card(
                ui,
                thread,
                self.selection.thread.as_deref() == Some(&thread.id),
                index == self.focus_index,
                selection::status(
                    thread,
                    snapshot,
                    self.model.connected,
                    chrono::Utc::now().timestamp(),
                ),
            );
            if self.scroll_focus && index == self.focus_index {
                response.scroll_to_me(Some(Align::Center));
            }
            if response.clicked() {
                self.selection.thread = Some(thread.id.clone());
                self.selection.follow_latest = false;
                self.focus_index = index;
                self.displayed_approval = None;
            }
        }
        if threads.is_empty() {
            ui.label(lang.text(
                "이 프로젝트의 대화가 없습니다.",
                "No conversations in this project yet.",
            ));
        }
        self.scroll_focus = false;
    }

    fn render_status_line(&self, root: &mut egui::Ui) {
        let lang = self.preferences.language;
        let preferences_warning = self.preference_warning.as_ref().map(|_| lang.text(
            "화면 설정을 저장하지 못했습니다. 기존 설정 파일은 보존하며 이번 창에서만 변경합니다.",
            "UI preferences could not be saved. The original file is preserved; changes apply to this window only.",
        ));
        let Some(message) = self
            .model
            .protocol_error
            .as_deref()
            .or(self.model.connection_message.as_deref())
            .or(preferences_warning)
            .or(self.model.command_message.as_deref())
        else {
            return;
        };
        egui::Panel::bottom("status_line")
            .exact_size(30.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL_RAISED)
                    .inner_margin(6.0),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(message).size(12.0).color(theme::MUTED));
                    if let Some(retry) = self.model.retry_ms {
                        ui.label(
                            RichText::new(format!("retry {retry}ms"))
                                .size(12.0)
                                .color(theme::YELLOW),
                        );
                    }
                });
            });
    }
}

impl eframe::App for OrangeDeckApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        i18n::set_language(root.ctx(), self.preferences.language);
        self.process_network();
        self.update_monitor_selection();
        self.update_deck_watches();
        let ctx = root.ctx().clone();
        if self.editor.is_none() {
            self.announce_notifications(&ctx);
        }
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        let approval_pending = self.current_approval_id().is_some() && self.editor.is_none();
        for action in self.controller.poll(&ctx, focused, approval_pending) {
            self.handle_action(action);
        }
        if self.deck_modal.is_some() || self.editor.is_some() {
            root.disable();
        }
        self.render_header(root);
        self.render_nav(root);
        self.render_status_line(root);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(12.0))
            .show(root, |ui| {
                let Some(snapshot) = self.model.snapshot.clone() else {
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(self.preferences.language.text(
                                    "Mac 통신 모듈에 연결 중",
                                    "CONNECTING TO ORANGEDECK CONNECTOR",
                                ))
                                .strong()
                                .color(theme::MUTED),
                            );
                        });
                    });
                    return;
                };
                if self.page != Page::Agents {
                    self.render_project_picker(ui, &snapshot);
                }
                if self.deck_modal.is_none() {
                    self.displayed_approval = None;
                }
                match self.page {
                    Page::Dashboard => self.render_dashboard(ui, &snapshot),
                    Page::Agents => self.render_agents(ui, &snapshot),
                    Page::Projects => {
                        egui::ScrollArea::vertical()
                            .id_salt("projects_content")
                            .show(ui, |ui| self.render_projects(ui, &snapshot));
                    }
                    Page::Codex => {
                        egui::ScrollArea::vertical()
                            .id_salt("page_content")
                            .show(ui, |ui| self.render_codex(ui, &snapshot));
                    }
                }
            });
        self.render_deck_modal(&ctx);
        self.render_attention(&ctx);
        self.render_key_editor(&ctx);
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn section_title(ui: &mut egui::Ui, title: &str, color: Color32) {
    ui.label(RichText::new(title).size(13.0).strong().color(color));
}

fn small_action(ui: &mut egui::Ui, label: &str, focused: bool) -> egui::Response {
    ui.add_sized(
        [96.0, 36.0],
        egui::Button::new(RichText::new(label).size(12.0).strong()).fill(if focused {
            theme::ORANGE
        } else {
            theme::PANEL_RAISED
        }),
    )
}

fn thread_card(
    ui: &mut egui::Ui,
    thread: &CodexThreadDto,
    selected: bool,
    focused: bool,
    status: CodexThreadStatusDto,
) -> egui::Response {
    let lang = i18n::language(ui.ctx());
    let frame = if selected || focused {
        theme::accent_panel(codex_status_color(status))
    } else {
        theme::panel()
    };
    let response = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let text_width = (ui.available_width() - 200.0).max(100.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(text_width, 0.0),
                    Layout::top_down(Align::Min),
                    |ui| {
                        let mut question = egui::text::LayoutJob::simple(
                            selection::latest_prompt(thread)
                                .unwrap_or(lang.text(
                                    "최신 질문을 아직 읽지 못했습니다",
                                    "Latest question not available yet",
                                ))
                                .to_owned(),
                            egui::FontId::proportional(20.0),
                            theme::TEXT,
                            text_width,
                        );
                        question.wrap.max_rows = 2;
                        ui.label(question);
                        ui.add(
                            egui::Label::new(
                                RichText::new(if lang == Language::English {
                                    format!("Conversation · {}", thread.title)
                                } else {
                                    format!("대화 제목 · {}", thread.title)
                                })
                                .size(12.0)
                                .monospace()
                                .color(theme::MUTED),
                            )
                            .truncate(),
                        );
                        ui.label(
                            RichText::new(if lang == Language::English {
                                format!(
                                    "Last active {}",
                                    activity_time(lang, selection::updated_at(thread))
                                )
                            } else {
                                format!(
                                    "최근 활동 {}",
                                    activity_time(lang, selection::updated_at(thread))
                                )
                            })
                            .size(12.0)
                            .color(theme::MUTED),
                        );
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if selected {
                        ui.label(
                            RichText::new(lang.text("현재 LIVE", "ON LIVE"))
                                .size(14.0)
                                .strong()
                                .color(theme::ORANGE),
                        );
                    }
                    ui.label(
                        RichText::new(codex_status_label(lang, status))
                            .size(14.0)
                            .strong()
                            .color(codex_status_color(status)),
                    );
                });
            });
        })
        .response;
    response.interact(egui::Sense::click())
}

const fn codex_status_label(lang: Language, status: CodexThreadStatusDto) -> &'static str {
    match status {
        CodexThreadStatusDto::NotLoaded | CodexThreadStatusDto::Unknown => {
            lang.text("활동 미확인", "Unknown")
        }
        CodexThreadStatusDto::Idle => lang.text("대기", "Idle"),
        CodexThreadStatusDto::Working => lang.text("작업 중", "Working"),
        CodexThreadStatusDto::WaitingApproval => lang.text("판단 대기", "Needs attention"),
        CodexThreadStatusDto::Completed => lang.text("완료 기록", "Completed"),
        CodexThreadStatusDto::Error => lang.text("오류", "Error"),
    }
}

const fn codex_status_color(status: CodexThreadStatusDto) -> Color32 {
    match status {
        CodexThreadStatusDto::Working => theme::CYAN,
        CodexThreadStatusDto::WaitingApproval => theme::PINK,
        CodexThreadStatusDto::Completed => theme::GREEN,
        CodexThreadStatusDto::Error => theme::RED,
        CodexThreadStatusDto::Idle => theme::YELLOW,
        CodexThreadStatusDto::NotLoaded | CodexThreadStatusDto::Unknown => theme::MUTED,
    }
}

fn desktop_notification(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args([
            "--app-name",
            "OrangeDeck",
            "--urgency",
            "critical",
            "--",
            title,
            body,
        ])
        .spawn();
}

fn activity_time(lang: Language, timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0).map_or_else(
        || lang.text("시각 미제공", "Time unavailable").to_owned(),
        |at| {
            at.with_timezone(&chrono::Local)
                .format("%m-%d %H:%M")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests;
