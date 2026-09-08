use super::*;
use orangedeck_protocol::ApprovalDto;

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
fn navigation_has_four_tabs_with_agents_second() {
    assert_eq!(
        Page::ALL.map(Page::label),
        ["LIVE", "에이전트들", "프로젝트들", "대화"]
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
    frame_with_size(app, ctx, events, egui::vec2(960.0, 600.0))
}

fn frame_with_size(
    app: &mut OrangeDeckApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    size: egui::Vec2,
) -> Painted {
    let snapshot = app.model.snapshot.as_ref().unwrap().clone();
    let mut keys = [egui::Rect::NOTHING; 10];
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
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
                    app.render_agents(ui, &snapshot);
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
fn confirmed_no_edits_is_inert_but_loading_opens_a_recovery_modal() {
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
    app.page = Page::Agents;
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
    assert!(app.deck_modal.is_some());
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::CodexReadThread { thread_id } if thread_id == "a")
    );
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
fn files_open_directly_and_editor_failures_offer_retry_without_folder_registration() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    let ClientCommand::OpenCodexChange {
        navigation_id,
        thread_id,
        turn_id,
        path,
    } = commands.try_recv().unwrap()
    else {
        panic!("direct file open expected")
    };
    assert_eq!(
        (thread_id.as_str(), turn_id.as_str(), path.as_str()),
        ("a", "new", "src/first.rs")
    );
    event(
        &mut app,
        crate::model::NetworkEvent::FileOpenCompleted {
            navigation_id,
            result: Err("editor_unavailable: 편집기 확인 / Check editor".into()),
        },
    );
    assert!(
        commands.try_recv().is_err(),
        "no automatic registration or retries"
    );
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    assert!(painted.label("편집기 확인").y < painted.label("src/first.rs").y);
    assert!(
        !painted
            .labels
            .iter()
            .any(|(text, _)| text.contains("폴더 등록"))
    );
    // The retry remains bound to the displayed conversation and turn.
    app.model.snapshot.as_mut().unwrap().codex.threads[0].cwd = "/other/project".into();
    click(&mut app, &ctx, painted.label("다시 시도"));
    let ClientCommand::OpenCodexChange {
        navigation_id: retried,
        thread_id,
        turn_id,
        path,
    } = commands.try_recv().unwrap()
    else {
        panic!("file retry expected")
    };
    assert_eq!(
        (thread_id.as_str(), turn_id.as_str(), path.as_str()),
        ("a", "new", "src/first.rs")
    );
    event(
        &mut app,
        crate::model::NetworkEvent::FileOpenCompleted {
            navigation_id: retried,
            result: Ok(outcome(true)),
        },
    );
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]).label("Mac 편집기로 파일 열기를 보냈습니다.");
    assert!(commands.try_recv().is_err());
}

