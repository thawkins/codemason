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
    App, ConfigSavePickerState, HistoryPickerState, LoopSetupField, PendingForceCleanup,
    PlanApprovalState, QUESTION_ANSWER_DELAY, RollbackOfferState,
};
use codemason_app_core::geometry::Area;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::project_properties::{Choice, Field, ProjectProperties};
use codemason_app_core::session_menu::{MenuAction, MenuItem, MenuTarget, SessionMenu};
use codemason_gui::Shell;
use codemason_gui::dialogs::banner::{BANNER_LABEL, DISMISS_BANNER};
use codemason_gui::dialogs::confirm::{
    APPROVE, FORCE_CLEANUP_TITLE, KEEP_CHANGES, NO, NOTICE_OK, PLAN_TITLE, ROLLBACK_TITLE,
    ROUTER_SAVE_TITLE, db_prune_title,
};
use codemason_gui::dialogs::loop_setup::{LOOP_TITLE, START};
use codemason_gui::dialogs::pickers::{
    APPLY_THEME, CANCEL_THEME, CONFIG_SAVES_TITLE, HISTORY_TITLE, SELECT_ENTRY, THEME_TITLE,
};
use codemason_gui::dialogs::properties::properties_title;
use codemason_gui::dialogs::question::{ANSWER_LABEL, QUESTION_TITLE, SELECT_OPTION};
use codemason_gui::dialogs::session_menu::SESSION_MENU_TITLE;
use codemason_gui::dialogs::{ActiveDialog, active};
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::prompt::PROMPT_LABEL;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
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
    assert!(dialog_shown(&harness, SESSION_MENU_TITLE));

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
