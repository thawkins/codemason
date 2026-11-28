//! Tests for the remaining GUI dialogs (Task 3.3b): the agent's question,
//! plan approval, the post-loop rollback offer, force-cleanup, the router
//! save, notice and `/db prune` confirmations, the `/loop` setup dialog, the
//! project properties dialog, the session menu, the input history and config
//! save pickers, and the run-cost banner.
//!
//! Every test drives the whole [`Shell`] frame, so the dialog dispatch, the
//! key router and app-core's key handling are exercised together. GUI
//! assertions use egui_kittest accessibility queries only (no snapshots).
//! Each test owns a tokio runtime that it does not enter while the harness
//! runs, because the shell drives `App::handle_key` with `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::event::Event;
use codemason_app_core::app::{
    App, ConfigSavePickerState, ContextAction, HistoryPickerState, LoopSetupField,
    PendingForceCleanup, PlanApprovalState, QUESTION_ANSWER_DELAY, RollbackOfferState,
    SelectionPane,
};
use codemason_app_core::geometry::Area;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::project_properties::{Choice, Field, ProjectProperties};
use codemason_app_core::session_menu::{MenuAction, MenuItem, MenuTarget, SessionMenu};
use codemason_gui::Shell;
use codemason_gui::dialogs::banner::{BANNER_LABEL, DISMISS_BANNER};
use codemason_gui::dialogs::confirm::{
    APPROVE, COMPACT_DELETE, COMPACT_PRUNE_TITLE, FORCE_CLEANUP_TITLE, KEEP_CHANGES, NO, NOTICE_OK,
    PLAN_TITLE, PRUNE_DELETE, ROLLBACK_TITLE, ROUTER_SAVE_TITLE, db_prune_title,
};
use codemason_gui::dialogs::context_menu::{CONTEXT_MENU_LABEL, item_label};
use codemason_gui::dialogs::loop_setup::{AGENTS_LABEL, CHECKPOINTS_LABEL, LOOP_TITLE, START};
use codemason_gui::dialogs::pickers::{
    APPLY_THEME, CANCEL_THEME, CONFIG_SAVES_TITLE, ENTRIES_LABEL, HISTORY_TITLE, SELECT_ENTRY,
    THEME_TITLE,
};
use codemason_gui::dialogs::properties::{
    BACKEND_LABEL, CANCEL, CHOICES_LABEL, DESCRIPTION_LABEL, MODEL_LABEL, SAVE, properties_title,
};
use codemason_gui::dialogs::question::{
    ANSWER_LABEL, DEFAULT_MARK, DISMISS, OPTIONS_LABEL, QUESTION_TITLE, SELECT_OPTION,
};
use codemason_gui::dialogs::session_menu::SESSION_MENU_TITLE;
use codemason_gui::dialogs::{ActiveDialog, active};
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::prompt::PROMPT_LABEL;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use tokio::sync::mpsc::UnboundedSender;

/// The session the tests' questions belong to.
const SESSION: &str = "session-dialogs-more";

/// A shell over `app` with sources attached, the sender of its agent-event
/// queue, and the runtime driving it.
fn rig(
    configure: impl FnOnce(&mut App),
) -> (
    tokio::runtime::Runtime,
    UnboundedSender<Event>,
    Harness<'static, Shell>,
) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        // The answer delay reads a frozen clock that only `wait_shown` moves.
        app.advance_permission_clock_for_tests(std::time::Duration::ZERO);
        configure(&mut app);
        app
    };
    let (events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, events_tx, harness)
}

/// The agent asks question `id` with `options` (free text when empty).
fn ask(events: &UnboundedSender<Event>, id: &str, options: &[&str]) {
    events
        .send(Event::QuestionRequested {
            session_id: SESSION.to_string(),
            request_id: id.to_string(),
            question: format!("Question {id}?"),
            options: options.iter().map(ToString::to_string).collect(),
            default_index: if options.is_empty() { None } else { Some(0) },
            secret: false,
        })
        .expect("send question");
}

/// The `(request_id, response)` of the `QuestionAnswered` events published
/// so far.
fn answers(bus: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = bus.try_recv() {
        if let Event::QuestionAnswered {
            request_id,
            response,
            ..
        } = event
        {
            out.push((request_id, response));
        }
    }
    out
}

/// A press (or release) of `key` with no modifiers.
fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press and release `key` with its typed text, as egui-winit sends them.
fn type_key(harness: &mut Harness<'static, Shell>, key: egui::Key, text: Option<&str>) {
    let input = harness.input_mut();
    input.events.push(key_event(key, true));
    if let Some(text) = text {
        input.events.push(egui::Event::Text(text.to_string()));
    }
    input.events.push(key_event(key, false));
    harness.run();
}

/// Move app-core's frozen test clock past the answer delay
/// (`QUESTION_ANSWER_DELAY`), so the shown question can be answered, then
/// draw a frame. Nothing sleeps.
fn wait_shown(harness: &mut Harness<'static, Shell>) {
    harness
        .state_mut()
        .app_mut()
        .advance_permission_clock_for_tests(QUESTION_ANSWER_DELAY);
    harness.run();
}

/// Whether a dialog labelled `title` is drawn.
fn dialog_shown(harness: &Harness<'static, Shell>, title: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Dialog, title)
        .is_some()
}

#[test]
fn test_question_option_click_and_select_answers_through_app_core() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    ask(&events, "q-1", &["red", "green", "blue"]);
    harness.run();
    wait_shown(&mut harness);
    assert!(dialog_shown(&harness, QUESTION_TITLE));
    assert!(harness.query_by_label("Question q-1?").is_some());

    harness.get_by_label("green").click();
    harness.run();
    assert_eq!(harness.state().app().session.question_selected_index, 1);
    harness.get_by_label(SELECT_OPTION).click();
    harness.run();

    assert_eq!(
        answers(&mut bus),
        vec![("q-1".to_string(), "green".to_string())]
    );
    assert!(harness.state().app().session.question_queue.is_empty());
    harness.run();
    assert!(!dialog_shown(&harness, QUESTION_TITLE));
}

