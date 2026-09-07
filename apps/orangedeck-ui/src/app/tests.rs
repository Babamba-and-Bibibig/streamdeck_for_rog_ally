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
        deck_modal: None,
        watched_deck: None,
        file_request: None,
    };
    (app, commands)
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
fn shortcuts_are_second_and_notifications_are_fifth() {
    assert_eq!(
        Page::ALL.map(Page::label),
        ["LIVE", "단축키", "프로젝트들", "대화", "알림"]
    );
}

fn bind(app: &mut OrangeDeckApp, column: usize, id: &str) {
    app.preferences.conversations[column] = orangedeck_domain::ConversationSlot {
        thread_id: id.to_owned(),
        label: format!("Terminal {}", column + 1),
    };
}

fn files_snapshot() -> SnapshotDto {
    let mut snapshot = crate::test_support::snapshot();
    snapshot
        .codex
        .supported_features
        .push("paired_conversations".to_owned());
    let thread = &mut snapshot.codex.threads[0];
    thread.status = CodexThreadStatusDto::Completed;
    thread.live_usage = None;
    let observation = thread.observation.as_mut().unwrap();
    observation.last_turn_status = CodexThreadStatusDto::Completed;
    observation.changes = Some(orangedeck_protocol::TurnChangesDto {
        files: ["src/first.rs", "src/second.rs"]
            .iter()
            .enumerate()
            .map(|(index, path)| orangedeck_protocol::CodeChangeDto {
                path: (*path).to_owned(),
                previous_path: None,
                kind: orangedeck_protocol::CodeChangeKindDto::Modified,
                first_line: u32::try_from(index + 10).unwrap(),
                diff: "@@ -10 +10 @@\n-old\n+new".to_owned(),
                truncated: false,
            })
            .collect(),
        truncated: false,
    });
    snapshot
}

fn request(thread: &str, turn: &str) -> ApprovalDto {
    ApprovalDto {
        id: uuid::Uuid::new_v4(),
        thread_id: Some(thread.to_owned()),
        turn_id: Some(turn.to_owned()),
        kind: orangedeck_protocol::ApprovalKindDto::CommandExecution,
        title: "Test approval".to_owned(),
        summary: "cargo check --offline".to_owned(),
        details: vec!["Synthetic request only".to_owned()],
        requested_at: chrono::Utc::now(),
    }
}

fn outcome(accepted: bool) -> orangedeck_protocol::CommandResponse {
    orangedeck_protocol::CommandResponse {
        protocol_version: orangedeck_protocol::PROTOCOL_VERSION,
        request_id: uuid::Uuid::new_v4(),
        accepted,
        message: "Test delivery result".to_owned(),
        job_id: None,
    }
}

fn event(app: &mut OrangeDeckApp, event: crate::model::NetworkEvent) {
    let close = app.process_deck_network(&event);
    app.model.apply_network(event);
    if close {
        app.close_deck_modal();
    }
}

struct Painted {
    labels: Vec<(String, egui::Rect)>,
    keys: [egui::Rect; 10],
}
impl Painted {
    fn label(&self, text: &str) -> egui::Pos2 {
        self.labels
            .iter()
            .find(|(value, _)| value.contains(text))
            .unwrap_or_else(|| panic!("missing {text}: {:?}", self.labels))
            .1
            .center()
    }
}

fn frame(app: &mut OrangeDeckApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Painted {
    let snapshot = app.model.snapshot.as_ref().unwrap().clone();
    let mut keys = [egui::Rect::NOTHING; 10];
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 600.0),
            )),
            events,
            time: Some(f64::from(u32::try_from(ctx.cumulative_frame_nr()).unwrap_or(0)) / 10.0),
            ..Default::default()
        },
        |root| {
            let pending = app.current_approval_id().is_some();
            for action in app.controller.poll(ctx, true, pending) {
                app.handle_action(action);
            }
            if app.deck_modal.is_some() || app.editor.is_some() {
                root.disable();
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    keys = crate::paired::key_rects(ui.available_rect_before_wrap());
                    app.render_shortcuts(ui, &snapshot);
                });
            app.render_deck_modal(ctx);
            app.render_key_editor(ctx);
        },
    );
    let labels = output
        .shapes
        .iter()
        .filter_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape {
                Some((
                    text.galley.job.text.clone(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                ))
            } else {
                None
            }
        })
        .collect();
    output.drop_without_applying_deltas();
    Painted { labels, keys }
}

fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
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
fn click(app: &mut OrangeDeckApp, ctx: &egui::Context, pos: egui::Pos2) {
    frame(app, ctx, pointer(pos, true));
    frame(app, ctx, pointer(pos, false));
}
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    ctx
}

