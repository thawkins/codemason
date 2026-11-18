//! Tests for the remaining dock panels (`codemason_gui::panels`): Profile,
//! Tasks, Memory, Active agents and Teams.
//!
//! Every test drives the whole [`Shell`] frame, so the dock, the key router
//! and app-core's key handling are exercised together: each panel is found
//! by its accessible label and acted on through app-core (a key press the
//! App handles, or a click that calls an app-core action). Each test owns a
//! tokio runtime that it does not enter while the harness runs, because the
//! shell drives `App::handle_key` with `Handle::block_on`. Nothing sleeps,
//! and nothing is written outside the in-memory store.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamMember};
use codemason_app_core::app::App;
use codemason_gui::Shell;
use codemason_gui::dialogs::confirm::{
    MEMORY_CLEAR_TITLE, MEMORY_DELETE_TITLE, NO, YES, memory_view_title, row_action_title,
};
use codemason_gui::dock::PaneId;
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::agents::AGENTS_LABEL;
use codemason_gui::panels::memory::MEMORY_LABEL;
use codemason_gui::panels::profile::PROFILE_LABEL;
use codemason_gui::panels::prompt::PROMPT_LABEL;
use codemason_gui::panels::tasks::TASKS_LABEL;
use codemason_gui::panels::teams::TEAMS_LABEL;
use craft_keymap::Platform;
use craft_ui::docking::PanelState;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window: the runtime and the harness over the shell.
struct Rig {
    runtime: tokio::runtime::Runtime,
    harness: Harness<'static, Shell>,
}

impl Rig {
    /// Run `/command` through the App's slash path, outside the harness.
    fn slash(&mut self, command: &str) {
        let app = self.harness.state_mut().app_mut();
        self.runtime.block_on(app.execute_slash_command(command));
        self.harness.run();
    }
}

/// A shell over an App `configure` prepared, with sources attached.
fn rig(configure: impl FnOnce(&tokio::runtime::Runtime, &mut App)) -> Rig {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    configure(&runtime, &mut app);
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = codemason_app_core::log_channel::tui_log_channel(64);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    // The core draws glyphs (status icons, box drawing) that egui's bundled
    // fonts lack; these tests are about the panels, so they may draw as
    // tofu.
    let harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    let mut rig = Rig { runtime, harness };
    rig.harness.run();
    rig
}

/// Start a stored session so the App has a primary session id.
fn start_session(runtime: &tokio::runtime::Runtime, app: &mut App) -> String {
    runtime.block_on(app.execute_slash_command("/session list"));
    app.session.session_id.clone().expect("session")
}