#[test]
fn test_question_option_keys_answer_like_the_tui() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    ask(&events, "q-2", &["one", "two", "three"]);
    harness.run();
    wait_shown(&mut harness);

    type_key(&mut harness, egui::Key::J, Some("j"));
    type_key(&mut harness, egui::Key::ArrowDown, None);
    type_key(&mut harness, egui::Key::Enter, None);

    assert_eq!(
        answers(&mut bus),
        vec![("q-2".to_string(), "three".to_string())]
    );
    assert_eq!(harness.state().app().session.input, "");
}

#[test]
fn test_question_free_text_typed_answer_and_enter_answers() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    ask(&events, "q-3", &[]);
    harness.run();
    wait_shown(&mut harness);
    assert!(harness.get_by_label(ANSWER_LABEL).is_focused());

    type_key(&mut harness, egui::Key::H, Some("h"));
    type_key(&mut harness, egui::Key::I, Some("i"));
    assert_eq!(harness.state().app().session.pending_question_input, "hi");
    type_key(&mut harness, egui::Key::Enter, None);

    assert_eq!(
        answers(&mut bus),
        vec![("q-3".to_string(), "hi".to_string())]
    );
    let app = harness.state().app();
    assert!(app.session.question_queue.is_empty());
    assert_eq!(app.session.input, "", "the prompt behind it is untouched");
}

#[test]
fn test_question_free_text_accepts_paste_and_ime_commit() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    ask(&events, "q-4", &[]);
    harness.run();
    wait_shown(&mut harness);

    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("from the clipboard ".to_string()));
    harness.run();
    // The Enter that confirms the composed word belongs to the input method.
    harness.input_mut().events.extend([
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".to_string(),
            active_range_chars: None,
        }),
        key_event(egui::Key::Enter, true),
        key_event(egui::Key::Enter, false),
    ]);
    harness.run();
    assert!(answers(&mut bus).is_empty(), "the IME Enter did not submit");
    harness
        .input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit("ni".to_string())));
    harness.run();

    assert_eq!(
        harness.state().app().session.pending_question_input,
        "from the clipboard ni"
    );

    type_key(&mut harness, egui::Key::Enter, None);
    assert_eq!(
        answers(&mut bus),
        vec![("q-4".to_string(), "from the clipboard ni".to_string())]
    );
}

#[test]
fn test_question_double_enter_answers_once() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    ask(&events, "q-5", &["a", "b"]);
    ask(&events, "q-6", &["c", "d"]);
    harness.run();
    wait_shown(&mut harness);

    let input = harness.input_mut();
    for _ in 0..2 {
        input.events.push(key_event(egui::Key::Enter, true));
        input.events.push(key_event(egui::Key::Enter, false));
    }
    harness.run();
    harness.get_by_label(SELECT_OPTION).click();
    harness.run();

    assert_eq!(
        answers(&mut bus),
        vec![("q-5".to_string(), "a".to_string())]
    );
    assert_eq!(
        harness
            .state()
            .app()
            .session
            .question_queue
            .front()
            .map(|r| r.id.as_str()),
        Some("q-6"),
        "the next question is not answered unseen"
    );
    wait_shown(&mut harness);
    harness.get_by_label(SELECT_OPTION).click();
    harness.run();
    assert_eq!(
        answers(&mut bus),
        vec![("q-6".to_string(), "c".to_string())]
    );
}

#[test]
fn test_question_options_are_a_material_list_with_the_default_marked() {
    let (_rt, events, mut harness) = rig(|_| {});
    ask(&events, "q-10", &["red", "green", "blue"]);
    harness.run();
    wait_shown(&mut harness);

    let list = harness
        .query_by_role_and_label(Role::List, OPTIONS_LABEL)
        .expect("the options are a Material list");
    assert!(list.rect().width() > 0.0);

    // The first option is the one app-core highlights and the default, so it
    // is the selected row and carries the default mark as its trailing text.
    let selected = harness.get_by_role_and_label(Role::ListItem, "red");
    assert_eq!(selected.accesskit_node().is_selected(), Some(true));
    assert_eq!(
        selected.accesskit_node().description().as_deref(),
        Some(DEFAULT_MARK),
        "the default option is marked"
    );
    for other in ["green", "blue"] {
        let row = harness.get_by_role_and_label(Role::ListItem, other);
        assert_ne!(row.accesskit_node().is_selected(), Some(true), "{other}");
        assert!(
            row.accesskit_node().description().is_none(),
            "{other} is not marked"
        );
    }
}

#[test]
fn test_question_options_and_buttons_are_disabled_until_answerable() {
    let (_rt, events, mut harness) = rig(|_| {});
    ask(&events, "q-11", &["yes", "no"]);
    harness.run();
    assert!(
        dialog_shown(&harness, QUESTION_TITLE),
        "the question is drawn before it can be answered"
    );
    assert!(
        harness
            .get_by_label(SELECT_OPTION)
            .accesskit_node()
            .is_disabled(),
        "Select waits for app-core's answer delay"
    );
    assert!(
        harness.get_by_label(DISMISS).accesskit_node().is_disabled(),
        "Dismiss waits too"
    );

    wait_shown(&mut harness);
    assert!(
        !harness
            .get_by_label(SELECT_OPTION)
            .accesskit_node()
            .is_disabled()
    );
    assert!(!harness.get_by_label(DISMISS).accesskit_node().is_disabled());
}