#[test]
fn no_edits_is_inert_for_touch_and_controller_and_not_confused_with_loading() {
    let mut snapshot = files_snapshot();
    snapshot.codex.threads[0]
        .observation
        .as_mut()
        .unwrap()
        .changes
        .as_mut()
        .unwrap()
        .files
        .clear();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.page = Page::Shortcuts;
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    assert!(
        painted
            .labels
            .iter()
            .any(|(text, _)| text == "파일 수정 없음")
    );
    click(&mut app, &ctx, painted.keys[5].center());
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
    app.focus_index = 5;
    app.handle_action(ControlAction::Activate);
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
    app.model.snapshot.as_mut().unwrap().codex.threads[0]
        .observation
        .as_mut()
        .unwrap()
        .turn_id = Some("previous-query".to_owned());
    assert_eq!(
        app.pair_views(app.model.snapshot.as_ref().unwrap())[0].files,
        crate::paired::FileState::Loading
    );
    app.activate_pair(5);
    assert!(app.deck_modal.is_none());
}

#[test]
fn columns_stay_bound_across_project_selection_and_most_recent_thread_changes() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "b");
    bind(&mut app, 4, "a");
    app.select_project("/Users/mac/project-b");
    app.activate_pair(4);
    assert_eq!(app.deck_modal.as_ref().unwrap().thread.id, "a");
    app.close_deck_modal();
    app.update_deck_watches();
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::CodexWatchThreads { thread_ids } if thread_ids == ["a", "b"])
    );
    app.model.snapshot.as_mut().unwrap().codex.threads.reverse();
    app.update_deck_watches();
    assert!(commands.try_recv().is_err());
    assert_eq!(app.preferences.conversations[4].thread_id, "a");
}

#[test]
fn response_and_file_modals_have_distinct_actions_and_outside_tap_never_opens_a_background_key() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    bind(&mut app, 1, "b");
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    app.activate_pair(0);
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    assert!(
        painted
            .labels
            .iter()
            .any(|(text, _)| text == "CURRENT ANSWER")
    );
    assert!(
        !painted
            .labels
            .iter()
            .any(|(text, _)| text.contains("A  승인"))
    );
    assert!(commands.try_recv().is_err());
    let underlying_key = egui::pos2(painted.keys[0].left() + 2.0, painted.keys[0].center().y);
    assert!(painted.keys[0].contains(underlying_key));
    click(&mut app, &ctx, underlying_key);
    assert!(app.deck_modal.is_none());
    assert!(app.editor.is_none());
    assert!(commands.try_recv().is_err());
    app.activate_pair(5);
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::OpenCodexChange { path, thread_id, turn_id, .. } if path == "src/first.rs" && thread_id == "a" && turn_id == "new")
    );
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    assert!(
        painted
            .labels
            .iter()
            .any(|(text, _)| text.contains("src/second.rs"))
    );
    assert!(
        !painted
            .labels
            .iter()
            .any(|(text, _)| text.contains("A  승인") || text.contains("B  거부"))
    );
    click(&mut app, &ctx, painted.label("닫기 ×"));
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
}

#[test]
fn choosing_a_conversation_stays_stable_while_other_terminals_update() {
    let mut snapshot = files_snapshot();
    snapshot.codex.threads[0].updated_at = 100;
    snapshot.codex.threads[1].updated_at = 90;
    let (mut app, mut commands) = test_app(&snapshot);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, painted.keys[0].center());
    assert!(
        app.editor.is_some(),
        "opening touch must not immediately dismiss the picker"
    );
    let painted = frame(&mut app, &ctx, vec![]);
    let target = painted.label(&snapshot.codex.threads[0].cwd);
    frame(&mut app, &ctx, pointer(target, true));
    app.model.snapshot.as_mut().unwrap().codex.threads[1].updated_at = 1_000;
    frame(&mut app, &ctx, pointer(target, false));
    assert!(app.editor.is_none());
    assert_eq!(app.preferences.conversations[0].thread_id, "a");
    assert!(
        commands.try_recv().is_err(),
        "assigning does not execute a command"
    );
    app.open_key_editor(0);
    frame(&mut app, &ctx, vec![]);
    app.model.snapshot.as_mut().unwrap().codex.threads[0].updated_at = 2_000;
    app.handle_action(ControlAction::Activate);
    assert_eq!(app.preferences.conversations[0].thread_id, "a");
    assert!(commands.try_recv().is_err());
}

#[test]
fn file_clicks_keep_the_latest_selection_in_order_and_ignore_late_results_after_close() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    let ClientCommand::OpenCodexChange {
        navigation_id: first,
        ..
    } = commands.try_recv().unwrap()
    else {
        panic!("file navigation expected")
    };
    app.handle_action(ControlAction::NavigateDown);
    app.handle_action(ControlAction::Activate);
    assert!(
        commands.try_recv().is_err(),
        "second file stays queued until the first editor request returns"
    );
    event(
        &mut app,
        crate::model::NetworkEvent::FileOpenCompleted {
            navigation_id: first,
            result: Ok(outcome(true)),
        },
    );
    let ClientCommand::OpenCodexChange {
        navigation_id: second,
        path,
        ..
    } = commands.try_recv().unwrap()
    else {
        panic!("queued navigation expected")
    };
    assert_eq!(path, "src/second.rs");
    assert_ne!(first, second);
    app.close_deck_modal();
    event(
        &mut app,
        crate::model::NetworkEvent::FileOpenCompleted {
            navigation_id: second,
            result: Ok(outcome(true)),
        },
    );
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
}