#[test]
fn folder_errors_and_close_controls_remain_visible_on_small_korean_and_english_screens() {
    for size in [egui::vec2(820.0, 480.0), egui::vec2(1038.0, 584.0)] {
        for language in [Language::Korean, Language::English] {
            let mut snapshot = files_snapshot();
            snapshot.codex.threads[0].cwd =
                format!("/mock/{}/project", "long-folder-name/".repeat(12));
            let (mut app, mut commands) = test_app(&snapshot);
            app.preferences.language = language;
            bind(&mut app, 0, "a");
            app.activate_pair(5);
            let ClientCommand::OpenCodexChange { navigation_id, .. } = commands.try_recv().unwrap()
            else {
                panic!("open expected")
            };
            event(
                &mut app,
                crate::model::NetworkEvent::FileOpenCompleted {
                    navigation_id,
                    result: Err(format!(
                        "editor_workspace_unavailable: 작업 폴더를 찾을 수 없습니다: {} / Working folder is unavailable: {}",
                        snapshot.codex.threads[0].cwd, snapshot.codex.threads[0].cwd
                    )),
                },
            );
            let ctx = context();
            for _ in 0..3 {
                frame_with_size(&mut app, &ctx, vec![], size);
            }
            let painted = frame_with_size(&mut app, &ctx, vec![], size);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            for label in [
                language.text("파일을 열지 못했습니다", "Could not open the file"),
                language.text("다시 시도", "Try again"),
                language.text("닫기 ×", "Close ×"),
            ] {
                let (_, rect) = painted
                    .labels
                    .iter()
                    .find(|(text, _)| text == label)
                    .unwrap();
                assert!(
                    screen.contains_rect(*rect),
                    "{label} outside {screen:?}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn old_connector_explains_the_update_and_missing_approvals_do_not_create_choices() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    let ctx = context();
    for message in [
        "file_not_allowed: Mac에 등록한 프로젝트의 수정 파일만 열 수 있습니다 / Register this project on your Mac first",
        "editor_project_not_registered: This folder is not registered",
    ] {
        app.activate_pair(5);
        let ClientCommand::OpenCodexChange { navigation_id, .. } = commands.try_recv().unwrap()
        else {
            panic!("open expected")
        };
        event(
            &mut app,
            crate::model::NetworkEvent::FileOpenCompleted {
                navigation_id,
                result: Err(message.into()),
            },
        );
        frame(&mut app, &ctx, vec![]);
        let painted = frame(&mut app, &ctx, vec![]);
        painted.label("0.1.26 이상으로 업데이트");
        assert!(
            !painted
                .labels
                .iter()
                .any(|(text, _)| text == "이 폴더 등록하고 파일 열기")
        );
        assert!(commands.try_recv().is_err());
        app.close_deck_modal();
    }
    app.activate_pair(0);
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    painted.label("이 질의에 전달된 승인 요청이 없습니다.");
    assert!(!painted.labels.iter().any(|(text, _)| text == "A  승인"));
    app.model
        .snapshot
        .as_mut()
        .unwrap()
        .codex
        .pending_approvals
        .push(request("a", "new"));
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]).label("A  승인");
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

#[test]
fn lower_key_click_opens_files_and_sends_the_exact_recorded_path() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, painted.keys[5].center());
    assert_eq!(
        app.deck_modal.as_ref().unwrap().kind,
        paired_deck::ModalKind::Files
    );
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::OpenCodexChange { thread_id, turn_id, path, .. } if thread_id == "a" && turn_id == "new" && path == "src/first.rs")
    );
    frame(&mut app, &ctx, vec![]).label("src/first.rs");
}

#[test]
fn missing_records_are_clickable_and_fresh_matching_records_open_the_file() {
    let mut snapshot = files_snapshot();
    let fresh = snapshot.codex.threads[0].clone();
    snapshot.codex.threads[0]
        .observation
        .as_mut()
        .unwrap()
        .changes = None;
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, painted.keys[5].center());
    assert!(app.deck_modal.is_some());
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::CodexReadThread { thread_id } if thread_id == "a")
    );
    frame(&mut app, &ctx, vec![]).label("파일 정보 확인");
    let mut updated = fresh;
    updated.observation.as_mut().unwrap().observed_at += chrono::Duration::seconds(1);
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(updated),
        )),
    );
    frame(&mut app, &ctx, vec![]);
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::OpenCodexChange { thread_id, turn_id, path, .. } if thread_id == "a" && turn_id == "new" && path == "src/first.rs")
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn cached_file_modal_still_opens_with_codex_or_transport_disconnected() {
    for transport in [true, false] {
        let mut snapshot = files_snapshot();
        snapshot.codex.connection.state = orangedeck_protocol::CodexConnectionStateDto::Error;
        let (mut app, mut commands) = test_app(&snapshot);
        bind(&mut app, 0, "a");
        if !transport {
            event(
                &mut app,
                crate::model::NetworkEvent::Disconnected {
                    message: "fixture".into(),
                    retry_ms: 1000,
                },
            );
        }
        app.activate_pair(5);
        assert!(app.deck_modal.is_some());
        if transport {
            assert!(matches!(
                commands.try_recv().unwrap(),
                ClientCommand::OpenCodexChange { .. }
            ));
        } else {
            assert!(commands.try_recv().is_err());
            assert!(app.deck_modal.as_ref().unwrap().file_error);
        }
    }
}