#[test]
fn test_question_free_text_answer_field_masks_a_secret_question() {
    let (_rt, events, mut harness) = rig(|_| {});
    events
        .send(Event::QuestionRequested {
            session_id: SESSION.to_string(),
            request_id: "q-12".to_string(),
            question: "Token?".to_string(),
            options: Vec::new(),
            default_index: None,
            secret: true,
        })
        .expect("send question");
    harness.run();
    wait_shown(&mut harness);
    // The password editor reports its role and never its text.
    assert!(
        harness
            .query_by_role_and_label(Role::PasswordInput, ANSWER_LABEL)
            .is_some(),
        "a secret question masks its answer field"
    );
}

#[test]
fn test_rollback_dialog_keep_changes_declines() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.pending_rollback = Some(RollbackOfferState {
            session_id: SESSION.to_string(),
            status: "budget_exhausted".to_string(),
            iterations: 7,
            diffstat: "2 files changed".to_string(),
            files: vec!["src/lib.rs".to_string()],
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, ROLLBACK_TITLE));
    assert!(harness.query_by_label("src/lib.rs").is_some());

    harness.get_by_label(KEEP_CHANGES).click();
    harness.run();

    let app = harness.state().app();
    assert!(app.session.pending_rollback.is_none());
    assert_eq!(app.status, "rollback declined - changes kept");
}

#[test]
fn test_plan_approval_dialog_keys_and_approve_button() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.session_id = None;
        app.session.plan_approval_pending = Some(PlanApprovalState {
            plan_text: "1. Read the code".to_string(),
            cursor_approve: false,
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, PLAN_TITLE));
    assert!(harness.query_by_label("1. Read the code").is_some());

    // Left/Right move the TUI's cursor between Approve and Reject.
    type_key(&mut harness, egui::Key::ArrowRight, None);
    let cursor = harness
        .state()
        .app()
        .session
        .plan_approval_pending
        .as_ref()
        .map(|p| p.cursor_approve);
    assert_eq!(cursor, Some(true));

    harness.get_by_label(APPROVE).click();
    harness.run();
    assert!(
        harness
            .state()
            .app()
            .session
            .plan_approval_pending
            .is_none()
    );
}

#[test]
fn test_force_cleanup_dialog_esc_cancels() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.pending_forcecleanup = Some(PendingForceCleanup {
            team_name: "alpha".to_string(),
            active_members: vec!["builder".to_string()],
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, FORCE_CLEANUP_TITLE));
    assert!(harness.query_by_label("Team: alpha").is_some());

    type_key(&mut harness, egui::Key::Escape, None);

    let app = harness.state().app();
    assert!(app.session.pending_forcecleanup.is_none());
    assert_eq!(app.status, "forcecleanup cancelled");
}

#[test]
fn test_router_save_dialog_cancel_button() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.pending_router_save =
            Some(codemason_agent::provider::router_config::RouterConfig::default());
    });
    harness.run();
    assert!(dialog_shown(&harness, ROUTER_SAVE_TITLE));

    harness
        .get_by_label(codemason_gui::dialogs::confirm::CANCEL)
        .click();
    harness.run();

    let app = harness.state().app();
    assert!(app.pending_router_save.is_none());
    assert_eq!(app.status, "Router save cancelled");
}

#[test]
fn test_notice_dialog_ok_closes() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.notice_open = true;
        app.notice_title = " Worktree missing ".to_string();
        app.notice_message = "The session's worktree no longer exists.".to_string();
        app.notice_detail = Some("/work/tree".to_string());
    });
    harness.run();
    assert!(dialog_shown(&harness, "Worktree missing"));
    assert!(harness.query_by_label("/work/tree").is_some());

    harness.get_by_label(NOTICE_OK).click();
    harness.run();
    assert!(!harness.state().app().notice_open);
}

#[test]
fn test_db_prune_dialog_no_cancels_and_keys_move_the_choice() {
    let (_rt, _events, mut harness) = rig(|app| app.open_db_prune_confirm(30));
    harness.run();
    assert!(dialog_shown(&harness, &db_prune_title(30)));

    type_key(&mut harness, egui::Key::ArrowLeft, None);
    assert!(harness.state().app().db_prune_confirm_is_yes());

    harness.get_by_label(NO).click();
    harness.run();
    let app = harness.state().app();
    assert!(!app.db_prune_confirm_open, "No cancels");
}

#[test]
fn test_db_prune_dialog_is_a_material_row_with_the_confirming_button_rightmost() {
    let (_rt, _events, mut harness) = rig(|app| app.open_db_prune_confirm(30));
    harness.run();
    assert!(dialog_shown(&harness, &db_prune_title(30)));

    // Both answers are Material buttons on one right-aligned row, and the
    // confirming one (Delete) sits to the right of the cancelling one (No).
    let confirm = harness
        .get_by_role_and_label(Role::Button, PRUNE_DELETE)
        .rect();
    let no = harness.get_by_role_and_label(Role::Button, NO).rect();
    assert!(
        confirm.left() > no.right(),
        "Delete {confirm:?} should be right of No {no:?}"
    );

    // The confirming button still moves app-core's own cursor before acting,
    // so the two-option choice stays app-core's.
    assert!(!harness.state().app().db_prune_confirm_is_yes());
    harness.get_by_label(PRUNE_DELETE).click();
    harness.run();
    let app = harness.state().app();
    assert!(!app.db_prune_confirm_open, "Delete runs the prune");
    assert_eq!(app.status, "db: pruned 0");
}

#[test]
fn test_compact_prune_dialog_no_cancels_and_yes_deletes() {
    let (_rt, _events, mut harness) = rig(|app| app.open_compact_prune_confirm());
    harness.run();
    assert!(dialog_shown(&harness, COMPACT_PRUNE_TITLE));

    harness.get_by_label(NO).click();
    harness.run();
    assert!(
        !harness.state().app().compact_prune_confirm_open,
        "No cancels"
    );

    harness.state_mut().app_mut().open_compact_prune_confirm();
    harness.run();
    type_key(&mut harness, egui::Key::ArrowLeft, None);
    assert!(harness.state().app().compact_prune_confirm_is_yes());
    harness.get_by_label(COMPACT_DELETE).click();
    harness.run();
    let app = harness.state().app();
    assert!(!app.compact_prune_confirm_open, "Delete all prunes");
    assert_eq!(app.status, "compact: pruned 0");
}

