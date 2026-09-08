use super::{ControlAction, OrangeDeckApp, egui, notifications, selection, theme};
use crate::{
    model::NetworkEvent,
    paired::{self, FileState},
};
use eframe::egui::RichText;
use orangedeck_protocol::{
    ApprovalDecisionDto, ApprovalDto, ClientCommand, CodeChangeKindDto, CodexThreadDto,
    CodexThreadStatusDto, SnapshotDto,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModalKind {
    Response,
    Files,
}

#[derive(Clone)]
pub(super) struct DeckModal {
    pub kind: ModalKind,
    pub thread: CodexThreadDto,
    pub turn_id: Option<String>,
    pub label: String,
    pub selected_file: usize,
    pub submitted: Option<uuid::Uuid>,
    pub just_opened: bool,
    pub file_request: Option<uuid::Uuid>,
    pub file_message: Option<String>,
    pub file_error: bool,
    pub connector_update_needed: bool,
    pub file_refresh: Option<std::time::Instant>,
}

pub(super) fn file_state(thread: &CodexThreadDto) -> FileState {
    let Some(observation) = thread
        .observation
        .as_ref()
        .filter(|observation| observation.turn_id.as_deref() == selection::turn_id(thread))
    else {
        return FileState::Loading;
    };
    let Some(changes) = &observation.changes else {
        return FileState::Unavailable;
    };
    if !changes.files.is_empty() {
        return FileState::Changes(changes.files.len());
    }
    if matches!(
        observation.last_turn_status,
        CodexThreadStatusDto::Working | CodexThreadStatusDto::WaitingApproval
    ) {
        FileState::Loading
    } else if changes.truncated {
        FileState::Unavailable
    } else {
        FileState::None
    }
}

impl OrangeDeckApp {
    pub(super) fn pair_views(&self, snapshot: &SnapshotDto) -> [paired::PairView; 5] {
        let lang = self.preferences.language;
        std::array::from_fn(|index| {
            let slot = &self.preferences.conversations[index];
            let thread = snapshot
                .codex
                .threads
                .iter()
                .find(|thread| thread.id == slot.thread_id);
            let pending = thread.is_some_and(|thread| {
                snapshot.codex.pending_approvals.iter().any(|approval| {
                    approval.thread_id.as_deref() == Some(&thread.id)
                        && approval.turn_id.as_deref() == selection::turn_id(thread)
                })
            });
            let unread = thread.is_some_and(|thread| self.model.alerts.current_unread(thread));
            let label = if slot.label.is_empty() {
                thread.map_or_else(
                    || format!("Codex {}", index + 1),
                    |thread| thread.title.clone(),
                )
            } else {
                slot.label.clone()
            };
            let status = if !self.model.connected {
                lang.text("연결 끊김", "Disconnected")
            } else if pending {
                lang.text("승인 대기", "Approval needed")
            } else if unread {
                lang.text("새 응답", "New response")
            } else if let Some(thread) = thread {
                match selection::status(
                    thread,
                    snapshot,
                    self.model.connected,
                    chrono::Utc::now().timestamp(),
                ) {
                    CodexThreadStatusDto::Working => lang.text("작업 중", "Working"),
                    CodexThreadStatusDto::Completed => lang.text("응답 완료", "Complete"),
                    CodexThreadStatusDto::WaitingApproval => lang.text("답변 필요", "Input needed"),
                    CodexThreadStatusDto::Error => lang.text("오류", "Error"),
                    _ => lang.text("대기", "Idle"),
                }
            } else if slot.thread_id.is_empty() {
                lang.text("+ 눌러 선택", "Tap + to choose")
            } else {
                lang.text("대화 확인 중", "Finding conversation")
            };
            paired::PairView {
                identity: format!(
                    "{}:{}",
                    slot.thread_id,
                    thread.and_then(selection::turn_id).unwrap_or("")
                ),
                label,
                status: status.to_owned(),
                assigned: !slot.thread_id.is_empty(),
                available: thread.is_some(),
                attention: pending || unread,
                files: thread.map_or(FileState::Unavailable, file_state),
            }
        })
    }

    pub(super) fn render_agents(&mut self, ui: &mut egui::Ui, snapshot: &SnapshotDto) {
        let pairs = self.pair_views(snapshot);
        let action = paired::render(
            ui,
            &paired::DeckView {
                pairs: &pairs,
                focused: self.focus_index,
                pulse: Self::attention_pulse(ui.ctx()),
                editing: self.deck_editing,
                sound: self.preferences.notification_sound,
                connected: self.model.connected,
            },
        );
        if let Some(column) = action.edit {
            self.open_key_editor(column);
        } else if let Some(index) = action.activate {
            self.activate_pair(index);
        } else if action.toggle_edit {
            self.deck_editing = !self.deck_editing;
        } else if action.toggle_sound {
            self.preferences.notification_sound = !self.preferences.notification_sound;
            self.save_preferences();
        }
    }

    pub(super) fn activate_pair(&mut self, index: usize) {
        if index >= 10 {
            return;
        }
        let column = index % 5;
        let slot = &self.preferences.conversations[column];
        if index < 5 && (self.deck_editing || slot.thread_id.is_empty()) {
            self.open_key_editor(column);
            return;
        }
        if self.deck_editing {
            return;
        }
        let Some(thread) = self
            .model
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .codex
                    .threads
                    .iter()
                    .find(|thread| thread.id == slot.thread_id)
            })
            .cloned()
        else {
            return;
        };
        let kind = if index < 5 {
            ModalKind::Response
        } else {
            ModalKind::Files
        };
        // A no-edits key is inert for touch, mouse, keyboard and controller alike.
        if kind == ModalKind::Files && file_state(&thread) == FileState::None {
            return;
        }
        let label = if slot.label.is_empty() {
            thread.title.clone()
        } else {
            slot.label.clone()
        };
        self.model.alerts.mark_current_read(&thread);
        self.displayed_approval = None;
        self.selected_approval = None;
        self.model.command_message = None;
        self.deck_modal = Some(DeckModal {
            kind,
            turn_id: selection::turn_id(&thread).map(str::to_owned),
            thread,
            label,
            selected_file: 0,
            submitted: None,
            just_opened: true,
            file_request: None,
            file_message: None,
            file_error: false,
            connector_update_needed: false,
            file_refresh: None,
        });
        if kind == ModalKind::Files {
            let first = self
                .deck_modal
                .as_ref()
                .and_then(|modal| modal.thread.observation.as_ref())
                .and_then(|observation| observation.changes.as_ref())
                .and_then(|changes| {
                    changes
                        .files
                        .iter()
                        .position(|file| file.kind != CodeChangeKindDto::Deleted)
                });
            if let Some(index) = first {
                self.open_modal_file(index);
            } else if !matches!(
                self.deck_modal
                    .as_ref()
                    .map(|modal| file_state(&modal.thread)),
                Some(FileState::Changes(_))
            ) {
                self.refresh_modal_files();
            }
        }
    }

    pub(super) fn close_deck_modal(&mut self) {
        self.deck_modal = None;
        self.displayed_approval = None;
        self.selected_approval = None;
    }

    pub(super) fn pending_for_modal(&self) -> Vec<&ApprovalDto> {
        let Some(modal) = self
            .deck_modal
            .as_ref()
            .filter(|modal| modal.kind == ModalKind::Response)
        else {
            return Vec::new();
        };
        self.model
            .snapshot
            .as_ref()
            .map_or_else(Vec::new, |snapshot| {
                snapshot
                    .codex
                    .pending_approvals
                    .iter()
                    .filter(|approval| {
                        approval.thread_id.as_deref() == Some(&modal.thread.id)
                            && approval.turn_id.as_deref() == modal.turn_id.as_deref()
                            && modal.turn_id.is_some()
                    })
                    .collect()
            })
    }

    pub(super) fn update_deck_watches(&mut self) {
        if !self.model.approval_ready() {
            self.watched_deck = None;
            return;
        }
        let Some(snapshot) = &self.model.snapshot else {
            return;
        };
        if !snapshot
            .codex
            .supported_features
            .iter()
            .any(|feature| feature == "paired_conversations")
        {
            return;
        }
        let mut ids = self
            .preferences
            .conversations
            .iter()
            .filter(|slot| {
                !slot.thread_id.is_empty()
                    && snapshot
                        .codex
                        .threads
                        .iter()
                        .any(|thread| thread.id == slot.thread_id)
            })
            .map(|slot| slot.thread_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        if self.watched_deck.as_ref() != Some(&ids) {
            self.watched_deck = Some(ids.clone());
            self.send(ClientCommand::CodexWatchThreads { thread_ids: ids });
        }
        // Refresh the visible response only within its original turn. A later query
        // cannot replace what the user is reading or receive its approval input.
        if let Some(modal) = &mut self.deck_modal
            && modal.kind == ModalKind::Response
            && let Some(thread) = self.model.snapshot.as_ref().and_then(|snapshot| {
                snapshot
                    .codex
                    .threads
                    .iter()
                    .find(|thread| thread.id == modal.thread.id)
            })
            && selection::turn_id(thread) == modal.turn_id.as_deref()
        {
            modal.thread = thread.clone();
        }
    }

    pub(super) fn handle_modal_input(&mut self, action: ControlAction) {
        let Some(modal) = &self.deck_modal else {
            return;
        };
        if action == ControlAction::Context {
            self.close_deck_modal();
            return;
        }
        if modal.kind == ModalKind::Response {
            if let Some(id) = self.current_approval_id() {
                match action {
                    ControlAction::Activate | ControlAction::Back
                        if self.displayed_approval == Some(id) =>
                    {
                        self.submit_approval(
                            id,
                            if action == ControlAction::Activate {
                                ApprovalDecisionDto::Approve
                            } else {
                                ApprovalDecisionDto::Reject
                            },
                        );
                    }
                    ControlAction::PreviousThread | ControlAction::NextThread => {
                        self.cycle_modal_approval(action == ControlAction::PreviousThread);
                    }
                    _ => {}
                }
            } else if action == ControlAction::Back {
                self.close_deck_modal();
            }
        } else {
            match action {
                ControlAction::Back => self.close_deck_modal(),
                ControlAction::Activate => self.open_modal_file(modal.selected_file),
                ControlAction::NavigateUp | ControlAction::NavigateDown => {
                    let modal = self.deck_modal.as_mut().unwrap();
                    let count = modal
                        .thread
                        .observation
                        .as_ref()
                        .and_then(|observation| observation.changes.as_ref())
                        .map_or(0, |changes| changes.files.len());
                    modal.selected_file = modal
                        .selected_file
                        .saturating_add_signed(if action == ControlAction::NavigateUp {
                            -1
                        } else {
                            1
                        })
                        .min(count.saturating_sub(1));
                }
                _ => {}
            }
        }
    }

    fn cycle_modal_approval(&mut self, previous: bool) {
        let pending = self.pending_for_modal();
        if pending.is_empty() {
            return;
        }
        let index = pending
            .iter()
            .position(|approval| Some(approval.id) == self.selected_approval)
            .unwrap_or(0);
        let next = if previous {
            (index + pending.len() - 1) % pending.len()
        } else {
            (index + 1) % pending.len()
        };
        self.selected_approval = Some(pending[next].id);
        self.displayed_approval = None;
    }

    fn refresh_modal_files(&mut self) {
        let Some(modal) = self
            .deck_modal
            .as_mut()
            .filter(|modal| modal.kind == ModalKind::Files)
        else {
            return;
        };
        if modal.file_refresh.is_some() {
            return;
        }
        modal.file_error = !self.model.file_navigation_ready();
        modal.file_message = Some(
            self.preferences
                .language
                .text(
                    if modal.file_error {
                        "Mac 연결이 끊겼습니다. 다시 연결한 뒤 확인하세요."
                    } else {
                        "Mac에서 이 질의의 파일 기록을 다시 가져오는 중…"
                    },
                    if modal.file_error {
                        "Mac disconnected. Reconnect, then check again."
                    } else {
                        "Reading this turn's file records from the Mac…"
                    },
                )
                .to_owned(),
        );
        if modal.file_error {
            return;
        }
        modal.file_refresh = Some(std::time::Instant::now());
        if let Err(error) = self.network.send(ClientCommand::CodexReadThread {
            thread_id: modal.thread.id.clone(),
        }) {
            modal.file_refresh = None;
            modal.file_error = true;
            modal.file_message = Some(error);
        }
    }

    fn sync_file_modal(&mut self) {
        let Some(modal) = self
            .deck_modal
            .as_mut()
            .filter(|modal| modal.kind == ModalKind::Files)
        else {
            return;
        };
        let refreshing = modal.file_refresh.is_some();
        let current = self.model.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .codex
                .threads
                .iter()
                .find(|thread| thread.id == modal.thread.id)
        });
        let fresh = current.filter(|thread| {
            thread.observation.as_ref().is_some_and(|observation| {
                modal.thread.observation.as_ref().is_none_or(|old| {
                    observation.observed_at >= old.observed_at
                        && (observation.changes != old.changes
                            || refreshing && observation.observed_at > old.observed_at)
                })
            })
        });
        let lang = self.preferences.language;
        if let Some(thread) = fresh {
            if let Some(expected) = modal.turn_id.as_deref()
                && (selection::turn_id(thread) != Some(expected)
                    || thread
                        .observation
                        .as_ref()
                        .and_then(|observation| observation.turn_id.as_deref())
                        != Some(expected))
            {
                if !refreshing {
                    return;
                }
                modal.file_refresh = None;
                modal.file_error = true;
                modal.file_message = Some(lang.text("대화가 새 질의로 바뀌었습니다. 창을 닫고 다시 확인하세요.", "The conversation moved to another question. Close this window and reopen it.").to_owned());
                return;
            }
            let old_changes = modal
                .thread
                .observation
                .as_ref()
                .and_then(|value| value.changes.as_ref());
            let was_empty = old_changes.is_none_or(|changes| changes.files.is_empty());
            let selected_path = old_changes
                .and_then(|changes| changes.files.get(modal.selected_file))
                .map(|file| file.path.clone());
            modal.thread = thread.clone();
            modal.turn_id = selection::turn_id(thread).map(str::to_owned);
            modal.selected_file = thread
                .observation
                .as_ref()
                .and_then(|value| value.changes.as_ref())
                .and_then(|changes| {
                    changes
                        .files
                        .iter()
                        .position(|file| Some(&file.path) == selected_path.as_ref())
                })
                .unwrap_or(0);
            let state = file_state(thread);
            if !refreshing {
                // Live captures update the open list without repeated editor launches.
                if was_empty && matches!(state, FileState::Changes(_)) {
                    modal.file_error = false;
                    modal.file_message = None;
                }
                return;
            }
            if state != FileState::Loading {
                modal.file_refresh = None;
                modal.file_error = state == FileState::Unavailable;
                modal.file_message = match state {
                    FileState::Unavailable => Some(lang.text(
                        "이 질의의 파일 기록이 불완전합니다. Mac에서 업데이트 후 Enable Codex Notifications.command를 다시 실행하고 Codex /hooks에서 신뢰하세요. 이후 작업부터 수집하며, 이전에 없던 기록은 복원할 수 없습니다.",
                        "This turn's file records are incomplete. After updating the Mac, rerun Enable Codex Notifications.command and trust it in Codex /hooks. Capture applies to subsequent work; missing earlier records cannot be reconstructed.",
                    ).to_owned()),
                    FileState::None => Some(lang.text(
                        "이 질의에 기록된 파일 수정이 없습니다.",
                        "No recorded file edits in this turn.",
                    ).to_owned()),
                    _ => None,
                };
                if let Some(index) = thread
                    .observation
                    .as_ref()
                    .and_then(|o| o.changes.as_ref())
                    .and_then(|c| {
                        c.files
                            .iter()
                            .position(|f| f.kind != CodeChangeKindDto::Deleted)
                    })
                {
                    self.open_modal_file(index);
                }
                return;
            }
        }
        if modal
            .file_refresh
            .is_some_and(|started| started.elapsed() > std::time::Duration::from_secs(12))
        {
            modal.file_refresh = None;
            modal.file_error = true;
            modal.file_message = Some(
                lang.text(
                    "Mac에서 파일 기록을 받지 못했습니다. 연결 상태를 확인하고 다시 시도하세요.",
                    "No file records received from the Mac. Check the connection and try again.",
                )
                .to_owned(),
            );
        }
    }

    fn open_modal_file(&mut self, index: usize) {
        if self
            .deck_modal
            .as_ref()
            .is_some_and(|modal| !matches!(file_state(&modal.thread), FileState::Changes(_)))
        {
            self.refresh_modal_files();
            return;
        }
        if !self.model.file_navigation_ready() {
            if let Some(modal) = &mut self.deck_modal {
                modal.file_error = true;
                modal.file_message = Some(self.preferences.language.text("Mac 연결이 끊겼습니다. 파일 목록은 볼 수 있으며, 다시 연결한 뒤 열어 주세요.", "Mac disconnected. You can review the file list and retry after reconnecting.").to_owned());
            }
            return;
        }
        let Some(modal) = self
            .deck_modal
            .as_mut()
            .filter(|modal| modal.kind == ModalKind::Files)
        else {
            return;
        };
        let Some(file) = modal
            .thread
            .observation
            .as_ref()
            .and_then(|observation| observation.changes.as_ref())
            .and_then(|changes| changes.files.get(index))
        else {
            return;
        };
        modal.selected_file = index;
        if file.kind == CodeChangeKindDto::Deleted {
            modal.file_message = None;
            modal.file_error = false;
            modal.connector_update_needed = false;
            return;
        }
        let Some(turn_id) = modal.turn_id.clone() else {
            return;
        };
        modal.file_message = Some(
            self.preferences
                .language
                .text("Mac 편집기에서 여는 중…", "Opening in your Mac editor…")
                .to_owned(),
        );
        modal.file_error = false;
        modal.connector_update_needed = false;
        // Keep the most recent selection queued while a previous editor navigation is in flight.
        if self.file_request.is_some() {
            return;
        }
        let navigation_id = uuid::Uuid::new_v4();
        let path = file.path.clone();
        let command = ClientCommand::OpenCodexChange {
            navigation_id,
            thread_id: modal.thread.id.clone(),
            turn_id,
            path: path.clone(),
        };
        modal.file_request = Some(navigation_id);
        self.file_request = Some((navigation_id, path));
        if let Err(error) = self.network.send(command) {
            self.file_request = None;
            if let Some(modal) = &mut self.deck_modal {
                modal.file_message = Some(error);
                modal.file_error = true;
                modal.file_request = None;
            }
        }
    }

    pub(super) fn process_deck_network(&mut self, event: &NetworkEvent) -> bool {
        if matches!(event, NetworkEvent::Server(envelope) if envelope.protocol_version != orangedeck_protocol::PROTOCOL_VERSION)
        {
            return false;
        }
        if let NetworkEvent::FileOpenCompleted {
            navigation_id,
            result,
        } = event
            && self
                .file_request
                .as_ref()
                .is_some_and(|(id, _)| id == navigation_id)
        {
            let (_, path) = self.file_request.take().unwrap();
            let mut next = None;
            if let Some(modal) = self
                .deck_modal
                .as_mut()
                .filter(|modal| modal.kind == ModalKind::Files)
            {
                let same_request = modal.file_request == Some(*navigation_id);
                if same_request {
                    modal.file_request = None;
                    modal.file_error = !result.as_ref().is_ok_and(|response| response.accepted);
                    modal.connector_update_needed = result.as_ref().err().is_some_and(|message| {
                        message.starts_with("editor_project_not_registered:")
                            || message.starts_with(
                                "file_not_allowed: Mac에 등록한 프로젝트의 수정 파일만",
                            )
                    });
                    modal.file_message = Some(match result {
                        Ok(response) if response.accepted => self
                            .preferences
                            .language
                            .text(
                                "Mac 편집기로 파일 열기를 보냈습니다.",
                                "File sent to your Mac editor.",
                            )
                            .to_owned(),
                        Ok(response) => response.message.clone(),
                        Err(message) => {
                            let text = message
                                .split_once(": ")
                                .map_or(message.as_str(), |(_, text)| text);
                            text.split_once(" / ").map_or_else(
                                || text.to_owned(),
                                |(ko, en)| self.preferences.language.text(ko, en).to_owned(),
                            )
                        }
                    });
                }
                if let Some(file) = modal
                    .thread
                    .observation
                    .as_ref()
                    .and_then(|observation| observation.changes.as_ref())
                    .and_then(|changes| changes.files.get(modal.selected_file))
                    && (!same_request || file.path != path)
                {
                    next = Some(modal.selected_file);
                }
            }
            if let Some(index) = next {
                self.open_modal_file(index);
            }
        }
        if matches!(event, NetworkEvent::Disconnected { .. }) {
            self.file_request = None;
            if let Some(modal) = &mut self.deck_modal {
                modal.file_request = None;
                modal.file_refresh = None;
                if modal.kind == ModalKind::Files {
                    modal.file_error = true;
                    modal.file_message = Some(
                        self.preferences
                            .language
                            .text(
                                "연결이 끊겼습니다. 다시 연결한 뒤 재시도하세요.",
                                "Disconnected. Reconnect, then try again.",
                            )
                            .to_owned(),
                    );
                }
            }
        }
        let Some(id) = self.deck_modal.as_ref().and_then(|modal| modal.submitted) else {
            return false;
        };
        if matches!(event, NetworkEvent::ApprovalCompleted { approval_id, result } if *approval_id == id && !result.as_ref().is_ok_and(|response| response.accepted))
            && let Some(modal) = &mut self.deck_modal
        {
            modal.submitted = None;
        }
        matches!(event, NetworkEvent::ApprovalCompleted { approval_id, result: Ok(response) } if *approval_id == id && response.accepted)
            || matches!(event, NetworkEvent::Server(envelope) if matches!(&envelope.event, orangedeck_protocol::ServerEvent::CodexApprovalResolved { approval_id } if *approval_id == id))
    }

    pub(super) fn announce_notifications(&mut self, ctx: &egui::Context) -> usize {
        let notices: Vec<_> = self.model.alerts.take_announcements().collect();
        let relevant: Vec<_> = notices
            .into_iter()
            .filter(|alert| {
                self.preferences.conversations.iter().any(|slot| {
                    !slot.thread_id.is_empty()
                        && alert.notification.thread_id.as_deref() == Some(&slot.thread_id)
                        && self
                            .model
                            .snapshot
                            .as_ref()
                            .and_then(|snapshot| {
                                snapshot
                                    .codex
                                    .threads
                                    .iter()
                                    .find(|thread| thread.id == slot.thread_id)
                            })
                            .is_some_and(|thread| crate::alerts::is_current(alert, thread))
                })
            })
            .collect();
        if relevant.is_empty() {
            return 0;
        }
        if self.preferences.notification_sound && !self.demo_mode {
            orangedeck_infra::play_notification_sound();
        }
        if self.config.desktop_notifications {
            for alert in &relevant {
                super::desktop_notification(&alert.notification.title, &alert.notification.body);
            }
        }
        if !ctx.input(|input| input.viewport().focused.unwrap_or(true)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }
        // Signal the paired keys. Never cover a user's current work with an automatic modal.
        relevant.len()
    }

    pub(super) fn render_deck_modal(&mut self, ctx: &egui::Context) {
        self.sync_file_modal();
        let Some(modal) = self.deck_modal.clone() else {
            return;
        };
        let lang = self.preferences.language;
        let id = self.current_approval_id();
        let approval = id
            .and_then(|id| {
                self.pending_for_modal()
                    .into_iter()
                    .find(|approval| approval.id == id)
            })
            .cloned();
        let previously_displayed = self.displayed_approval;
        let ready = self.model.approval_ready() && previously_displayed == id && id.is_some();
        let mut close = false;
        let mut decision = None;
        let mut file_clicked = None;
        let mut cycle = 0;
        let response = egui::Modal::new(egui::Id::new((
            "paired_modal",
            &modal.thread.id,
            &modal.turn_id,
            modal.kind == ModalKind::Files,
        )))
        .show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 70.0).clamp(300.0, 920.0));
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(&modal.label).size(21.0).strong()).truncate(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui
                        .add_sized(
                            [80.0, 36.0],
                            egui::Button::new(lang.text("닫기 ×", "Close ×")),
                        )
                        .clicked();
                });
            });
            ui.label(
                RichText::new(lang.text(
                    if modal.kind == ModalKind::Response {
                        "응답 · 이 질의의 내용"
                    } else {
                        "수정 파일 · 누르면 Mac 편집기로 이동"
                    },
                    if modal.kind == ModalKind::Response {
                        "Response · this question"
                    } else {
                        "Changed files · tap to open in your Mac editor"
                    },
                ))
                .color(theme::CYAN),
            );
            ui.separator();
            if modal.kind == ModalKind::Response {
                let height = (ctx.content_rect().height()
                    - if approval.is_some() { 375.0 } else { 155.0 })
                .clamp(70.0, 520.0);
                egui::ScrollArea::vertical()
                    .id_salt("response_body")
                    .max_height(height)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(lang.text("내 질의", "YOUR QUESTION"))
                                .small()
                                .color(theme::ORANGE),
                        );
                        ui.add(
                            egui::Label::new(selection::latest_prompt(&modal.thread).unwrap_or(
                                lang.text("질의를 받는 중…", "Receiving the question…"),
                            ))
                            .wrap(),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            RichText::new(lang.text("Codex 응답", "CODEX RESPONSE"))
                                .small()
                                .color(theme::CYAN),
                        );
                        let observation = modal.thread.observation.as_ref().filter(|observation| {
                            observation.turn_id.as_deref() == modal.turn_id.as_deref()
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    observation
                                        .and_then(|observation| {
                                            observation.latest_codex_reply.as_deref()
                                        })
                                        .unwrap_or(
                                            lang.text("응답을 받는 중…", "Receiving the response…"),
                                        ),
                                )
                                .size(18.0),
                            )
                            .wrap(),
                        );
                        if let Some(input) = modal
                            .thread
                            .activity
                            .as_ref()
                            .filter(|activity| Some(&activity.turn_id) == modal.turn_id.as_ref())
                            .and_then(|activity| activity.user_input.as_deref())
                        {
                            ui.separator();
                            ui.add(egui::Label::new(input).wrap());
                            ui.label(lang.text(
                                "이 질문의 답변은 Mac Codex에서 선택하세요.",
                                "Answer this question in Codex on your Mac.",
                            ));
                        }
                    });
                if let Some(approval) = &approval {
                    ui.separator();
                    let pending = self.pending_for_modal();
                    if pending.len() > 1 {
                        ui.horizontal(|ui| {
                            if ui.button("‹").clicked() {
                                cycle = -1;
                            }
                            let position = pending
                                .iter()
                                .position(|entry| entry.id == approval.id)
                                .unwrap_or(0)
                                + 1;
                            ui.label(format!("{position} / {}", pending.len()));
                            if ui.button("›").clicked() {
                                cycle = 1;
                            }
                        });
                    }
                    ui.push_id(approval.id, |ui| {
                        decision = notifications::approval_panel(
                            ui,
                            approval,
                            ready,
                            self.model.approvals_in_flight.contains(&approval.id),
                            true,
                        );
                    });
                    if let Some((error_id, message)) = &self.model.approval_error
                        && *error_id == approval.id
                    {
                        ui.colored_label(theme::RED, message);
                    }
                } else {
                    ui.separator();
                    ui.label(
                        RichText::new(lang.text(
                            "이 질의에 전달된 승인 요청이 없습니다.",
                            "No approval request has been received for this question.",
                        ))
                        .color(theme::MUTED),
                    );
                }
            } else {
                self.render_changed_files(
                    ui,
                    &modal,
                    &mut file_clicked,
                    ctx.content_rect().height(),
                );
            }
            ui.add_space(5.0);
            ui.label(
                RichText::new(lang.text("바깥을 누르거나 닫기 · X", "Tap outside or Close · X"))
                    .small()
                    .color(theme::MUTED),
            );
        });
        if let Some(current) = &mut self.deck_modal {
            current.just_opened = false;
        }
        self.displayed_approval = id;
        if close || (!modal.just_opened && response.should_close()) {
            self.close_deck_modal();
        } else if cycle != 0 {
            self.cycle_modal_approval(cycle < 0);
        } else if let Some(decision) = decision
            && let Some(id) = id
            && previously_displayed == Some(id)
            && self.current_approval_id() == Some(id)
        {
            self.submit_approval(id, decision);
        } else if let Some(index) = file_clicked {
            self.open_modal_file(index);
        }
    }

    fn render_changed_files(
        &self,
        ui: &mut egui::Ui,
        modal: &DeckModal,
        clicked: &mut Option<usize>,
        screen_height: f32,
    ) {
        let lang = self.preferences.language;
        let changes = modal
            .thread
            .observation
            .as_ref()
            .and_then(|observation| observation.changes.as_ref());
        if changes.is_none_or(|changes| changes.files.is_empty()) {
            theme::accent_panel(theme::YELLOW).show(ui, |ui| {
                ui.label(
                    RichText::new(lang.text("파일 정보 확인", "File records"))
                        .size(22.0)
                        .strong(),
                );
                ui.add(
                    egui::Label::new(modal.file_message.as_deref().unwrap_or(lang.text(
                        "파일 기록을 기다리고 있습니다.",
                        "Waiting for file records.",
                    )))
                    .wrap(),
                );
                if modal.file_refresh.is_some() {
                    ui.spinner();
                }
                if ui
                    .add_enabled(
                        modal.file_refresh.is_none() && self.model.file_navigation_ready(),
                        egui::Button::new(lang.text("다시 확인", "Check again"))
                            .min_size(egui::vec2(130.0, 42.0)),
                    )
                    .clicked()
                {
                    *clicked = Some(0);
                }
            });
            return;
        }
        let changes = changes.unwrap();
        if changes.truncated {
            ui.colored_label(
                theme::YELLOW,
                lang.text(
                    "일부 파일 기록이나 변경 내용이 빠져 있을 수 있습니다. 전체 내용은 Mac에서 확인하세요.",
                    "Some file records or change details may be missing. Review the full files on your Mac.",
                ),
            );
        }
        let status_start = ui.cursor().top();
        if modal.file_message.is_some() || !self.model.connected {
            let error = modal.file_error || !self.model.connected;
            let color = if error { theme::YELLOW } else { theme::CYAN };
            egui::Frame::new().fill(color.gamma_multiply(0.08))
                .stroke(egui::Stroke::new(1.0, color)).inner_margin(10.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(RichText::new(lang.text(
                        if modal.connector_update_needed { "Mac 통신 모듈 업데이트 필요" } else if error { "파일을 열지 못했습니다" } else { "Mac 편집기" },
                        if modal.connector_update_needed { "Update your Mac Connector" } else if error { "Could not open the file" } else { "Mac editor" }
                    )).size(18.0).strong().color(color));
                    egui::ScrollArea::vertical().id_salt("editor_result_notice").max_height(80.0).show(ui, |ui| {
                        let message = if self.model.connected {
                            if modal.connector_update_needed {
                            lang.text(
                                "Mac 통신 모듈을 0.1.26 이상으로 업데이트하세요. 대화의 작업 폴더를 자동으로 사용하므로 폴더를 따로 등록할 필요가 없습니다.",
                                "Update the Mac Connector to 0.1.26 or later. It uses the conversation's working folder automatically; no folder registration is needed.")
                            } else {
                                modal.file_message.as_deref().unwrap_or("")
                            }
                        } else {
                            lang.text("연결이 끊겼습니다. 다시 연결한 뒤 재시도하세요.", "Disconnected. Reconnect, then try again.")
                        };
                        ui.add(egui::Label::new(RichText::new(message).size(16.0)).wrap());
                    });
                    if error && ui.add_enabled(self.model.file_navigation_ready() && self.file_request.is_none(),
                        egui::Button::new(lang.text("다시 시도", "Try again")).min_size(egui::vec2(120.0, 38.0))).clicked() {
                        *clicked = Some(modal.selected_file);
                    }
                });
            ui.add_space(8.0);
        }
        let status_height = ui.cursor().top() - status_start;
        let height = (screen_height - 230.0 - status_height).clamp(60.0, 430.0);
        ui.columns(2, |columns| {
            egui::ScrollArea::vertical()
                .id_salt("changed_file_list")
                .max_height(height)
                .show(&mut columns[0], |ui| {
                    for (index, file) in changes.files.iter().enumerate() {
                        let kind = change_label(file.kind, lang);
                        let label = format!("{kind} · {}\n{}", file.first_line, file.path);
                        let response = ui.add_sized(
                            [ui.available_width(), 64.0],
                            egui::Button::new(RichText::new(label).size(13.0))
                                .wrap()
                                .selected(index == modal.selected_file),
                        );
                        if response.clicked() {
                            *clicked = Some(index);
                        }
                    }
                });
            egui::ScrollArea::both()
                .id_salt("changed_file_diff")
                .max_height(height)
                .show(&mut columns[1], |ui| {
                    if let Some(file) = changes.files.get(modal.selected_file) {
                        ui.add(egui::Label::new(RichText::new(&file.path).strong()).wrap());
                        if let Some(previous) = &file.previous_path {
                            ui.add(egui::Label::new(format!("{previous} → {}", file.path)).wrap());
                        }
                        if file.kind == CodeChangeKindDto::Deleted {
                            ui.label(lang.text(
                                "삭제된 파일 · Mac에서 열 수 없습니다",
                                "Deleted file · cannot open on Mac",
                            ));
                        }
                        for line in file.diff.lines() {
                            let color = if line.starts_with('+') {
                                theme::GREEN
                            } else if line.starts_with('-') {
                                theme::PINK
                            } else {
                                theme::TEXT
                            };
                            ui.label(RichText::new(line).monospace().size(12.0).color(color));
                        }
                        if file.truncated {
                            ui.colored_label(
                                theme::YELLOW,
                                lang.text("… 변경 내용 일부 생략", "… diff shortened"),
                            );
                        }
                    }
                });
        });
    }
}

fn change_label(kind: CodeChangeKindDto, lang: super::Language) -> &'static str {
    match kind {
        CodeChangeKindDto::Added => lang.text("추가", "Added"),
        CodeChangeKindDto::Modified => lang.text("수정", "Modified"),
        CodeChangeKindDto::Deleted => lang.text("삭제", "Deleted"),
        CodeChangeKindDto::Renamed => lang.text("이동", "Renamed"),
    }
}