#[test]
fn old_notification_entry_points_open_agents_without_any_approval() {
    let snapshot = files_snapshot();
    let (mut app, mut commands) = test_app(&snapshot);
    app.handle_action(ControlAction::Detail);
    assert_eq!(app.page, Page::Agents);
    assert!(commands.try_recv().is_err());
}

#[test]
fn file_refresh_cannot_open_a_later_turn_or_a_closed_modal() {
    for close in [false, true] {
        let mut snapshot = files_snapshot();
        let mut fresh = snapshot.codex.threads[0].clone();
        snapshot.codex.threads[0]
            .observation
            .as_mut()
            .unwrap()
            .changes = None;
        let (mut app, mut commands) = test_app(&snapshot);
        bind(&mut app, 0, "a");
        app.activate_pair(5);
        assert!(matches!(
            commands.try_recv().unwrap(),
            ClientCommand::CodexReadThread { .. }
        ));
        if close {
            app.handle_action(ControlAction::Back);
        } else {
            fresh.observation.as_mut().unwrap().turn_id = Some("later".into());
        }
        fresh.observation.as_mut().unwrap().observed_at += chrono::Duration::seconds(1);
        event(
            &mut app,
            crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
                orangedeck_protocol::ServerEvent::CodexThreadUpdated(fresh),
            )),
        );
        frame(&mut app, &context(), vec![]);
        assert!(commands.try_recv().is_err());
        if close {
            assert!(app.deck_modal.is_none());
        } else {
            assert!(app.deck_modal.as_ref().unwrap().file_error);
        }
    }
}

#[test]
fn live_file_capture_updates_the_waiting_dialog_and_opens_the_recorded_file_once() {
    let mut snapshot = files_snapshot();
    let thread = &mut snapshot.codex.threads[0];
    thread.status = CodexThreadStatusDto::Working;
    thread.activity = None;
    let observation = thread.observation.as_mut().unwrap();
    observation.last_turn_status = CodexThreadStatusDto::Working;
    thread.active_turn_id = observation.turn_id.clone();
    let mut captured = thread.clone();
    let changes = captured
        .observation
        .as_mut()
        .unwrap()
        .changes
        .as_mut()
        .unwrap();
    changes.files.truncate(1);
    changes.files[0].path = "codex_approval_test.py".to_owned();
    changes.files[0].diff = "@@ -2 +2 @@\n-before\n+after\n".to_owned();
    thread.observation.as_mut().unwrap().changes = Some(orangedeck_protocol::TurnChangesDto {
        files: Vec::new(),
        truncated: true,
    });
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    assert!(matches!(
        commands.try_recv().unwrap(),
        ClientCommand::CodexReadThread { .. }
    ));
    assert_eq!(
        paired_deck::file_state(&app.deck_modal.as_ref().unwrap().thread),
        crate::paired::FileState::Loading
    );
    // Tool changes can arrive without a new history-read timestamp.
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(captured.clone()),
        )),
    );
    let ctx = context();
    // egui places a new modal on its first frame and paints it on the next.
    frame(&mut app, &ctx, vec![]);
    let painted = frame(&mut app, &ctx, vec![]);
    assert!(
        painted
            .labels
            .iter()
            .any(|(text, _)| text.contains("codex_approval_test.py")),
        "painted labels: {:?}",
        painted.labels
    );
    assert!(
        matches!(commands.try_recv().unwrap(), ClientCommand::OpenCodexChange { path, .. } if path == "codex_approval_test.py")
    );
    assert!(commands.try_recv().is_err());
    assert!(app.deck_modal.as_ref().unwrap().file_refresh.is_none());

    let changes = captured
        .observation
        .as_mut()
        .unwrap()
        .changes
        .as_mut()
        .unwrap();
    let mut second = changes.files[0].clone();
    second.path = "later-created.py".to_owned();
    changes.files.insert(0, second);
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(captured),
        )),
    );
    frame(&mut app, &ctx, vec![]);
    let modal = app.deck_modal.as_ref().unwrap();
    assert_eq!(
        paired_deck::file_state(&modal.thread),
        crate::paired::FileState::Changes(2)
    );
    assert_eq!(
        modal.selected_file, 1,
        "selection follows the same path after reordering"
    );
    assert!(
        commands.try_recv().is_err(),
        "new file data must not launch the editor again"
    );
}