#[test]
fn test_loop_dialog_rejects_empty_goal() {
    let (_rt, _events, mut harness) = rig(codemason_app_core::app::open_loop_setup);
    harness.run();
    assert!(dialog_shown(&harness, LOOP_TITLE));

    harness.get_by_label(START).click();
    harness.run();

    let app = harness.state().app();
    let error = app.loop_setup.as_ref().and_then(|s| s.error.clone());
    assert!(
        error.as_deref().is_some_and(|e| e.contains("goal")),
        "the dialog stays open with the error: {error:?}"
    );
    assert!(dialog_shown(&harness, LOOP_TITLE));
    assert!(
        harness
            .query_by_label_contains("missing field: goal")
            .is_some()
    );
}

#[test]
fn test_loop_dialog_typing_fills_the_goal_and_esc_keeps_the_draft() {
    let (_rt, _events, mut harness) = rig(codemason_app_core::app::open_loop_setup);
    harness.run();

    type_key(&mut harness, egui::Key::Tab, None);
    type_key(&mut harness, egui::Key::G, Some("g"));
    type_key(&mut harness, egui::Key::O, Some("o"));
    let goal = harness
        .state()
        .app()
        .loop_setup
        .as_ref()
        .map(|s| s.goal_field.text().to_string());
    assert_eq!(goal.as_deref(), Some("go"));

    type_key(&mut harness, egui::Key::Escape, None);
    let app = harness.state().app();
    assert!(app.loop_setup.is_none());
    assert!(app.loop_setup_draft.is_some(), "Esc keeps the values");
    assert_eq!(app.session.input, "");
}

#[test]
fn test_loop_dialog_draws_material_fields_a_list_and_a_chip() {
    let (_rt, _events, mut harness) = rig(codemason_app_core::app::open_loop_setup);
    harness.run();
    assert!(dialog_shown(&harness, LOOP_TITLE));

    // The seven key-driven fields are read-only Material fields whose value
    // is the App's buffer; the goal starts empty and `max steps` carries the
    // config default.
    assert_eq!(
        harness.get_by_label("goal").value().as_deref(),
        Some(""),
        "the goal is an outlined field showing the App's buffer"
    );
    assert_eq!(
        harness.get_by_label("max steps").value().as_deref(),
        Some("512"),
        "the config default is shown, not a second editor"
    );

    // The agent picker is a labelled Material list; app-core's selected
    // agent is the selected row.
    harness.get_by_role_and_label(Role::List, AGENTS_LABEL);
    let (selected_name, _) = harness
        .state()
        .app()
        .loop_setup
        .as_ref()
        .and_then(|s| s.agents.get(s.selected_agent).cloned())
        .expect("the dialog has agents");
    let row = harness.get_by_role_and_label(Role::ListItem, &selected_name);
    assert_eq!(row.accesskit_node().is_selected(), Some(true));

    // The checkpoints switch is a filter chip reporting its own state.
    let chip = harness.get_by_label(CHECKPOINTS_LABEL);
    assert_eq!(
        chip.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True),
        "checkpoints default on"
    );
}

#[test]
fn test_loop_dialog_chip_toggles_checkpoints_through_app_core() {
    let (_rt, _events, mut harness) = rig(codemason_app_core::app::open_loop_setup);
    harness.run();
    let checkpoints = |harness: &Harness<'static, Shell>| {
        harness
            .state()
            .app()
            .loop_setup
            .as_ref()
            .map(|s| s.checkpoints)
    };
    assert_eq!(checkpoints(&harness), Some(true));

    harness.get_by_label(CHECKPOINTS_LABEL).click();
    harness.run();

    assert_eq!(
        checkpoints(&harness),
        Some(false),
        "a click sends Space, so app-core toggles it"
    );
    assert_eq!(
        harness
            .state()
            .app()
            .loop_setup
            .as_ref()
            .map(|s| s.active_field),
        Some(LoopSetupField::Checkpoints),
        "the click moves the active field to the chip first"
    );
}

#[test]
fn test_loop_dialog_agent_row_click_moves_the_highlight_only() {
    let (_rt, _events, mut harness) = rig(|app| {
        codemason_app_core::app::open_loop_setup(app);
        if let Some(state) = app.loop_setup.as_mut() {
            state.selected_agent = 0;
        }
    });
    harness.run();
    let names: Vec<String> = harness
        .state()
        .app()
        .loop_setup
        .as_ref()
        .map(|s| s.agents.iter().map(|(name, _)| name.clone()).collect())
        .unwrap_or_default();
    assert!(names.len() > 1, "the roster has a second agent: {names:?}");

    harness
        .get_by_role_and_label(Role::ListItem, &names[1])
        .click();
    harness.run();

    let app = harness.state().app();
    let state = app.loop_setup.as_ref().expect("the dialog stays open");
    assert_eq!(state.selected_agent, 1, "the click moves the highlight");
    assert!(state.error.is_none(), "and starts nothing");
}

/// An open properties dialog for project `demo`.
fn properties() -> ProjectProperties {
    let global = |label: &str| Choice {
        label: label.to_string(),
        value: None,
        enabled: true,
    };
    ProjectProperties {
        project_id: "p-1".to_string(),
        project_name: "demo".to_string(),
        description: String::new(),
        caret: 0,
        backends: vec![global("Global (local)")],
        backend: 0,
        models: vec![global("Global (none)")],
        model: 0,
        focus: Field::Description,
        list: None,
        error: None,
        area: Area::default(),
    }
}