/// A press (or release) of `key` with `modifiers`.
fn key_event(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

/// Press and release `key` with `modifiers` and the text it typed, as
/// egui-winit sends them, then draw a frame.
fn press(rig: &mut Rig, key: egui::Key, modifiers: egui::Modifiers, text: Option<&str>) {
    let input = rig.harness.input_mut();
    input.events.push(key_event(key, true, modifiers));
    if let Some(text) = text {
        input.events.push(egui::Event::Text(text.to_string()));
    }
    input.events.push(key_event(key, false, modifiers));
    rig.harness.run();
}

/// Press `key` with no modifiers and no text.
fn tap(rig: &mut Rig, key: egui::Key) {
    press(rig, key, egui::Modifiers::NONE, None);
}

/// Press Alt+`key` (egui-winit types its letter with Alt on Linux).
fn alt(rig: &mut Rig, key: egui::Key, letter: &str) {
    press(rig, key, egui::Modifiers::ALT, Some(letter));
    rig.harness.run();
}

/// Focus the prompt editor and draw a frame.
fn focus_prompt(rig: &mut Rig) {
    rig.harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    rig.harness.run();
}

/// Whether the node with `role` and `label` is drawn.
fn shown(rig: &Rig, role: Role, label: &str) -> bool {
    rig.harness.query_by_role_and_label(role, label).is_some()
}

/// A live sub-agent task under `parent`.
fn task(id: &str, parent: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// Store a memory of `category` for the App's project.
fn remember(app: &App, content: &str, category: &str) -> i64 {
    app.storage
        .create_memory(
            content,
            category,
            "test",
            0.9,
            &app.cwd_path.display().to_string(),
            "",
            &["gui".to_string()],
        )
        .expect("store memory")
}

/// The number of memories stored for the App's project.
fn memory_count(rig: &Rig) -> u64 {
    let app = rig.harness.state().app();
    app.storage
        .count_memories_for_project(&app.cwd_path)
        .expect("count memories")
}

#[test]
fn test_profile_panel_shows_profiler_table() {
    let mut rig = rig(|_, _| {});
    focus_prompt(&mut rig);
    assert!(!shown(&rig, Role::Table, PROFILE_LABEL));

    // Alt+P runs app-core's ToggleProfile, which turns the profiler on.
    alt(&mut rig, egui::Key::P, "p");
    assert_eq!(rig.harness.state().app().status, "profile panel visible");
    assert!(shown(&rig, Role::Table, PROFILE_LABEL));
    rig.harness.get_by_label_contains("sorted by self time");
    rig.harness.get_by_label_contains("operation");

    alt(&mut rig, egui::Key::P, "p");
    assert!(!rig.harness.state().app().show_profile);
    assert!(!shown(&rig, Role::Table, PROFILE_LABEL));
}

#[test]
fn test_tasks_panel_lists_tasks() {
    let mut rig = rig(|rt, app| {
        let sid = start_session(rt, app);
        app.storage
            .create_task_simple("t-1", &sid, "Write the parser", "pending", "")
            .expect("task 1");
        app.storage
            .create_task_simple("t-2", &sid, "Ship the release", "completed", "")
            .expect("task 2");
    });
    focus_prompt(&mut rig);

    // Alt+I runs app-core's ToggleTasksPanel, which marks the cache dirty.
    alt(&mut rig, egui::Key::I, "i");
    assert_eq!(rig.harness.state().app().status, "tasks panel visible");
    assert!(shown(&rig, Role::List, TASKS_LABEL));
    rig.harness
        .get_by_label_contains("[PENDING] t-1 Write the parser");
    rig.harness
        .get_by_label_contains("[COMPLETED] t-2 Ship the release");
}

#[test]
fn test_memory_panel_clear_requires_confirm() {
    let mut rig = rig(|_, app| {
        remember(app, "prefers tabs", "preference");
        remember(app, "uses nextest", "fact");
    });
    focus_prompt(&mut rig);
    alt(&mut rig, egui::Key::M, "m");
    assert!(shown(&rig, Role::List, MEMORY_LABEL));
    rig.harness.get_by_label_contains("prefers tabs");
    assert_eq!(memory_count(&rig), 2);

    // `/memory clear` only opens the confirmation; No keeps every memory.
    rig.slash("/memory clear");
    assert!(shown(&rig, Role::Dialog, MEMORY_CLEAR_TITLE));
    assert_eq!(memory_count(&rig), 2);
    rig.harness.get_by_label(NO).click();
    rig.harness.run();
    rig.harness.run();
    assert!(!shown(&rig, Role::Dialog, MEMORY_CLEAR_TITLE));
    assert_eq!(memory_count(&rig), 2);

    // The dialog owns the keys, as in the TUI, even with the prompt
    // focused: Right moves to Yes, Enter clears, and the panel shows its
    // empty state.
    rig.slash("/memory clear");
    rig.harness.get_by_label(YES);
    tap(&mut rig, egui::Key::ArrowRight);
    assert!(rig.harness.state().app().memory_clear_confirm_is_yes());
    tap(&mut rig, egui::Key::Enter);
    rig.harness.run();
    assert!(!shown(&rig, Role::Dialog, MEMORY_CLEAR_TITLE));
    assert_eq!(memory_count(&rig), 0);
    rig.harness
        .get_by_label_contains("(no memories for this project)");
}

#[test]
fn test_memory_panel_delete_requires_confirm() {
    let mut fact = 0;
    let mut rig = rig(|_, app| {
        remember(app, "prefers tabs", "preference");
        fact = remember(app, "uses nextest", "fact");
    });
    focus_prompt(&mut rig);
    alt(&mut rig, egui::Key::M, "m");

    // Rows are ordered by category: `fact` first. A click selects the
    // second row through app-core, Up moves back to the first.
    rig.harness.get_by_label_contains("prefers tabs").click();
    rig.harness.run();
    assert_eq!(rig.harness.state().app().memory_cursor, 1);
    tap(&mut rig, egui::Key::ArrowUp);
    assert_eq!(rig.harness.state().app().memory_cursor, 0);

    // Delete on the empty prompt asks first; Esc keeps the memory.
    tap(&mut rig, egui::Key::Delete);
    assert!(shown(&rig, Role::Dialog, MEMORY_DELETE_TITLE));
    rig.harness
        .get_by_label_contains(&format!("Memory ID: {fact}"));
    tap(&mut rig, egui::Key::Escape);
    assert!(!shown(&rig, Role::Dialog, MEMORY_DELETE_TITLE));
    assert_eq!(memory_count(&rig), 2);

    // Delete, then Enter, deletes it.
    tap(&mut rig, egui::Key::Delete);
    tap(&mut rig, egui::Key::Enter);
    rig.harness.run();
    assert_eq!(memory_count(&rig), 1);
    assert_eq!(rig.harness.state().app().session.input, "");
}

#[test]
fn test_memory_panel_enter_opens_the_memory() {
    let mut id = 0;
    let mut rig = rig(|_, app| id = remember(app, "uses nextest for tests", "fact"));
    focus_prompt(&mut rig);
    alt(&mut rig, egui::Key::M, "m");

    // Enter on the empty prompt opens the selected memory (app-core's
    // OpenMemoryView); Esc closes it.
    tap(&mut rig, egui::Key::Enter);
    let title = memory_view_title(id, "fact");
    assert!(shown(&rig, Role::Dialog, &title));
    rig.harness.get_by_label("Category: fact");
    tap(&mut rig, egui::Key::Escape);
    rig.harness.run();
    assert!(rig.harness.state().app().memory_view.is_none());
    assert!(!shown(&rig, Role::Dialog, &title));
}

#[test]
fn test_agents_panel_keys_select_and_kill_confirm() {
    let mut rig = rig(|rt, app| {
        let sid = start_session(rt, app);
        app.active_tasks.push(task("task-alpha-0001", &sid));
        app.active_tasks.push(task("task-beta-00002", &sid));
    });
    focus_prompt(&mut rig);

    alt(&mut rig, egui::Key::A, "a");
    assert!(rig.harness.state().app().show_agents_window);
    assert!(shown(&rig, Role::Table, AGENTS_LABEL));
    assert_eq!(
        rig.harness
            .state()
            .layout()
            .panel_state(&PaneId::ActiveAgents),
        PanelState::Front
    );
    rig.harness.get_by_label_contains("task-alpha-0001");

    // The window owns its keys, as the TUI's popup does: Down moves the
    // cursor, Space does not type, Delete arms app-core's confirmation.
    tap(&mut rig, egui::Key::ArrowDown);
    tap(&mut rig, egui::Key::ArrowDown);
    assert_eq!(rig.harness.state().app().agents_cursor_subagent, Some(1));
    tap(&mut rig, egui::Key::Delete);
    let title = row_action_title(rig.harness.state().app()).expect("confirm open");
    assert_eq!(title, "Kill sub-agent?");
    assert!(shown(&rig, Role::Dialog, &title));

    // No (the default) cancels; the task is untouched.
    rig.harness.get_by_label(NO).click();
    rig.harness.run();
    rig.harness.run();
    assert!(rig.harness.state().app().row_action_confirm.is_none());
    assert_eq!(rig.harness.state().app().active_tasks.len(), 2);
    assert_eq!(rig.harness.state().app().session.input, "");

    // A click on a row selects it through app-core.
    rig.harness.get_by_label_contains("task-alpha-0001").click();
    rig.harness.run();
    assert_eq!(rig.harness.state().app().agents_cursor_subagent, Some(0));

    // Esc closes the window.
    tap(&mut rig, egui::Key::Escape);
    rig.harness.run();
    assert!(!rig.harness.state().app().show_agents_window);
    assert!(!shown(&rig, Role::Table, AGENTS_LABEL));
}

#[test]
fn test_teams_panel_keys_select_and_shutdown_confirm() {
    let mut rig = rig(|rt, app| {
        start_session(rt, app);
        for (id, name) in [("agent-one", "ada"), ("agent-two", "bob")] {
            let mut member = TeamMember::new(name, id, "general");
            member.status = MemberStatus::Working;
            app.session.team_members.push(member);
        }
    });
    focus_prompt(&mut rig);

    alt(&mut rig, egui::Key::T, "t");
    assert!(shown(&rig, Role::Table, TEAMS_LABEL));
    rig.harness.get_by_label_contains("lead + 2 teammates");
    rig.harness.get_by_label_contains("bob-gent-two");

    // A click selects a teammate through app-core; Delete arms the
    // shutdown confirmation for it.
    rig.harness.get_by_label_contains("bob-gent-two").click();
    rig.harness.run();
    assert_eq!(rig.harness.state().app().teams_cursor_teammate, Some(1));
    tap(&mut rig, egui::Key::ArrowUp);
    assert_eq!(rig.harness.state().app().teams_cursor_teammate, Some(0));
    tap(&mut rig, egui::Key::Delete);
    let title = row_action_title(rig.harness.state().app()).expect("confirm open");
    assert_eq!(title, "Shut down teammate?");
    assert!(shown(&rig, Role::Dialog, &title));
    tap(&mut rig, egui::Key::Escape);
    rig.harness.run();
    assert!(rig.harness.state().app().row_action_confirm.is_none());
    assert_eq!(rig.harness.state().app().session.team_members.len(), 2);
}

#[test]
fn test_agents_panel_cursor_stops_at_the_last_sub_agent() {
    let mut rig = rig(|rt, app| {
        let sid = start_session(rt, app);
        app.active_tasks.push(task("task-alpha-0001", &sid));
        let now = chrono::Utc::now();
        app.agents_history
            .push_back(codemason_app_core::app::CompletedTaskRow {
                id: "task-done-00003".to_string(),
                child_session_id: "task-done-00003-session".to_string(),
                agent_name: "explore".to_string(),
                status: TaskStatus::Completed,
                background: false,
                detached: false,
                created_at: now,
                completed_at: now,
                report_status: ReportStatus::default(),
                output_file: None,
            });
    });
    focus_prompt(&mut rig);
    alt(&mut rig, egui::Key::A, "a");
    rig.harness
        .get_by_label_contains("Completed (newest first)");

    // Down past the only sub-agent stays on it: the history rows are not
    // cursor rows.
    for _ in 0..4 {
        tap(&mut rig, egui::Key::ArrowDown);
    }
    let app = rig.harness.state().app();
    assert_eq!(app.agents_subagent_task_ids, vec!["task-alpha-0001"]);
    assert_eq!(app.agents_cursor_subagent, Some(0));
}

#[test]
fn test_agents_panel_shown_keeps_the_prompt_for_paste_ime_and_typing() {
    let mut rig = rig(|rt, app| {
        let sid = start_session(rt, app);
        app.active_tasks.push(task("task-alpha-0001", &sid));
        app.active_tasks.push(task("task-beta-00002", &sid));
    });
    focus_prompt(&mut rig);
    alt(&mut rig, egui::Key::A, "a");
    assert!(rig.harness.state().app().show_agents_window);
    assert_eq!(
        rig.harness
            .state()
            .layout()
            .panel_state(&PaneId::ActiveAgents),
        PanelState::Front
    );

    // A paste, an IME commit and plain typing each reach the prompt once,
    // as the TUI's input editor keeps them while the popup is open.
    rig.harness.event(egui::Event::Paste("abc".to_string()));
    rig.harness.run();
    assert_eq!(rig.harness.state().app().session.input, "abc");
    rig.harness.event(egui::Event::Ime(egui::ImeEvent::Commit(
        "\u{4f60}".to_string(),
    )));
    rig.harness.run();
    assert_eq!(rig.harness.state().app().session.input, "abc\u{4f60}");
    press(&mut rig, egui::Key::X, egui::Modifiers::NONE, Some("x"));
    assert_eq!(rig.harness.state().app().session.input, "abc\u{4f60}x");

    // The window's own keys still drive its cursor, and Space types nothing.
    tap(&mut rig, egui::Key::ArrowDown);
    tap(&mut rig, egui::Key::ArrowDown);
    assert_eq!(rig.harness.state().app().agents_cursor_subagent, Some(1));
    press(&mut rig, egui::Key::Space, egui::Modifiers::NONE, Some(" "));
    assert_eq!(rig.harness.state().app().session.input, "abc\u{4f60}x");
    assert_eq!(
        rig.harness
            .state()
            .layout()
            .panel_state(&PaneId::ActiveAgents),
        PanelState::Front
    );
}