#[test]
fn late_capture_recovers_an_open_incomplete_dialog_but_cannot_replace_it_with_another_turn() {
    let mut snapshot = files_snapshot();
    let mut captured = snapshot.codex.threads[0].clone();
    snapshot.codex.threads[0]
        .observation
        .as_mut()
        .unwrap()
        .changes = None;
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    assert!(matches!(
        commands.try_recv().unwrap(),
        ClientCommand::CodexReadThread { .. }
    ));
    let modal = app.deck_modal.as_mut().unwrap();
    modal.file_refresh = None;
    modal.file_error = true;
    modal.file_message = Some("incomplete record".to_owned());
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(captured.clone()),
        )),
    );
    let ctx = context();
    frame(&mut app, &ctx, vec![]);
    assert!(!app.deck_modal.as_ref().unwrap().file_error);
    assert!(app.deck_modal.as_ref().unwrap().file_message.is_none());
    assert_eq!(
        paired_deck::file_state(&app.deck_modal.as_ref().unwrap().thread),
        crate::paired::FileState::Changes(2)
    );
    assert!(commands.try_recv().is_err());
    captured.active_turn_id = Some("another-turn".to_owned());
    let observation = captured.observation.as_mut().unwrap();
    observation.turn_id = Some("another-turn".to_owned());
    observation.observed_at += chrono::Duration::seconds(1);
    observation.changes.as_mut().unwrap().files[0].path = "wrong-turn.py".to_owned();
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(captured),
        )),
    );
    frame(&mut app, &ctx, vec![]);
    assert_ne!(
        app.deck_modal.as_ref().unwrap().turn_id.as_deref(),
        Some("another-turn")
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn recovered_deletions_show_their_diff_without_claiming_no_edits_or_opening_editor() {
    let mut snapshot = files_snapshot();
    let mut fresh = snapshot.codex.threads[0].clone();
    let observation = fresh.observation.as_mut().unwrap();
    observation.observed_at += chrono::Duration::seconds(1);
    for file in &mut observation.changes.as_mut().unwrap().files {
        file.kind = orangedeck_protocol::CodeChangeKindDto::Deleted;
    }
    snapshot.codex.threads[0]
        .observation
        .as_mut()
        .unwrap()
        .changes = None;
    let (mut app, mut commands) = test_app(&snapshot);
    bind(&mut app, 0, "a");
    app.activate_pair(5);
    assert!(matches!(
        commands.try_recv().unwrap(),
        ClientCommand::CodexReadThread { .. }
    ));
    event(
        &mut app,
        crate::model::NetworkEvent::Server(orangedeck_protocol::ServerEnvelope::new(
            orangedeck_protocol::ServerEvent::CodexThreadUpdated(fresh),
        )),
    );
    frame(&mut app, &context(), vec![]);
    let modal = app.deck_modal.as_ref().unwrap();
    assert_eq!(
        paired_deck::file_state(&modal.thread),
        crate::paired::FileState::Changes(2)
    );
    assert!(modal.file_message.is_none());
    assert!(!modal.file_error);
    assert!(commands.try_recv().is_err());
}