#[test]
fn test_properties_dialog_typing_paste_and_esc() {
    let (_rt, _events, mut harness) = rig(|app| app.sidebar.properties = Some(properties()));
    harness.run();
    assert!(dialog_shown(&harness, &properties_title("demo")));

    type_key(&mut harness, egui::Key::X, Some("x"));
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("yz".to_string()));
    harness.run();
    let description = harness
        .state()
        .app()
        .sidebar
        .properties
        .as_ref()
        .map(|p| p.description.clone());
    assert_eq!(description.as_deref(), Some("xyz"));
    assert_eq!(harness.state().app().session.input, "", "not the prompt");

    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().sidebar.properties.is_none());
}

#[test]
fn test_properties_dialog_draws_material_fields_with_their_values() {
    let (_rt, _events, mut harness) = rig(|app| app.sidebar.properties = Some(properties()));
    harness.run();
    assert!(dialog_shown(&harness, &properties_title("demo")));
    assert_eq!(
        harness.get_by_label(DESCRIPTION_LABEL).value().as_deref(),
        Some(""),
        "the description is an outlined field"
    );
    assert_eq!(
        harness.get_by_label(BACKEND_LABEL).value().as_deref(),
        Some("Global (local)")
    );
    assert_eq!(
        harness.get_by_label(MODEL_LABEL).value().as_deref(),
        Some("Global (none)")
    );
    // The description is the focused field, so typing reaches it.
    type_key(&mut harness, egui::Key::X, Some("x"));
    assert_eq!(
        harness.get_by_label(DESCRIPTION_LABEL).value().as_deref(),
        Some("x"),
        "the focused field shows what was typed"
    );
}

#[test]
fn test_properties_dialog_choice_list_is_a_material_list_and_click_picks() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.sidebar.properties = Some(ProjectProperties {
            backends: vec![
                Choice {
                    label: "Global (local)".to_string(),
                    value: None,
                    enabled: true,
                },
                Choice {
                    label: "remote (not configured)".to_string(),
                    value: Some("remote".to_string()),
                    enabled: false,
                },
            ],
            ..properties()
        });
    });
    harness.run();
    // No list until Space opens the focused (description) field's list; the
    // backend's opens once Tab moves there.
    assert!(
        harness
            .query_by_role_and_label(Role::List, CHOICES_LABEL)
            .is_none()
    );
    type_key(&mut harness, egui::Key::Tab, None);
    type_key(&mut harness, egui::Key::Space, Some(" "));
    let list = harness
        .query_by_role_and_label(Role::List, CHOICES_LABEL)
        .expect("the open choice list is a Material list");
    assert!(list.rect().width() > 0.0);

    // The disabled entry is drawn but does nothing; the enabled one picks.
    let disabled = harness.get_by_label("remote (not configured)");
    assert!(
        disabled.accesskit_node().is_disabled(),
        "a backend that cannot run is greyed"
    );
    disabled.click();
    harness.run();
    assert_eq!(
        harness
            .state()
            .app()
            .sidebar
            .properties
            .as_ref()
            .map(|p| p.backend),
        Some(0),
        "a disabled entry does not pick"
    );
    harness.get_by_label("Global (local)").click();
    harness.run();
    let app = harness.state().app();
    assert_eq!(
        app.sidebar.properties.as_ref().map(|p| p.backend),
        Some(0),
        "picking Global keeps the stored choice"
    );
}

#[test]
fn test_properties_dialog_material_buttons_save_and_cancel() {
    let (_rt, _events, mut harness) = rig(|app| app.sidebar.properties = Some(properties()));
    harness.run();
    type_key(&mut harness, egui::Key::X, Some("x"));
    harness.get_by_label(CANCEL).click();
    harness.run();
    assert!(
        harness.state().app().sidebar.properties.is_none(),
        "Cancel sends Esc"
    );

    let (_rt, _events, mut harness) = rig(|app| app.sidebar.properties = Some(properties()));
    harness.run();
    harness.get_by_label(SAVE).click();
    harness.run();
    // Save sends Enter, which is app-core's own save and close.
    let app = harness.state().app();
    assert!(app.sidebar.properties.is_none(), "Save sends Enter");
    assert!(
        app.status.starts_with("demo: backend "),
        "the status names what was saved: {}",
        app.status
    );
}

/// A session menu over the multi-selection with one item.
fn selection_menu() -> SessionMenu {
    SessionMenu {
        target: MenuTarget::Selection,
        row_y: 0,
        items: vec![MenuItem {
            label: "Clear selection".to_string(),
            accel: Some('c'),
            action: MenuAction::ClearSelection,
            group: 0,
            danger: false,
            enabled: true,
        }],
        cursor: 0,
        area: Area::default(),
        scroll: 0,
        submenu: None,
    }
}

#[test]
fn test_session_menu_click_runs_the_item() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.sidebar.selected.insert("s-a".to_string());
        app.sidebar.menu = Some(selection_menu());
    });
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Menu, SESSION_MENU_TITLE)
            .is_some()
    );

    harness.get_by_label("Clear selection").click();
    harness.run();

    let app = harness.state().app();
    assert!(app.sidebar.menu.is_none());
    assert!(app.sidebar.selected.is_empty());
}

#[test]
fn test_session_menu_typing_never_reaches_the_prompt() {
    let (_rt, _events, mut harness) = rig(|app| app.sidebar.menu = Some(selection_menu()));
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();

    type_key(&mut harness, egui::Key::Z, Some("z"));
    assert_eq!(harness.state().app().session.input, "");
    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().sidebar.menu.is_none());
}

