use crate::i18n::{self, Language};
mod deck_settings;
use orangedeck_domain::UiPreferences;
use orangedeck_infra::UiPreferenceStore;
use std::{
    collections::BTreeSet,
    process::Command,
    time::{Duration, Instant},
};

use eframe::egui::{self, Align, Color32, Layout, RichText, Stroke, Vec2};
use orangedeck_infra::{AuthToken, UiConfig};
use orangedeck_protocol::{
    ApprovalDecisionDto, ApprovalDto, ClientCommand, CodexThreadDto, CodexThreadStatusDto,
    SnapshotDto,
};

use crate::{
    controller::{ControlAction, ControllerInput},
    model::UiModel,
    monitor,
    network::NetworkHandle,
    notifications,
    selection::{self, Selection},
    shortcuts, theme,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Dashboard,
    Shortcuts,
    Projects,
    Codex,
    Notifications,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::Dashboard,
        Self::Shortcuts,
        Self::Projects,
        Self::Codex,
        Self::Notifications,
    ];

    #[cfg(test)]
    const fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "LIVE",
            Self::Shortcuts => "단축키",
            Self::Projects => "프로젝트들",
            Self::Codex => "대화",
            Self::Notifications => "알림",
        }
    }

    const fn translated(self, lang: Language) -> &'static str {
        match self {
            Self::Dashboard => "LIVE",
            Self::Shortcuts => lang.text("단축키", "Shortcuts"),
            Self::Projects => lang.text("프로젝트들", "Projects"),
            Self::Codex => lang.text("대화", "Conversations"),
            Self::Notifications => lang.text("알림", "Notifications"),
        }
    }

    const fn short(self) -> &'static str {
        match self {
            Self::Dashboard => "01",
            Self::Shortcuts => "02",
            Self::Projects => "03",
            Self::Codex => "04",
            Self::Notifications => "05",
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
    shortcut_seen: BTreeSet<uuid::Uuid>,
    toast: Option<(crate::alerts::Alert, Instant)>,
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
            preferences: UiPreferences::default(),
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
            shortcut_seen: BTreeSet::new(),
            toast: None,
        })
    }

    pub fn open_notifications(&mut self) {
        self.page = Page::Notifications;
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
        if self.toast_is_current() {
            match action {
                ControlAction::Activate | ControlAction::Detail => self.dismiss_notification(true),
                ControlAction::Back => self.dismiss_notification(false),
                _ => {}
            }
            return;
        }
        if self.page == Page::Shortcuts && self.deck_editing {
            if action == ControlAction::Back {
                self.deck_editing = false;
                self.displayed_approval = None;
                return;
            }
            if action == ControlAction::Activate && self.focus_index < 2 {
                return;
            }
        }
        if let Some(approval_id) = self.current_approval_id() {
            let decision = match action {
                ControlAction::Activate if self.page == Page::Shortcuts => match self.focus_index {
                    0 => Some(ApprovalDecisionDto::Approve),
                    1 => Some(ApprovalDecisionDto::Reject),
                    _ => None,
                },
                ControlAction::Activate => Some(ApprovalDecisionDto::Approve),
                ControlAction::Back => Some(ApprovalDecisionDto::Reject),
                _ => None,
            };
            if let Some(decision) = decision {
                // Never apply an A/B press to an approval that has not been rendered yet.
                if self.displayed_approval == Some(approval_id) {
                    self.submit_approval(approval_id, decision);
                }
                return;
            }
            if matches!(
                action,
                ControlAction::PreviousThread | ControlAction::NextThread
            ) {
                self.cycle_approval(action == ControlAction::PreviousThread);
                return;
            }
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
            ControlAction::Detail => self.open_notifications(),
            ControlAction::PreviousThread | ControlAction::NextThread => {
                let delta = if action == ControlAction::PreviousThread {
                    -1
                } else {
                    1
                };
                if matches!(
                    self.page,
                    Page::Dashboard | Page::Shortcuts | Page::Projects
                ) {
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
        self.toast = None;
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
        self.toast = None;
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
        if self.page == Page::Shortcuts {
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
            Page::Dashboard | Page::Notifications => 1,
            Page::Shortcuts => 10,
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
        if self.page == Page::Shortcuts && self.focus_index >= 2 {
            self.activate_custom_key(self.focus_index);
            return;
        }
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        match self.page {
            Page::Dashboard => self.open_notifications(),
            // Approval keys are handled above, with the displayed-request guard.
            Page::Shortcuts => {}
            Page::Notifications => {
                if let Some(thread) = self.selection.selected(snapshot) {
                    self.model.alerts.mark_current_read(thread);
                }
            }
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
            self.model.apply_network(event);
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
                let height = ((ui.available_height() - 40.0) / 5.0).clamp(48.0, 78.0);
                for page in Page::ALL {
                    let selected = self.page == page;
                    let attention = match page {
                        Page::Notifications => self.attention_color(),
                        Page::Shortcuts
                            if !self.pending_for_selection().is_empty()
                                && self.model.approval_ready() =>
                        {
                            Some(theme::YELLOW)
                        }
                        _ => None,
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
                        Page::Shortcuts => orangedeck_domain::Shortcut::OpenTerminal,
                        Page::Projects => orangedeck_domain::Shortcut::Projects,
                        Page::Codex => orangedeck_domain::Shortcut::Conversations,
                        Page::Notifications => orangedeck_domain::Shortcut::Notifications,
                    };
                    let label = match page {
                        Page::Codex => lang.text("대화", "Chats"),
                        Page::Notifications => lang.text("알림", "Alerts"),
                        Page::Shortcuts => lang.text("단축키", "Deck"),
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

    fn pending_for_selection(&self) -> Vec<&ApprovalDto> {
        let Some(snapshot) = &self.model.snapshot else {
            return Vec::new();
        };
        let Some(thread) = self.selection.selected(snapshot) else {
            return Vec::new();
        };
        snapshot
            .codex
            .pending_approvals
            .iter()
            .filter(|approval| {
                approval.thread_id.as_deref() == Some(&thread.id)
                    && approval
                        .turn_id
                        .as_deref()
                        .is_none_or(|id| selection::turn_id(thread).is_none_or(|turn| id == turn))
            })
            .collect()
    }

    fn current_approval_id(&self) -> Option<uuid::Uuid> {
        if !matches!(self.page, Page::Notifications | Page::Shortcuts)
            || self.toast.is_some()
            || self.editor.is_some()
        {
            return None;
        }
        let pending = self.pending_for_selection();
        self.selected_approval
            .filter(|id| pending.iter().any(|approval| approval.id == *id))
            .or_else(|| pending.first().map(|approval| approval.id))
    }

    fn route_shortcut_approval(&mut self) {
        if self.editor.is_some() {
            return;
        }
        if !self.model.approval_ready() {
            return;
        }
        if let Some(snapshot) = &self.model.snapshot {
            self.shortcut_seen.retain(|id| {
                snapshot
                    .codex
                    .pending_approvals
                    .iter()
                    .any(|request| request.id == *id)
            });
        }
        let pending: Vec<_> = self
            .pending_for_selection()
            .iter()
            .map(|request| request.id)
            .collect();
        let Some(new_request) = pending
            .iter()
            .find(|id| !self.shortcut_seen.contains(id))
            .copied()
        else {
            return;
        };
        self.shortcut_seen.extend(pending.iter().copied());
        self.toast = None;
        // Do not replace an already visible request when more requests join its queue.
        if self.page != Page::Shortcuts
            || self
                .selected_approval
                .is_none_or(|id| !pending.contains(&id))
        {
            self.select_page(Page::Shortcuts);
            self.selected_approval = Some(new_request);
        }
    }

    fn cycle_approval(&mut self, previous: bool) {
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        let Some(thread) = self.selection.selected(snapshot) else {
            return;
        };
        let pending: Vec<_> = snapshot
            .codex
            .pending_approvals
            .iter()
            .filter(|approval| {
                approval.thread_id.as_deref() == Some(&thread.id)
                    && approval
                        .turn_id
                        .as_deref()
                        .is_none_or(|id| selection::turn_id(thread).is_none_or(|turn| id == turn))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        let index = pending
            .iter()
            .position(|approval| Some(approval.id) == self.current_approval_id())
            .unwrap_or(0);
        let next = if previous {
            (index + pending.len() - 1) % pending.len()
        } else {
            (index + 1) % pending.len()
        };
        self.selected_approval = Some(pending[next].id);
        self.displayed_approval = None;
    }

    fn submit_approval(&mut self, id: uuid::Uuid, decision: ApprovalDecisionDto) {
        if self.model.begin_approval(id) {
            self.model.alerts.mark_read(id);
            if let Err(error) = self.network.send(ClientCommand::CodexApprovalResponse {
                approval_id: id,
                decision,
            }) {
                self.model.approvals_in_flight.remove(&id);
                self.model.approval_error = Some((id, error.clone()));
                self.model.command_message = Some(error);
            }
        }
    }

    fn open_alert_thread(&mut self, id: &str) {
        if let Some(thread) = self
            .model
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.codex.threads.iter().find(|thread| thread.id == id))
        {
            self.selection.project = Some(selection::project_path(&thread.cwd));
            self.selection.thread = Some(thread.id.clone());
            self.selection.follow_latest = false;
            self.observed_selection = None;
        }
        self.open_notifications();
    }

    fn render_approval(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let lang = self.preferences.language;
        let Some(id) = self.current_approval_id() else {
            self.displayed_approval = None;
            return;
        };
        let Some(approval) = snapshot
            .codex
            .pending_approvals
            .iter()
            .find(|approval| approval.id == id)
        else {
            return;
        };
        self.selected_approval = Some(id);
        let pending_count = snapshot
            .codex
            .pending_approvals
            .iter()
            .filter(|request| {
                request.thread_id == approval.thread_id && request.turn_id == approval.turn_id
            })
            .count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if lang == Language::English {
                    format!("{pending_count} approval(s) pending")
                } else {
                    format!("승인 대기 {pending_count}건")
                })
                .color(theme::YELLOW),
            );
            if let Some(thread) = snapshot
                .codex
                .threads
                .iter()
                .find(|thread| Some(&thread.id) == approval.thread_id.as_ref())
            {
                ui.add(
                    egui::Label::new(RichText::new(&thread.cwd).size(12.0).color(theme::CYAN))
                        .truncate(),
                );
            }
        });
        if pending_count > 1 {
            ui.horizontal(|ui| {
                if ui
                    .button(lang.text("‹ 이전 요청 · LT", "‹ Previous · LT"))
                    .clicked()
                {
                    self.cycle_approval(true);
                }
                if ui
                    .button(lang.text("다음 요청 · RT ›", "Next · RT ›"))
                    .clicked()
                {
                    self.cycle_approval(false);
                }
            });
        }
        if let Some(decision) = notifications::approval_panel(
            ui,
            approval,
            self.model.approval_ready(),
            self.model.approvals_in_flight.contains(&id),
            self.page == Page::Notifications,
        ) {
            self.submit_approval(id, decision);
        }
        // The visible request, rather than the queue's first entry, owns the physical buttons.
        self.displayed_approval = Some(id);
        ui.add_space(8.0);
    }

    fn render_notifications(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        self.render_approval(ui, snapshot);
        notifications::render(
            ui,
            snapshot,
            self.selection.selected(snapshot),
            &mut self.model.alerts,
            self.model.connected,
        );
        if let Some(message) = &self.model.command_message {
            ui.label(RichText::new(message).size(12.0).color(theme::YELLOW));
        }
    }

    fn render_shortcuts(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let id = self.current_approval_id();
        let approval = id.and_then(|id| {
            snapshot
                .codex
                .pending_approvals
                .iter()
                .find(|request| request.id == id)
        });
        let pending = self.pending_for_selection();
        let position = pending
            .iter()
            .position(|request| Some(request.id) == id)
            .unwrap_or(0);
        let action: shortcuts::DeckAction = shortcuts::render(
            ui,
            &shortcuts::DeckView {
                slots: std::array::from_fn(|index| self.preferences.action(index + 2)),
                mode: if self.deck_editing {
                    shortcuts::DeckMode::Edit
                } else {
                    shortcuts::DeckMode::Run
                },
                approval,
                position,
                pending: pending.len(),
                ready: self.model.approval_ready(),
                armed: id.is_some() && self.displayed_approval == id,
                sending: id.is_some_and(|id| self.model.approvals_in_flight.contains(&id)),
                focused: self.focus_index,
                pulse: Self::attention_pulse(ui.ctx()),
                message: self
                    .model
                    .approval_error
                    .as_ref()
                    .filter(|(request, _)| Some(*request) == id)
                    .map(|(_, message)| message.as_str()),
            },
        );
        let previously_displayed = self.displayed_approval;
        self.displayed_approval = id;
        if let Some(id) = id {
            self.selected_approval = Some(id);
        }
        if let Some(index) = action.edit {
            self.open_key_editor(index);
        } else if let Some(index) = action.activate {
            self.activate_custom_key(index);
        } else if action.toggle_edit {
            self.deck_editing = !self.deck_editing;
            self.displayed_approval = None;
        } else if action.recommended {
            self.use_recommended_keys();
        } else if action.details {
            self.open_notifications();
        } else if action.cycle != 0 {
            self.cycle_approval(action.cycle < 0);
        } else if let Some(decision) = action.decision
            && let Some(id) = id
            && previously_displayed == Some(id)
            && self.current_approval_id() == Some(id)
        {
            self.submit_approval(id, decision);
        }
    }

    fn announce_notifications(&mut self, ctx: &egui::Context) {
        if !self.toast_is_current() {
            self.toast = None;
        }
        let notices: Vec<_> = self.model.alerts.take_announcements().collect();
        let latest = notices.into_iter().rfind(|alert| {
            self.model
                .snapshot
                .as_ref()
                .and_then(|snapshot| self.selection.selected(snapshot))
                .is_some_and(|thread| crate::alerts::is_current(alert, thread))
        });
        if let Some(alert) = latest {
            if self.config.desktop_notifications {
                desktop_notification(&alert.notification.title, &alert.notification.body);
            }
            if !ctx.input(|input| input.viewport().focused.unwrap_or(true)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Critical,
                ));
            }
            self.displayed_approval = None;
            let approval = self
                .pending_for_selection()
                .iter()
                .any(|request| request.id == alert.id);
            // Approval requests use the square keys directly; a modal must not cover them.
            self.toast = (!approval).then(|| (alert, Instant::now()));
        }
    }

    fn toast_is_current(&self) -> bool {
        self.toast.as_ref().is_some_and(|(alert, _)| {
            self.model
                .snapshot
                .as_ref()
                .and_then(|snapshot| self.selection.selected(snapshot))
                .is_some_and(|thread| crate::alerts::is_current(alert, thread))
                && self
                    .model
                    .alerts
                    .entries
                    .iter()
                    .any(|entry| entry.id == alert.id && !entry.read)
        })
    }

    fn dismiss_notification(&mut self, open: bool) {
        let Some((alert, _)) = self.toast.take() else {
            return;
        };
        if let Some(thread) = self
            .model
            .snapshot
            .as_ref()
            .and_then(|snapshot| self.selection.selected(snapshot))
            && crate::alerts::is_current(&alert, thread)
        {
            self.model.alerts.mark_current_read(thread);
            if open && let Some(id) = &alert.notification.thread_id {
                self.open_alert_thread(id);
            }
        }
        self.displayed_approval = None;
    }

    fn render_toast(&mut self, ctx: &egui::Context) {
        let lang = self.preferences.language;
        if !self.toast_is_current() {
            self.toast = None;
            return;
        }
        let Some((alert, _)) = self.toast.clone() else {
            return;
        };
        let color = notifications::level_color(alert.notification.level);
        let thread = self
            .model
            .snapshot
            .as_ref()
            .and_then(|snapshot| self.selection.selected(snapshot));
        let project = thread.map(|thread| thread.cwd.clone()).unwrap_or_default();
        let question = thread
            .and_then(selection::latest_prompt)
            .unwrap_or(lang.text("현재 질의", "Current question"))
            .to_owned();
        let width = (ctx.content_rect().width() - 88.0).clamp(240.0, 580.0);
        egui::Modal::new(egui::Id::new("notification_toast"))
            .backdrop_color(Color32::from_black_alpha(145))
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(Stroke::new(4.0, color))
                    .inner_margin(22.0)
                    .corner_radius(14),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 12.0;
                egui::ScrollArea::vertical()
                    .id_salt("alert_preview")
                    .max_height((ctx.content_rect().height() - 180.0).max(120.0))
                    .show(ui, |ui| {
                        let width = ui.available_width();
                        ui.label(
                            RichText::new(
                                lang.text("CODEX · 현재 질의 알림", "CODEX · CURRENT TURN"),
                            )
                            .size(14.0)
                            .strong()
                            .color(color),
                        );
                        let mut title = egui::text::LayoutJob::simple(
                            alert.notification.title.clone(),
                            egui::FontId::proportional(34.0),
                            color,
                            width,
                        );
                        title.wrap.max_rows = 2;
                        ui.label(title);
                        ui.add(
                            egui::Label::new(RichText::new(project).size(13.0).color(theme::MUTED))
                                .truncate(),
                        );
                        let mut prompt = egui::text::LayoutJob::simple(
                            question,
                            egui::FontId::proportional(22.0),
                            theme::TEXT,
                            width,
                        );
                        prompt.wrap.max_rows = 2;
                        ui.label(prompt);
                        let mut body = egui::text::LayoutJob::simple(
                            alert.notification.body.clone(),
                            egui::FontId::proportional(18.0),
                            theme::TEXT,
                            width,
                        );
                        body.wrap.max_rows = 2;
                        ui.label(body);
                    });
                ui.label(
                    RichText::new(lang.text(
                        "확인할 때까지 이 알림을 표시합니다",
                        "This alert stays until you acknowledge it",
                    ))
                    .size(13.0)
                    .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    let size = [(width - 12.0) / 2.0, 54.0];
                    if ui
                        .add_sized(
                            size,
                            egui::Button::new(
                                RichText::new(lang.text("내용 보기 · A", "View details · A"))
                                    .size(19.0)
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(color),
                        )
                        .clicked()
                    {
                        self.dismiss_notification(true);
                    }
                    if ui
                        .add_sized(
                            size,
                            egui::Button::new(
                                RichText::new(lang.text("확인했어요 · B", "Mark as read · B"))
                                    .size(19.0),
                            ),
                        )
                        .clicked()
                    {
                        self.dismiss_notification(false);
                    }
                });
            });
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
                    self.open_notifications();
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
        let ctx = root.ctx().clone();
        if self.editor.is_none() {
            self.announce_notifications(&ctx);
        }
        self.route_shortcut_approval();
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        let approval_pending = self.current_approval_id().is_some() && self.editor.is_none();
        for action in self.controller.poll(&ctx, focused, approval_pending) {
            self.handle_action(action);
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
                self.render_project_picker(ui, &snapshot);
                if !matches!(self.page, Page::Notifications | Page::Shortcuts) {
                    self.displayed_approval = None;
                }
                match self.page {
                    Page::Dashboard => self.render_dashboard(ui, &snapshot),
                    Page::Shortcuts => self.render_shortcuts(ui, &snapshot),
                    Page::Notifications => {
                        egui::ScrollArea::vertical()
                            .id_salt("notifications_content")
                            .show(ui, |ui| self.render_notifications(ui, &snapshot));
                    }
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
        self.render_toast(&ctx);
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
mod tests {
    use super::*;

    fn test_app(
        snapshot: &SnapshotDto,
    ) -> (
        OrangeDeckApp,
        tokio::sync::mpsc::UnboundedReceiver<ClientCommand>,
    ) {
        let mut model = UiModel::default();
        model.apply_network(crate::model::NetworkEvent::Connected { latency_ms: 1 });
        model.apply_network(crate::model::NetworkEvent::Server(
            orangedeck_protocol::ServerEnvelope::new(orangedeck_protocol::ServerEvent::Snapshot(
                snapshot.clone(),
            )),
        ));
        let (network, commands) = NetworkHandle::for_test();
        let app = OrangeDeckApp {
            preferences: UiPreferences::default(),
            preference_store: None,
            preference_warning: None,
            editor: None,
            deck_editing: false,
            config: UiConfig::demo(),
            network,
            model,
            controller: ControllerInput::for_test(),
            page: Page::Dashboard,
            focus_index: 0,
            scroll_focus: false,
            selection: Selection::default(),
            observed_selection: None,
            demo_mode: true,
            selected_approval: None,
            displayed_approval: None,
            shortcut_seen: BTreeSet::new(),
            toast: None,
        };
        (app, commands)
    }

    fn announce_test_alert(app: &mut OrangeDeckApp, thread_id: &str) {
        let id = uuid::Uuid::new_v4();
        app.model.alerts.record(
            orangedeck_protocol::NotificationDto {
                id: Some(id),
                turn_id: Some("new".to_owned()),
                thread_id: Some(thread_id.to_owned()),
                created_at: Some(chrono::Utc::now()),
                level: orangedeck_protocol::NotificationLevelDto::Success,
                title: "응답 완료".to_owned(),
                body: "요청한 작업을 완료했습니다".to_owned(),
            },
            id,
            chrono::Utc::now(),
            true,
            false,
        );
        app.config.desktop_notifications = false;
        app.announce_notifications(&egui::Context::default());
    }

    fn shortcut_request(thread: &str, turn: &str) -> ApprovalDto {
        ApprovalDto {
            id: uuid::Uuid::new_v4(),
            thread_id: Some(thread.to_owned()),
            turn_id: Some(turn.to_owned()),
            kind: orangedeck_protocol::ApprovalKindDto::CommandExecution,
            title: "명령 실행 승인".to_owned(),
            summary: "cargo check --offline".to_owned(),
            details: vec![
                "cwd: /Users/mac/project-a".to_owned(),
                "Synthetic request; nothing is executed".to_owned(),
            ],
            requested_at: chrono::Utc::now(),
        }
    }

    fn shortcuts_frame(
        app: &mut OrangeDeckApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> [egui::Pos2; 2] {
        let snapshot = app.model.snapshot.as_ref().unwrap().clone();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(676.0, 380.0),
                )),
                events,
                time: Some(0.5),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| app.render_shortcuts(ui, &snapshot));
            },
        );
        let positions = ["승인", "거절"].map(|label| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == label => {
                        let rect = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(shape.clip_rect.contains_rect(rect));
                        Some(rect.center())
                    }
                    _ => None,
                })
                .unwrap()
        });
        output.drop_without_applying_deltas();
        positions
    }

    fn pointer_events(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn shortcuts_route_current_approvals_once_without_a_covering_modal_and_keep_queue_stable() {
        let mut snapshot = crate::test_support::snapshot();
        let request = shortcut_request("a", "new");
        snapshot.codex.pending_approvals =
            vec![shortcut_request("b", "other"), shortcut_request("a", "old")];
        let (mut app, mut commands) = test_app(&snapshot);
        app.config.desktop_notifications = false;
        app.select_project("/Users/mac/project-a");
        app.route_shortcut_approval();
        assert_eq!(app.page, Page::Dashboard);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        app.model.apply_network(crate::model::NetworkEvent::Server(
            orangedeck_protocol::ServerEnvelope::new(
                orangedeck_protocol::ServerEvent::CodexApprovalRequested(request.clone()),
            ),
        ));
        app.announce_notifications(&ctx);
        app.route_shortcut_approval();
        assert_eq!(app.page, Page::Shortcuts);
        assert!(app.toast.is_none());
        assert_eq!(app.current_approval_id(), Some(request.id));
        app.handle_action(ControlAction::Activate);
        assert!(
            commands.try_recv().is_err(),
            "routing must not consume a pre-existing A press"
        );
        shortcuts_frame(&mut app, &ctx, vec![]);
        let next = shortcut_request("a", "new");
        app.model
            .snapshot
            .as_mut()
            .unwrap()
            .codex
            .pending_approvals
            .push(next.clone());
        app.route_shortcut_approval();
        assert_eq!(app.current_approval_id(), Some(request.id));
        app.select_page(Page::Dashboard);
        app.route_shortcut_approval();
        assert_eq!(
            app.page,
            Page::Dashboard,
            "same queue must not force the page repeatedly"
        );
        app.select_page(Page::Shortcuts);
        app.cycle_approval(false);
        assert_eq!(app.current_approval_id(), Some(next.id));
        app.handle_action(ControlAction::Back);
        assert!(
            commands.try_recv().is_err(),
            "newly cycled request must be rendered first"
        );
        shortcuts_frame(&mut app, &ctx, vec![]);
        app.handle_action(ControlAction::Back);
        assert!(
            matches!(commands.try_recv().unwrap(), ClientCommand::CodexApprovalResponse { approval_id, decision: ApprovalDecisionDto::Reject } if approval_id == next.id)
        );
        app.handle_action(ControlAction::Back);
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn shortcut_clicks_send_one_decision_and_cannot_cross_request_replacement_or_disconnect() {
        for (key, decision) in [
            (0, ApprovalDecisionDto::Approve),
            (1, ApprovalDecisionDto::Reject),
        ] {
            let mut snapshot = crate::test_support::snapshot();
            let first = shortcut_request("a", "new");
            snapshot.codex.pending_approvals.push(first.clone());
            let (mut app, mut commands) = test_app(&snapshot);
            app.select_project("/Users/mac/project-a");
            app.route_shortcut_approval();
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            shortcuts_frame(&mut app, &ctx, vec![]);
            let keys = shortcuts_frame(&mut app, &ctx, vec![]);
            shortcuts_frame(&mut app, &ctx, pointer_events(keys[key], true));
            let replacement = shortcut_request("a", "new");
            app.model.snapshot.as_mut().unwrap().codex.pending_approvals =
                vec![replacement.clone()];
            app.route_shortcut_approval();
            shortcuts_frame(&mut app, &ctx, pointer_events(keys[key], false));
            assert!(
                commands.try_recv().is_err(),
                "held pointer must not decide a replacement request"
            );
            shortcuts_frame(&mut app, &ctx, vec![]);
            shortcuts_frame(&mut app, &ctx, pointer_events(keys[key], true));
            shortcuts_frame(&mut app, &ctx, pointer_events(keys[key], false));
            assert!(
                matches!(commands.try_recv().unwrap(), ClientCommand::CodexApprovalResponse { approval_id, decision: actual } if approval_id == replacement.id && actual == decision)
            );
            for pressed in [true, false] {
                shortcuts_frame(&mut app, &ctx, pointer_events(keys[key], pressed));
            }
            assert!(commands.try_recv().is_err(), "sending must lock both keys");

            app.model
                .apply_network(crate::model::NetworkEvent::Disconnected {
                    message: "test disconnect".to_owned(),
                    retry_ms: 1,
                });
            for pressed in [true, false] {
                shortcuts_frame(&mut app, &ctx, pointer_events(keys[1 - key], pressed));
            }
            app.handle_action(ControlAction::Back);
            assert!(commands.try_recv().is_err());
            app.model
                .apply_network(crate::model::NetworkEvent::Connected { latency_ms: 1 });
            app.handle_action(ControlAction::Back);
            assert!(
                commands.try_recv().is_err(),
                "reconnect needs a new snapshot"
            );
        }
    }

    #[test]
    fn shortcut_keyboard_and_unassigned_keys_never_decide_an_approval() {
        let mut snapshot = crate::test_support::snapshot();
        snapshot
            .codex
            .pending_approvals
            .push(shortcut_request("a", "new"));
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        app.route_shortcut_approval();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        shortcuts_frame(&mut app, &ctx, vec![]);
        for key in [egui::Key::Enter, egui::Key::Escape, egui::Key::Space] {
            shortcuts_frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            for action in app.controller.poll(&ctx, true, true) {
                app.handle_action(action);
            }
        }
        assert!(commands.try_recv().is_err());
        app.handle_action(ControlAction::NavigateDown);
        assert_eq!(app.focus_index, 5);
        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_err());
        assert!(app.editor.is_some(), "an empty key opens its editor");
        app.handle_action(ControlAction::Back);
        assert!(app.editor.is_none());
        assert!(
            commands.try_recv().is_err(),
            "closing the editor must not reject an approval"
        );
        shortcuts_frame(&mut app, &ctx, vec![]);
        app.handle_action(ControlAction::NavigateUp);
        app.handle_action(ControlAction::NavigateRight);
        assert_eq!(app.focus_index, 1);
        app.handle_action(ControlAction::Activate);
        assert!(matches!(
            commands.try_recv().unwrap(),
            ClientCommand::CodexApprovalResponse {
                decision: ApprovalDecisionDto::Reject,
                ..
            }
        ));
    }

    #[test]
    fn key_editor_never_decides_or_executes_the_action_being_assigned() {
        let mut snapshot = crate::test_support::snapshot();
        let request = shortcut_request("a", "new");
        snapshot.codex.pending_approvals.push(request.clone());
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        app.route_shortcut_approval();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        shortcuts_frame(&mut app, &ctx, vec![]);
        app.open_key_editor(2);
        assert!(app.current_approval_id().is_none());
        app.route_shortcut_approval();
        app.handle_action(ControlAction::NavigateDown);
        app.handle_action(ControlAction::NavigateDown);
        app.handle_action(ControlAction::Activate);
        assert!(app.editor.is_none());
        assert_eq!(
            app.preferences.action(2),
            Some(orangedeck_domain::Shortcut::Refresh)
        );
        assert!(commands.try_recv().is_err());
        assert!(app.displayed_approval.is_none());
        app.activate_custom_key(2);
        assert_eq!(
            commands.try_recv().unwrap(),
            ClientCommand::CodexRefreshThreads
        );
        assert!(commands.try_recv().is_err());
        app.open_key_editor(2);
        app.handle_action(ControlAction::Back);
        assert!(app.editor.is_none());
        assert!(commands.try_recv().is_err());
        assert_eq!(app.current_approval_id(), Some(request.id));
    }

    #[test]
    fn host_keys_use_registered_ids_and_refuse_external_or_unconfigured_targets() {
        use orangedeck_domain::Shortcut;
        let mut snapshot = crate::test_support::snapshot();
        snapshot.projects.push(orangedeck_protocol::ProjectDto {
            id: "registered".to_owned(),
            name: "Registered".to_owned(),
            path: "/Users/mac/project-a".to_owned(),
            has_browser_url: false,
        });
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        for (action, expected) in [
            (
                Shortcut::OpenEditor,
                ClientCommand::OpenEditor {
                    project_id: "registered".to_owned(),
                },
            ),
            (
                Shortcut::OpenTerminal,
                ClientCommand::OpenTerminal {
                    project_id: "registered".to_owned(),
                },
            ),
            (
                Shortcut::OpenProject,
                ClientCommand::OpenProject {
                    project_id: "registered".to_owned(),
                },
            ),
        ] {
            app.preferences.assign(2, Some(action));
            app.activate_custom_key(2);
            assert_eq!(commands.try_recv().unwrap(), expected);
        }
        app.preferences.assign(2, Some(Shortcut::OpenBrowser));
        app.activate_custom_key(2);
        assert!(commands.try_recv().is_err());
        app.model.snapshot.as_mut().unwrap().projects[0].has_browser_url = true;
        app.activate_custom_key(2);
        assert_eq!(
            commands.try_recv().unwrap(),
            ClientCommand::OpenBrowser {
                project_id: "registered".to_owned()
            }
        );
        app.select_project("/Users/mac/project-b");
        app.activate_custom_key(2);
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn language_buttons_switch_all_five_tabs_and_editor_without_translating_user_data() {
        let snapshot = crate::test_support::snapshot();
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let frame = |app: &mut OrangeDeckApp, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(820.0, 480.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.render_header(ui),
            )
        };
        let output = frame(&mut app, vec![]);
        for shape in &output.shapes {
            if let egui::Shape::Rect(rect) = &shape.shape
                && rect.fill == theme::ORANGE
            {
                assert!(
                    shape.clip_rect.contains_rect(rect.rect),
                    "Language button clipped by the header"
                );
            }
        }
        let en = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == "EN" => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap();
        output.drop_without_applying_deltas();
        frame(&mut app, pointer_events(en, true)).drop_without_applying_deltas();
        frame(&mut app, pointer_events(en, false)).drop_without_applying_deltas();
        assert_eq!(app.preferences.language, Language::English);
        assert_eq!(i18n::language(&ctx), Language::English);
        app.use_recommended_keys();
        for page in Page::ALL {
            app.page = page;
            let labels = crate::test_support::render_text(1038.0, 584.0, |ui| {
                i18n::set_language(ui.ctx(), Language::English);
                app.render_header(ui);
                app.render_nav(ui);
                egui::CentralPanel::default().show(ui, |ui| {
                    app.render_project_picker(ui, &snapshot);
                    match page {
                        Page::Dashboard => app.render_dashboard(ui, &snapshot),
                        Page::Shortcuts => app.render_shortcuts(ui, &snapshot),
                        Page::Projects => app.render_projects(ui, &snapshot),
                        Page::Codex => app.render_codex(ui, &snapshot),
                        Page::Notifications => app.render_notifications(ui, &snapshot),
                    }
                });
            });
            for label in labels
                .iter()
                .filter(|label| label.rect.intersects(label.clip))
            {
                assert!(
                    label.text == "한국어"
                        || !label.text.chars().any(|ch| ('가'..='힣').contains(&ch)),
                    "Untranslated {page:?}: {}",
                    label.text
                );
            }
        }
        app.open_key_editor(2);
        let labels = crate::test_support::render_text(820.0, 480.0, |ui| {
            i18n::set_language(ui.ctx(), Language::English);
            app.render_key_editor(ui.ctx());
        });
        assert!(
            labels
                .iter()
                .any(|label| label.text.starts_with("Assign key"))
        );
        assert!(
            labels
                .iter()
                .all(|label| !label.text.chars().any(|ch| ('가'..='힣').contains(&ch)))
        );
        assert!(commands.try_recv().is_err());
        assert_eq!(
            app.model.snapshot.as_ref().unwrap().codex.threads[0].observation,
            snapshot.codex.threads[0].observation
        );
    }

    #[test]
    fn five_tabs_and_all_ten_key_labels_fit_a_small_complete_window() {
        let mut snapshot = crate::test_support::snapshot();
        snapshot
            .codex
            .pending_approvals
            .push(shortcut_request("a", "new"));
        let (mut app, _) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        app.route_shortcut_approval();
        for (width, height) in [(820.0, 480.0), (1038.0, 584.0)] {
            let labels = crate::test_support::render_text(width, height, |ui| {
                app.render_header(ui);
                app.render_nav(ui);
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().inner_margin(12.0))
                    .show(ui, |ui| {
                        app.render_project_picker(ui, &snapshot);
                        app.render_shortcuts(ui, &snapshot);
                    });
            });
            for label in labels.iter().filter(|label| {
                label.text == "승인"
                    || label.text == "거절"
                    || label.text == "미지정"
                    || label.text == "알림"
            }) {
                assert!(
                    label.clip.contains_rect(label.rect),
                    "clipped {}: {:?}",
                    label.text,
                    label.rect
                );
            }
            assert_eq!(
                labels.iter().filter(|label| label.text == "미지정").count(),
                8
            );
            assert!(labels.iter().any(|label| label.text == "단축키"));
            assert!(labels.iter().any(|label| label.text == "알림"));
        }
    }

    #[test]
    fn live_recent_auto_click_returns_from_a_pinned_thread_and_follows_only_its_project() {
        for (width, height) in [(676.0, 442.0), (894.0, 380.0)] {
            let mut snapshot = crate::test_support::snapshot();
            let mut older = snapshot.codex.threads[0].clone();
            older.id = "older".to_owned();
            older.updated_at -= 10;
            older.live_usage = None;
            snapshot.codex.threads.push(older);
            snapshot.codex.threads[1].updated_at += 100;
            let (mut app, mut commands) = test_app(&snapshot);
            app.select_project("/Users/mac/project-a");
            app.change_thread(1);
            assert_eq!(app.selection.thread.as_deref(), Some("older"));
            assert!(!app.selection.follow_latest);

            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let mut time = 0.0;
            let mut frame =
                |app: &mut OrangeDeckApp, snapshot: &SnapshotDto, events: Vec<egui::Event>| {
                    time += 0.1;
                    app.model.snapshot = Some(snapshot.clone());
                    app.update_monitor_selection();
                    ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, height),
                            )),
                            time: Some(time),
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE)
                                .show(ui, |ui| app.render_dashboard(ui, snapshot));
                        },
                    )
                };
            frame(&mut app, &snapshot, vec![]).drop_without_applying_deltas();
            let output = frame(&mut app, &snapshot, vec![]);
            let button = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "최근 자동" => {
                        Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                    }
                    _ => None,
                })
                .expect("LIVE recent-auto button is rendered");
            output.drop_without_applying_deltas();
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &snapshot,
                    vec![
                        egui::Event::PointerMoved(button),
                        egui::Event::PointerButton {
                            pos: button,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .drop_without_applying_deltas();
            }
            assert!(
                app.selection.follow_latest,
                "LIVE click must enable following"
            );
            let output = frame(&mut app, &snapshot, vec![]);
            let (shape, text) = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == "자동 ON" => {
                        Some((shape, text))
                    }
                    _ => None,
                })
                .expect("enabled auto mode must be visible after the click");
            assert!(
                shape
                    .clip_rect
                    .contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
            );
            output.drop_without_applying_deltas();
            assert_eq!(app.selection.thread.as_deref(), Some("a"));
            snapshot.codex.threads[2].updated_at += 200;
            frame(&mut app, &snapshot, vec![]).drop_without_applying_deltas();
            assert_eq!(app.selection.thread.as_deref(), Some("older"));
            snapshot.codex.threads[1].updated_at += 1000;
            frame(&mut app, &snapshot, vec![]).drop_without_applying_deltas();
            assert_eq!(app.selection.thread.as_deref(), Some("older"));
            assert_eq!(
                app.selection.project.as_deref(),
                Some("/Users/mac/project-a")
            );
            while let Ok(command) = commands.try_recv() {
                assert!(matches!(command, ClientCommand::CodexReadThread { .. }));
            }
        }
    }

    #[test]
    fn current_completion_stays_large_until_acknowledged_and_other_projects_do_not_interrupt() {
        let snapshot = crate::test_support::snapshot();
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        announce_test_alert(&mut app, "b");
        assert!(app.toast.is_none());
        announce_test_alert(&mut app, "a");
        app.toast.as_mut().unwrap().1 = Instant::now().checked_sub(Duration::from_mins(2)).unwrap();
        assert!(app.current_unread());
        for (width, height) in [(676.0, 480.0), (1038.0, 550.0)] {
            let labels =
                crate::test_support::render_text(width, height, |ui| app.render_toast(ui.ctx()));
            for expected in [
                "응답 완료",
                "CURRENT QUESTION",
                "내용 보기 · A",
                "확인했어요 · B",
            ] {
                let label = labels
                    .iter()
                    .find(|label| label.text == expected)
                    .unwrap_or_else(|| panic!("Missing {expected}"));
                assert!(
                    label.clip.contains_rect(label.rect),
                    "clipped {expected}: {:?}",
                    label.rect
                );
                if expected == "응답 완료" {
                    assert!(label.font_size >= 30.0);
                }
            }
        }
        assert!(app.toast.is_some());
        app.toast.as_mut().unwrap().0.notification.title =
            "매우 긴 응답 완료 알림 제목입니다 ".repeat(20);
        app.toast.as_mut().unwrap().0.notification.body = "긴 응답 내용을 표시합니다 ".repeat(100);
        let labels =
            crate::test_support::render_text(676.0, 480.0, |ui| app.render_toast(ui.ctx()));
        for expected in ["내용 보기 · A", "확인했어요 · B"] {
            let label = labels.iter().find(|label| label.text == expected).unwrap();
            assert!(label.clip.contains_rect(label.rect));
        }
        app.handle_action(ControlAction::Back);
        assert!(app.toast.is_none());
        assert!(!app.current_unread());
        assert!(app.attention_color().is_none());
        assert!(commands.try_recv().is_err());
        // Other project's read state was not changed by acknowledging this turn.
        assert_eq!(app.model.alerts.unread(), 1);
        announce_test_alert(&mut app, "a");
        app.model.snapshot.as_mut().unwrap().codex.threads[0].active_turn_id =
            Some("next".to_owned());
        app.announce_notifications(&egui::Context::default());
        assert!(app.toast.is_none());
    }

    #[test]
    fn project_changes_filter_all_pages_and_only_a_rendered_current_request_can_be_approved() {
        let mut snapshot = crate::test_support::snapshot();
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        for (id, thread_id, turn_id) in [(first, "a", "new"), (second, "b", "other-turn")] {
            snapshot
                .codex
                .pending_approvals
                .push(orangedeck_protocol::ApprovalDto {
                    id,
                    thread_id: Some(thread_id.to_owned()),
                    turn_id: Some(turn_id.to_owned()),
                    kind: orangedeck_protocol::ApprovalKindDto::CommandExecution,
                    title: "Approval".to_owned(),
                    summary: format!("Approve {thread_id}"),
                    details: vec!["Synthetic test request".to_owned()],
                    requested_at: chrono::Utc::now(),
                });
        }
        snapshot.codex.threads[1].active_turn_id = Some("other-turn".to_owned());
        let (mut app, mut commands) = test_app(&snapshot);
        app.select_project("/Users/mac/project-a");
        let live =
            crate::test_support::render(930.0, 600.0, |ui| app.render_dashboard(ui, &snapshot));
        app.select_page(Page::Codex);
        let conversations =
            crate::test_support::render(930.0, 600.0, |ui| app.render_codex(ui, &snapshot));
        assert!(live.contains("CURRENT QUESTION"));
        assert!(conversations.contains("CURRENT QUESTION"));
        assert!(conversations.contains("현재 LIVE"));
        assert!(!conversations.contains("OTHER PROJECT QUESTION"));
        assert!(app.current_approval_id().is_none());
        app.open_notifications();
        assert_eq!(app.current_approval_id(), Some(first));
        app.displayed_approval = Some(first);
        announce_test_alert(&mut app, "a");
        assert!(app.current_approval_id().is_none());
        app.handle_action(ControlAction::Activate);
        assert!(app.toast.is_none());
        assert!(commands.try_recv().is_err());
        assert!(app.displayed_approval.is_none());

        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_err());
        crate::test_support::render(930.0, 600.0, |ui| app.render_approval(ui, &snapshot));
        app.select_project("/Users/mac/project-b");
        assert_eq!(app.current_approval_id(), Some(second));
        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_err());
        assert_eq!(app.selection.threads(&snapshot).len(), 1);
        assert_eq!(app.selection.selected(&snapshot).unwrap().id, "b");
        crate::test_support::render(930.0, 600.0, |ui| app.render_approval(ui, &snapshot));
        app.handle_action(ControlAction::Activate);
        assert!(
            matches!(commands.try_recv().unwrap(), ClientCommand::CodexApprovalResponse { approval_id, decision: ApprovalDecisionDto::Approve } if approval_id == second)
        );
        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_err());
    }
    #[test]
    fn shortcuts_are_second_and_notifications_are_fifth() {
        assert_eq!(
            Page::ALL.map(Page::label),
            ["LIVE", "단축키", "프로젝트들", "대화", "알림"]
        );
    }
}