#[test]
fn modal_approval_requires_a_rendered_request_locks_duplicates_and_closes_only_on_delivery() {
    for decision in [ApprovalDecisionDto::Approve, ApprovalDecisionDto::Reject] {
        let mut snapshot = files_snapshot();
        let approval = request("a", "new");
        snapshot.codex.pending_approvals =
            vec![request("b", "other"), request("a", "old"), approval.clone()];
        let (mut app, mut commands) = test_app(&snapshot);
        bind(&mut app, 0, "a");
        app.activate_pair(0);
        app.handle_action(ControlAction::Activate);
        assert!(
            commands.try_recv().is_err(),
            "opening cannot approve before rendering"
        );
        let ctx = context();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        app.handle_action(if decision == ApprovalDecisionDto::Approve {
            ControlAction::Activate
        } else {
            ControlAction::Back
        });
        assert!(
            matches!(commands.try_recv().unwrap(), ClientCommand::CodexApprovalResponse { approval_id, decision: actual } if approval_id == approval.id && actual == decision)
        );
        assert!(
            app.deck_modal.is_some(),
            "wait for explicit host acknowledgement"
        );
        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_err());
        event(
            &mut app,
            crate::model::NetworkEvent::ApprovalCompleted {
                approval_id: approval.id,
                result: Ok(outcome(false)),
            },
        );
        assert!(
            app.deck_modal.is_some(),
            "delivery failure keeps the modal and its error visible"
        );
        app.handle_action(ControlAction::Activate);
        assert!(commands.try_recv().is_ok());
        event(
            &mut app,
            crate::model::NetworkEvent::ApprovalCompleted {
                approval_id: approval.id,
                result: Ok(outcome(true)),
            },
        );
        assert!(app.deck_modal.is_none());
    }
}

#[test]
fn held_pointer_cannot_approve_a_replacement_and_escape_only_dismisses() {
    let mut snapshot = files_snapshot();
    let original = request("a", "new");
    snapshot.codex.pending_approvals.push(original);
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(0);
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    let pos = painted.label("A  승인");
    frame(&mut app, &ctx, pointer(pos, true));
    let replacement = request("a", "new");
    app.model.snapshot.as_mut().unwrap().codex.pending_approvals = vec![replacement.clone()];
    frame(&mut app, &ctx, pointer(pos, false));
    assert!(commands.try_recv().is_err());
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, pos);
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::CodexApprovalResponse { approval_id, .. } if approval_id == replacement.id)
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
}

#[test]
fn file_modal_and_conversation_picker_cannot_decide_hidden_approvals() {
    let mut snapshot = files_snapshot();
    snapshot.codex.pending_approvals.push(request("a", "new"));
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    assert!(matches!(
        commands.try_recv().unwrap(),
        ClientCommand::OpenCodexChange { .. }
    ));
    app.handle_action(ControlAction::Back);
    assert!(app.deck_modal.is_none());
    assert!(commands.try_recv().is_err());
    app.open_key_editor(2);
    app.handle_action(ControlAction::Activate);
    assert!(commands.try_recv().is_err());
    assert!(app.current_approval_id().is_none());
}

#[test]
fn notices_light_only_the_assigned_pair_without_opening_any_modal_or_changing_page() {
    let snapshot = files_snapshot();
    let (mut app, _) = test_app(&snapshot);
    bind(&mut app, 3, "a");
    app.config.desktop_notifications = false;
    let notice = orangedeck_protocol::NotificationDto {
        id: Some(uuid::Uuid::new_v4()),
        thread_id: Some("a".to_owned()),
        turn_id: Some("new".to_owned()),
        created_at: Some(chrono::Utc::now()),
        level: orangedeck_protocol::NotificationLevelDto::Success,
        title: "Done".to_owned(),
        body: "Response".to_owned(),
    };
    let id = notice.id.unwrap();
    app.model
        .alerts
        .record(notice.clone(), id, chrono::Utc::now(), true, false);
    assert_eq!(app.announce_notifications(&context()), 1);
    let pairs = app.pair_views(app.model.snapshot.as_ref().unwrap());
    assert_eq!(
        pairs.map(|pair| pair.attention),
        [false, false, false, true, false]
    );
    assert!(app.deck_modal.is_none());
    assert_eq!(app.page, Page::Dashboard);
    app.model
        .alerts
        .record(notice, id, chrono::Utc::now(), true, false);
    assert_eq!(
        app.announce_notifications(&context()),
        0,
        "duplicate snapshots/events do not replay sounds"
    );
    app.activate_pair(3);
    assert!(!app.pair_views(app.model.snapshot.as_ref().unwrap())[3].attention);
}