#[test]
fn test_history_picker_click_and_select_fills_the_prompt() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.history_picker = Some(HistoryPickerState {
            entries: vec!["newest".to_string(), "older".to_string()],
            selected: 0,
            scroll_offset: 0,
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, HISTORY_TITLE));

    // The entries are a Material list of list items, the highlight marked
    // as the selected row.
    harness.get_by_role_and_label(Role::List, ENTRIES_LABEL);
    let rows = harness.get_all_by_role(Role::ListItem).count();
    assert_eq!(rows, 2, "one list item per entry");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::ListItem, "newest")
            .accesskit_node()
            .is_selected(),
        Some(true)
    );

    harness.get_by_label("older").click();
    harness.run();
    harness.get_by_label(SELECT_ENTRY).click();
    harness.run();

    let app = harness.state().app();
    assert!(app.history_picker.is_none());
    assert_eq!(app.session.input, "older");
}

/// A rig whose App has the built-in theme catalog, the `system` theme and
/// the theme picker open.
fn theme_picker_rig() -> (
    tokio::runtime::Runtime,
    UnboundedSender<Event>,
    Harness<'static, Shell>,
) {
    rig(|app| {
        app.themes = codemason_app_core::theme_tokens::ThemeCatalog::builtin();
        app.theme = codemason_app_core::theme_tokens::ThemeSelection::System;
        app.open_theme_picker();
    })
}

/// The built-in theme `name` as a selection.
fn theme_named(name: &str) -> codemason_app_core::theme_tokens::ThemeSelection {
    codemason_app_core::theme_tokens::ThemeSelection::Named(name.to_string())
}

/// The egui visuals of the built-in theme `name`.
fn theme_visuals(name: &str) -> egui::Visuals {
    let themes = codemason_app_core::theme_tokens::ThemeCatalog::builtin();
    codemason_gui::theme::visuals_for(&themes.get(name).expect("built-in theme"))
}

#[test]
fn test_theme_picker_previews_on_click_and_cancel_restores() {
    let _config = support::scratch_config_target();
    let (_rt, _events, mut harness) = theme_picker_rig();
    harness.run();
    assert!(dialog_shown(&harness, THEME_TITLE));

    // A click on an entry previews that theme at once.
    harness.get_by_label("nord").click();
    harness.run();
    assert_eq!(harness.state().app().theme, theme_named("nord"));
    assert_eq!(harness.ctx.global_style().visuals, theme_visuals("nord"));
    assert!(harness.state().app().theme_picker.is_some());

    // Cancel restores the theme the picker opened on.
    harness.get_by_label(CANCEL_THEME).click();
    harness.run();
    let app = harness.state().app();
    assert_eq!(
        app.theme,
        codemason_app_core::theme_tokens::ThemeSelection::System
    );
    assert!(app.theme_picker.is_none());
    assert_eq!(
        harness.ctx.global_style().visuals,
        theme_visuals("codemason-dark")
    );
}

#[test]
fn test_theme_picker_keys_preview_and_apply_keeps_the_theme() {
    let _config = support::scratch_config_target();
    let (_rt, _events, mut harness) = theme_picker_rig();
    harness.run();

    // Down moves the highlight from `system` to the first theme.
    type_key(&mut harness, egui::Key::ArrowDown, None);
    assert_eq!(harness.state().app().theme, theme_named("codemason-dark"));

    harness.get_by_label("codemason-light").click();
    harness.run();
    harness.get_by_label(APPLY_THEME).click();
    harness.run();
    let app = harness.state().app();
    assert_eq!(app.theme, theme_named("codemason-light"));
    assert!(app.theme_picker.is_none());
    assert_eq!(
        harness.ctx.global_style().visuals,
        theme_visuals("codemason-light")
    );
}

#[test]
fn test_theme_picker_e_opens_the_editor_on_the_highlighted_theme() {
    let _config = support::scratch_config_target();
    let (_rt, _events, mut harness) = theme_picker_rig();
    harness.run();

    // On `system` (row 0) `e` opens nothing: that row has no colours and
    // draws no edit button either.
    type_key(&mut harness, egui::Key::E, None);
    assert!(
        harness.state().app().theme_editor.is_none(),
        "the system row has no colours to edit"
    );

    // Down moves to the first theme; `e` then opens the editor on it, the
    // keyboard equivalent of that row's edit button.
    type_key(&mut harness, egui::Key::ArrowDown, None);
    let highlighted = harness.state().app().theme.display_name().to_string();
    type_key(&mut harness, egui::Key::E, None);
    let app = harness.state().app();
    let editor = app.theme_editor.as_ref().expect("e opens the editor");
    assert_eq!(
        editor.name, highlighted,
        "the editor opens on the shown theme"
    );
}

#[test]
fn test_config_save_picker_esc_cancels() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.config_save_picker = Some(ConfigSavePickerState {
            entries: vec![std::path::PathBuf::from("saves/codemason-1.json")],
            selected: 0,
            scroll_offset: 0,
            config_dir: std::path::PathBuf::from("target/temp/task33b-config"),
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, CONFIG_SAVES_TITLE));
    assert!(harness.query_by_label("codemason-1.json").is_some());

    type_key(&mut harness, egui::Key::Escape, None);
    let app = harness.state().app();
    assert!(app.config_save_picker.is_none());
    assert_eq!(app.status, "config list: cancelled");
}

#[test]
fn test_run_cost_banner_is_a_material_snackbar_with_a_dismiss_action() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.run_cost_banner = Some("* run complete * 10+20 tokens *".to_string());
        app.session.run_cost_banner_at = Some(std::time::Instant::now());
    });
    harness.run();
    // The banner keeps `BANNER_LABEL` as its accessible name (a
    // `Role::Status`, the summary read under it) and Dismiss as its one
    // action, which is what a Material snackbar is.
    harness.get_by_role_and_label(Role::Status, BANNER_LABEL);
    harness.get_by_role_and_label(Role::Button, DISMISS_BANNER);
    assert!(
        harness
            .query_by_label_contains("* run complete * 10+20 tokens *")
            .is_some(),
        "the summary is the snackbar's message"
    );
    assert!(
        harness
            .query_by_role_and_label(Role::MenuItem, DISMISS_BANNER)
            .is_none(),
        "the action is a button, not a menu row"
    );
}

#[test]
fn test_run_cost_banner_dismiss_and_typing() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.run_cost_banner = Some("* run complete * 10+20 tokens *".to_string());
        app.session.run_cost_banner_at = Some(std::time::Instant::now());
    });
    harness.run();
    assert!(harness.query_by_label(BANNER_LABEL).is_some());
    harness.get_by_label(DISMISS_BANNER).click();
    harness.run();
    assert!(harness.state().app().session.run_cost_banner.is_none());

    // As in the TUI, a typed character dismisses the banner and still types.
    harness.state_mut().app_mut().session.run_cost_banner = Some("again".to_string());
    harness.run();
    type_key(&mut harness, egui::Key::A, Some("a"));
    let app = harness.state().app();
    assert!(app.session.run_cost_banner.is_none());
    assert_eq!(app.session.input, "a");

    // A non-character key is used up dismissing it, as in the TUI.
    harness.state_mut().app_mut().session.run_cost_banner = Some("again".to_string());
    harness.run();
    type_key(&mut harness, egui::Key::Backspace, None);
    let app = harness.state().app();
    assert!(app.session.run_cost_banner.is_none());
    assert_eq!(app.session.input, "a");
}

#[test]
fn test_dialog_dispatch_follows_app_core_key_priority_for_the_new_dialogs() {
    let (_rt, events, mut harness) = rig(|app| {
        app.notice_open = true;
        app.notice_title = " Note ".to_string();
        app.session.pending_forcecleanup = Some(PendingForceCleanup {
            team_name: "alpha".to_string(),
            active_members: Vec::new(),
        });
        app.sidebar.menu = Some(selection_menu());
    });
    ask(&events, "q-7", &["x"]);
    harness.run();
    assert_eq!(active(harness.state().app()), Some(ActiveDialog::Question));
    assert!(dialog_shown(&harness, QUESTION_TITLE));
    assert!(!dialog_shown(&harness, "Note"));

    wait_shown(&mut harness);
    type_key(&mut harness, egui::Key::Escape, None);
    assert_eq!(active(harness.state().app()), Some(ActiveDialog::Notice));
    type_key(&mut harness, egui::Key::Enter, None);
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::ForceCleanup)
    );
    type_key(&mut harness, egui::Key::Escape, None);
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::SessionMenu)
    );
}

#[test]
fn test_answering_a_free_text_question_gives_the_keyboard_back_to_the_prompt() {
    let (_rt, events, mut harness) = rig(|_| {});
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();
    ask(&events, "q-8", &[]);
    harness.run();
    wait_shown(&mut harness);
    assert!(harness.get_by_label(ANSWER_LABEL).is_focused());

    type_key(&mut harness, egui::Key::K, Some("k"));
    type_key(&mut harness, egui::Key::Enter, None);
    harness.run();
    harness.run();

    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
            .is_focused()
    );
}

#[test]
fn test_timed_question_behind_another_modal_auto_answers_its_default() {
    let (_rt, events, mut harness) = rig(|app| app.show_shortcuts = true);
    let mut bus = harness.state().app().event_bus.subscribe();
    events
        .send(Event::QuestionRequested {
            session_id: SESSION.to_string(),
            request_id: "q-9".to_string(),
            question: "Proceed?".to_string(),
            options: vec!["yes".to_string(), "no".to_string()],
            default_index: Some(1),
            secret: false,
        })
        .expect("send question");
    harness.run();
    assert!(
        !dialog_shown(&harness, QUESTION_TITLE),
        "the shortcuts panel is above it"
    );
    assert_eq!(
        answers(&mut bus),
        [] as [(std::string::String, std::string::String); 0]
    );

    // The question's 120 s timeout passes on the test clock; nothing sleeps.
    harness
        .state_mut()
        .app_mut()
        .advance_permission_clock_for_tests(std::time::Duration::from_secs(121));
    harness.run();

    assert_eq!(
        answers(&mut bus),
        vec![("q-9".to_string(), "no".to_string())]
    );
    let app = harness.state().app();
    assert!(app.session.question_queue.is_empty());
    assert!(app.show_shortcuts, "the panel above it is untouched");
}

/// Open the `/loop` dialog with the goal field active; the runtime is
/// returned so it outlives the harness.
fn loop_on_goal() -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let (rt, _events, mut harness) = rig(codemason_app_core::app::open_loop_setup);
    harness.run();
    type_key(&mut harness, egui::Key::Tab, None);
    (rt, harness)
}

/// The loop dialog's goal text.
fn goal(harness: &Harness<'static, Shell>) -> Option<String> {
    harness
        .state()
        .app()
        .loop_setup
        .as_ref()
        .map(|s| s.goal_field.text().to_string())
}

#[test]
fn test_loop_goal_takes_text_with_no_key_once() {
    let (_rt, mut harness) = loop_on_goal();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("\u{fc}".to_string()));
    harness.run();
    assert_eq!(goal(&harness).as_deref(), Some("\u{fc}"));
}

#[test]
fn test_loop_goal_takes_an_ime_commit_once() {
    let (_rt, mut harness) = loop_on_goal();
    harness
        .input_mut()
        .events
        .push(egui::Event::Ime(egui::ImeEvent::Commit(
            "na\u{ef}ve".to_string(),
        )));
    harness.run();
    assert_eq!(goal(&harness).as_deref(), Some("na\u{ef}ve"));
}

#[test]
fn test_loop_goal_altgr_types_its_character_not_the_key() {
    let (_rt, mut harness) = loop_on_goal();
    let altgr = egui::Modifiers {
        ctrl: true,
        alt: true,
        ..egui::Modifiers::NONE
    };
    let input = harness.input_mut();
    input.events.push(egui::Event::Key {
        key: egui::Key::Q,
        physical_key: Some(egui::Key::Q),
        pressed: true,
        repeat: false,
        modifiers: altgr,
    });
    input.events.push(egui::Event::Text("@".to_string()));
    input.events.push(egui::Event::Key {
        key: egui::Key::Q,
        physical_key: Some(egui::Key::Q),
        pressed: false,
        repeat: false,
        modifiers: altgr,
    });
    harness.run();
    assert_eq!(goal(&harness).as_deref(), Some("@"));
}

#[test]
fn test_loop_goal_takes_a_paste_once() {
    let (_rt, mut harness) = loop_on_goal();
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("fix\r\nthe build".to_string()));
    harness.run();
    assert_eq!(goal(&harness).as_deref(), Some("fix the build"));
    assert!(
        harness.state().app().loop_setup.is_some(),
        "the line break did not start the loop"
    );
}

#[test]
fn test_loop_paste_is_ignored_off_the_text_fields() {
    let (_rt, _events, mut harness) = rig(|app| {
        codemason_app_core::app::open_loop_setup(app);
        if let Some(state) = app.loop_setup.as_mut() {
            state.active_field = LoopSetupField::Checkpoints;
        }
    });
    harness.run();
    let checkpoints = |harness: &Harness<'static, Shell>| {
        harness
            .state()
            .app()
            .loop_setup
            .as_ref()
            .map(|s| s.checkpoints)
    };
    let before = checkpoints(&harness);
    assert!(before.is_some());

    // A paste holding a space would toggle checkpoints if typed as keys.
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("a b".to_string()));
    harness.run();
    assert_eq!(
        checkpoints(&harness),
        before,
        "a paste never toggles checkpoints"
    );

    // On the agent picker a paste is ignored too.
    if let Some(state) = harness.state_mut().app_mut().loop_setup.as_mut() {
        state.active_field = LoopSetupField::Agent;
    }
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("goal text".to_string()));
    harness.run();
    let app = harness.state().app();
    let state = app.loop_setup.as_ref().expect("dialog still open");
    assert_eq!(state.goal_field.text(), "");
    assert_eq!(state.checkpoints, before.unwrap_or_default());
    assert_eq!(app.session.input, "");
}

#[test]
fn test_properties_description_takes_paste_and_text_with_no_key() {
    let (_rt, _events, mut harness) = rig(|app| app.sidebar.properties = Some(properties()));
    harness.run();
    let input = harness.input_mut();
    input.events.push(egui::Event::Paste("caf".to_string()));
    input.events.push(egui::Event::Text("\u{e9}".to_string()));
    harness.run();
    let description = harness
        .state()
        .app()
        .sidebar
        .properties
        .as_ref()
        .map(|p| p.description.clone());
    assert_eq!(description.as_deref(), Some("caf\u{e9}"));
}

#[test]
fn test_db_prune_dialog_keys_work_with_the_prompt_focused() {
    let (_rt, _events, mut harness) = rig(|_| {});
    harness.run();
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();
    // `/db prune` typed in the prompt opens the dialog while the prompt
    // still has the keyboard; a Right pressed in the very next frame (before
    // egui's modal takes the focus) must reach app-core, not the caret.
    harness.state_mut().app_mut().open_db_prune_confirm(30);
    type_key(&mut harness, egui::Key::ArrowRight, None);
    assert!(dialog_shown(&harness, &db_prune_title(30)));
    assert!(harness.state().app().db_prune_confirm_is_yes());
    type_key(&mut harness, egui::Key::Enter, None);
    harness.run();
    let app = harness.state().app();
    assert!(!app.db_prune_confirm_open, "Enter on Yes runs the prune");
    assert_eq!(app.session.input, "");
}

#[test]
fn test_context_menu_rows_are_material_items_with_their_icons_and_state() {
    // Pin an empty clipboard so Paste's enabled state does not follow the host.
    let _clipboard = codemason_app_core::clipboard::ClipboardTestOverrideGuard::new("");
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.input = "hello world".to_string();
        app.session.input_cursor = 11;
        app.open_context_menu(0, 0, SelectionPane::Input, true);
    });
    harness.run();
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::ContextMenu)
    );

    // The menu is one Material menu surface holding a row per action: the
    // label is the row (its icon is painted) and app-core's enabled state
    // and cursor come through, so the chrome adds nothing app-core did not
    // decide.
    harness.get_by_role_and_label(Role::MenuListPopup, CONTEXT_MENU_LABEL);
    let menu = {
        let app = harness.state().app();
        app.context_menu.clone().expect("menu open")
    };
    assert_eq!(menu.items.len(), 3, "Cut, Copy and Paste on the prompt");
    for (action, enabled) in &menu.items {
        let row = harness.get_by_role_and_label(Role::MenuItem, item_label(*action));
        assert_eq!(
            !row.accesskit_node().is_disabled(),
            *enabled,
            "{} mirrors app-core's enabled state",
            item_label(*action)
        );
    }
    let on_cursor = item_label(menu.items[menu.selected].0);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MenuItem, on_cursor)
            .accesskit_node()
            .is_selected(),
        Some(true),
        "the row app-core's cursor is on is the selected item"
    );

    // A disabled action is not chosen by a click, as the plain disabled
    // button was not: nothing runs and the prompt keeps its text.
    assert!(!menu.items[2].1, "Paste is disabled with no clipboard");
    harness
        .get_by_role_and_label(Role::MenuItem, item_label(ContextAction::Paste))
        .click();
    harness.run();
    let app = harness.state().app();
    assert_eq!(app.session.input, "hello world", "nothing was pasted");
}
