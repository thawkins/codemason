//! Tests for the docked window and its side panels (`codemason_gui::panels`):
//! the sessions sidebar, the log panel, the status bar, the dock's
//! visibility sync with the App's panel flags and the layout file.
//!
//! Every test drives the whole [`Shell`] frame, so the dock, the key router
//! and app-core's key handling are exercised together. GUI assertions use
//! egui_kittest accessibility queries only (no snapshots). Each test owns a
//! tokio runtime that it does not enter while the harness runs, because the
//! shell drives `App::handle_key` with `Handle::block_on`. Nothing sleeps,
//! and the only file written is under `target/temp`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use codemason_agent::event::Event;
use codemason_app_core::app::App;
use codemason_app_core::log_channel::{TuiLogRecord, TuiLogSender, tui_log_channel};
use codemason_gui::Shell;
use codemason_gui::dialogs::session_menu::SESSION_MENU_TITLE;
use codemason_gui::dock::{PaneId, default_layout, load_dock, load_layout, save_layout};
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::log::LOG_LABEL;
use codemason_gui::panels::prompt::PROMPT_LABEL;
use codemason_gui::panels::sessions::SESSIONS_LABEL;
use codemason_gui::panels::status_bar::STATUS_BAR_LABEL;
use craft_keymap::Platform;
use craft_ui::docking::{Location, PanelState, Placement, Zone};
use craft_ui::layout::SplitSize;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tokio::sync::mpsc::UnboundedSender;

/// A running window: the runtime, the agent-event and log senders, and the
/// harness over the shell.
struct Rig {
    _runtime: tokio::runtime::Runtime,
    events: UnboundedSender<Event>,
    logs: TuiLogSender,
    harness: Harness<'static, Shell>,
}

/// Build `app` on `runtime` and let `configure` change it (outside the
/// runtime context, so it may `block_on` the App's async commands).
fn build_app(runtime: &tokio::runtime::Runtime, configure: impl FnOnce(&mut App)) -> App {
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    configure(&mut app);
    app
}

/// A shell over an App `configure` prepared, with sources attached and,
/// when `layout_file` is set, the dock layout loaded from and saved to it.
fn rig_with(
    configure: impl FnOnce(&tokio::runtime::Runtime, &mut App),
    layout_file: Option<PathBuf>,
) -> Rig {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = build_app(&runtime, |app| configure(&runtime, app));
    let (events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (logs_tx, logs) = tui_log_channel(64);
    let mut shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    if let Some(path) = layout_file {
        shell = shell.with_layout_file(path);
    }
    // A new session's transcript starts with the block-character logo
    // (`announce_session_startup`), which egui's bundled fonts cannot draw;
    // these tests are about the panels, so the logo may draw as tofu.
    let harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    Rig {
        _runtime: runtime,
        events: events_tx,
        logs: logs_tx,
        harness,
    }
}

/// [`rig_with`] without a layout file.
fn rig(configure: impl FnOnce(&tokio::runtime::Runtime, &mut App)) -> Rig {
    rig_with(configure, None)
}

/// Create two stored sessions, `a` then `b`, leaving `b` focused.
fn two_sessions(runtime: &tokio::runtime::Runtime, app: &mut App) -> (String, String) {
    app.sidebar.visible = true;
    runtime.block_on(app.execute_slash_command("/session list"));
    let a = app.session.session_id.clone().expect("session A");
    runtime.block_on(app.execute_slash_command("/session new"));
    let b = app.session.session_id.clone().expect("session B");
    assert_ne!(a, b);
    (a, b)
}

/// The `untitled <tail>` title the sidebar shows for an untitled session.
fn untitled(id: &str) -> String {
    format!("untitled {}", &id[id.len() - 8..])
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
fn press(
    harness: &mut Harness<'static, Shell>,
    key: egui::Key,
    modifiers: egui::Modifiers,
    text: Option<&str>,
) {
    let input = harness.input_mut();
    input.events.push(key_event(key, true, modifiers));
    if let Some(text) = text {
        input.events.push(egui::Event::Text(text.to_string()));
    }
    input.events.push(key_event(key, false, modifiers));
    harness.run();
}

/// Press Alt+`key` (egui-winit types its letter with Alt on Linux).
fn alt(harness: &mut Harness<'static, Shell>, key: egui::Key, letter: &str) {
    press(harness, key, egui::Modifiers::ALT, Some(letter));
}

/// Focus the prompt editor and draw a frame.
fn focus_prompt(harness: &mut Harness<'static, Shell>) {
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();
}

/// Whether the sessions sidebar is drawn.
fn sessions_shown(harness: &Harness<'static, Shell>) -> bool {
    harness
        .query_by_role_and_label(Role::List, SESSIONS_LABEL)
        .is_some()
}

/// Whether the log panel is drawn.
fn log_shown(harness: &Harness<'static, Shell>) -> bool {
    harness
        .query_by_role_and_label(Role::Log, LOG_LABEL)
        .is_some()
}

/// A scratch file under `target/temp` for test `name`, removed first.
fn scratch_file(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/codemason-gui-test-panels")
        .join(name);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("gui-layout.json");
    // A missing file is the expected start; any other error shows on read.
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn test_sessions_panel_select_switches_session() {
    let mut ids = None;
    let mut rig = rig(|rt, app| ids = Some(two_sessions(rt, app)));
    let (a, b) = ids.expect("sessions");
    rig.harness.run();
    assert!(sessions_shown(&rig.harness));
    assert_eq!(
        rig.harness.state().app().session.session_id,
        Some(b.clone())
    );

    rig.harness.get_by_label_contains(&untitled(&a)).click();
    rig.harness.run();

    assert_eq!(rig.harness.state().app().session.session_id, Some(a));
    // The row for `b` is still listed; the switch kept both sessions.
    rig.harness.get_by_label_contains(&untitled(&b));
}

#[test]
fn test_sessions_panel_right_click_opens_session_menu() {
    let mut ids = None;
    let mut rig = rig(|rt, app| ids = Some(two_sessions(rt, app)));
    let (a, _b) = ids.expect("sessions");
    rig.harness.run();

    rig.harness
        .get_by_label_contains(&untitled(&a))
        .click_secondary();
    rig.harness.run();

    assert!(rig.harness.state().app().sidebar.menu.is_some());
    assert!(
        rig.harness
            .query_by_role_and_label(Role::Dialog, SESSION_MENU_TITLE)
            .is_some()
    );
}

#[test]
fn test_sessions_panel_keys_move_and_m_opens_session_menu() {
    let mut ids = None;
    let mut rig = rig(|rt, app| ids = Some(two_sessions(rt, app)));
    let _ = ids.expect("sessions");
    rig.harness.run();
    focus_prompt(&mut rig.harness);

    // Alt+S on a shown sidebar gives it the keyboard, as in the TUI.
    alt(&mut rig.harness, egui::Key::S, "s");
    assert!(rig.harness.state().app().sidebar.focused);
    // The last row is a session; `m` opens its menu instead of typing.
    press(
        &mut rig.harness,
        egui::Key::End,
        egui::Modifiers::NONE,
        None,
    );
    press(
        &mut rig.harness,
        egui::Key::M,
        egui::Modifiers::NONE,
        Some("m"),
    );

    let app = rig.harness.state().app();
    assert!(app.sidebar.menu.is_some(), "m opens the session menu");
    assert_eq!(app.session.input, "", "the sidebar's keys never type");
}

#[test]
fn test_sessions_panel_alt_s_shows_focuses_and_hides_the_pane() {
    let mut rig = rig(|_, app| app.sidebar.visible = true);
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    assert!(sessions_shown(&rig.harness));

    alt(&mut rig.harness, egui::Key::S, "s");
    assert!(rig.harness.state().app().sidebar.focused);
    assert!(sessions_shown(&rig.harness));

    alt(&mut rig.harness, egui::Key::S, "s");
    assert!(!rig.harness.state().app().sidebar.visible);
    rig.harness.run();
    assert!(!sessions_shown(&rig.harness));
    assert!(!rig.harness.state().layout().contains(&PaneId::Sessions));

    alt(&mut rig.harness, egui::Key::S, "s");
    rig.harness.run();
    assert!(sessions_shown(&rig.harness));
    assert!(rig.harness.state().layout().contains(&PaneId::Sessions));
    assert_eq!(rig.harness.state().app().session.input, "");
}

#[test]
fn test_log_panel_shows_tracing_lines() {
    let mut rig = rig(|_, app| app.show_log = true);
    rig.logs
        .send(TuiLogRecord {
            level: tracing::Level::WARN,
            message: "disk nearly full".to_string(),
        })
        .expect("send log record");
    rig.logs
        .send(TuiLogRecord {
            level: tracing::Level::INFO,
            message: "hello from tracing".to_string(),
        })
        .expect("send log record");
    rig.harness.run();

    assert!(log_shown(&rig.harness));
    rig.harness.get_by_label_contains("WRN disk nearly full");
    rig.harness.get_by_label_contains("INF hello from tracing");
}

#[test]
fn test_log_panel_alt_l_shows_and_hides_the_pane() {
    let mut rig = rig(|_, _| {});
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    assert!(!log_shown(&rig.harness));
    assert!(!rig.harness.state().layout().contains(&PaneId::Log));

    alt(&mut rig.harness, egui::Key::L, "l");
    rig.harness.run();
    assert!(rig.harness.state().app().show_log);
    assert!(log_shown(&rig.harness));
    assert!(rig.harness.state().layout().contains(&PaneId::Log));

    alt(&mut rig.harness, egui::Key::L, "l");
    rig.harness.run();
    assert!(!rig.harness.state().app().show_log);
    assert!(!log_shown(&rig.harness));
    assert!(!rig.harness.state().layout().contains(&PaneId::Log));
    assert_eq!(rig.harness.state().app().session.input, "");
}

#[test]
fn test_status_bar_shows_model_and_cost() {
    let mut rig = rig(|_, app| {
        app.session.session_id = Some("session-status".to_string());
        app.session.selected_model = Some("openai/gpt-status-test".to_string());
    });
    rig.harness.run();
    rig.harness
        .get_by_role_and_label(Role::Status, STATUS_BAR_LABEL);
    rig.harness.get_by_label_contains("Model: ");
    rig.harness.get_by_label_contains("gpt-status-test");

    rig.events
        .send(Event::RunCostSummary {
            session_id: "session-status".to_string(),
            model_id: "gpt-status-test".to_string(),
            input_tokens: 1200,
            output_tokens: 300,
            total_cost_usd: 0.0123,
            duration_ms: 1500,
        })
        .expect("send run cost");
    rig.harness.run();
    rig.harness.get_by_label_contains("$0.0123");
    rig.harness.get_by_label_contains("1200+300 tokens");
}

#[test]
fn test_dock_placeholder_panes_are_labelled() {
    let mut rig = rig(|_, _| {});
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    assert!(!rig.harness.state().layout().contains(&PaneId::Profile));

    // Alt+P turns the profile panel on; its pane comes back in front of
    // its tab group and shows its (labelled) placeholder.
    alt(&mut rig.harness, egui::Key::P, "p");
    rig.harness.run();

    assert!(rig.harness.state().app().show_profile);
    assert!(rig.harness.state().layout().contains(&PaneId::Profile));
    rig.harness.get_by_label_contains("Profile panel");
}

#[test]
fn test_dock_closing_a_pane_clears_its_app_flag() {
    let mut rig = rig(|_, app| app.show_log = true);
    rig.harness.run();
    assert!(rig.harness.state().layout().contains(&PaneId::Log));

    // The dock's own close control: the Log tab's context menu, `Close
    // Panel`. The shell hides the pane through app-core's Alt+L toggle.
    rig.harness
        .get_by_role_and_label(Role::Tab, LOG_LABEL)
        .click_secondary();
    rig.harness.run();
    rig.harness.get_by_label("Close Panel").click();
    rig.harness.run();
    rig.harness.run();

    let app = rig.harness.state().app();
    assert!(!app.show_log);
    assert_eq!(app.status, "log panel hidden", "app-core's toggle ran");
    assert!(!rig.harness.state().layout().contains(&PaneId::Log));
    assert!(!log_shown(&rig.harness));
}

#[test]
fn test_dock_closing_the_sessions_tab_hides_the_sidebar() {
    let mut rig = rig(|_, app| app.sidebar.visible = true);
    rig.harness.run();

    rig.harness
        .get_by_role_and_label(Role::Tab, SESSIONS_LABEL)
        .click_secondary();
    rig.harness.run();
    rig.harness.get_by_label("Close Panel").click();
    rig.harness.run();

    assert!(!rig.harness.state().app().sidebar.visible);
    assert!(!rig.harness.state().layout().contains(&PaneId::Sessions));
    assert!(!sessions_shown(&rig.harness));
}

#[test]
fn test_sessions_panel_alt_s_fronts_the_sidebar_over_profile() {
    let mut ids = None;
    let mut rig = rig(|rt, app| ids = Some(two_sessions(rt, app)));
    let _ = ids.expect("sessions");
    rig.harness.run();
    focus_prompt(&mut rig.harness);

    // Alt+P puts Profile in front of Sessions (same tab group).
    alt(&mut rig.harness, egui::Key::P, "p");
    rig.harness.run();
    assert_eq!(
        rig.harness.state().layout().panel_state(&PaneId::Profile),
        PanelState::Front
    );
    assert!(!sessions_shown(&rig.harness));

    // Alt+S gives the sidebar the keyboard: it comes to the front.
    alt(&mut rig.harness, egui::Key::S, "s");
    rig.harness.run();
    assert!(rig.harness.state().app().sidebar.focused);
    assert_eq!(
        rig.harness.state().layout().panel_state(&PaneId::Sessions),
        PanelState::Front
    );
    assert!(sessions_shown(&rig.harness));

    // And it receives the keys: `m` on the last row opens the session menu.
    press(
        &mut rig.harness,
        egui::Key::End,
        egui::Modifiers::NONE,
        None,
    );
    press(
        &mut rig.harness,
        egui::Key::M,
        egui::Modifiers::NONE,
        Some("m"),
    );
    assert!(rig.harness.state().app().sidebar.menu.is_some());
    assert_eq!(rig.harness.state().app().session.input, "");
}

#[test]
fn test_dock_hidden_pane_returns_to_its_place_after_restart() {
    let path = scratch_file("hidden-after-restart");
    // First run: the log is shown, the user moves nothing, then hides it.
    let before = {
        let mut rig = rig_with(|_, app| app.show_log = true, Some(path.clone()));
        rig.harness.run();
        focus_prompt(&mut rig.harness);
        let before = rig
            .harness
            .state()
            .layout()
            .location(&PaneId::Log)
            .expect("log docked");
        alt(&mut rig.harness, egui::Key::L, "l");
        rig.harness.run();
        assert!(!rig.harness.state().layout().contains(&PaneId::Log));
        before
    };
    let (_, hidden) = load_dock(&path);
    assert!(
        hidden.contains_key(&PaneId::Log),
        "the hidden place is saved"
    );

    // Second run: showing the log puts it back where it was.
    let mut rig = rig_with(|_, _| {}, Some(path));
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    alt(&mut rig.harness, egui::Key::L, "l");
    rig.harness.run();
    assert_eq!(
        rig.harness.state().layout().location(&PaneId::Log).ok(),
        Some(before)
    );
}

/// Show the log and then the sidebar with their shortcuts, starting from
/// every side flag off, and return where each pane lands.
fn show_log_and_sidebar(layout_file: Option<PathBuf>) -> Vec<Option<Location<PaneId>>> {
    let mut rig = rig_with(|_, app| app.sidebar.visible = false, layout_file);
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    alt(&mut rig.harness, egui::Key::L, "l");
    rig.harness.run();
    alt(&mut rig.harness, egui::Key::S, "s");
    rig.harness.run();
    let layout = rig.harness.state().layout();
    [PaneId::Log, PaneId::Sessions]
        .iter()
        .map(|pane| layout.location(pane).ok())
        .collect()
}

#[test]
fn test_dock_fresh_start_shows_panes_at_their_default_places() {
    // Without a file the hidden places come straight from the default
    // layout: this is where each pane is designed to appear.
    let designed = show_log_and_sidebar(None);
    // The log goes back to the default right-hand column, beside the
    // transcript at the default 3:1 split, not into a new edge frame.
    let log = designed[0].as_ref().expect("log docked");
    assert_eq!(log.anchor, Some(PaneId::Messages));
    assert_eq!(log.placement, Placement::Split(Zone::Right));
    assert_eq!(log.size, Some(SplitSize::Ratio(0.75)));
    let sessions = designed[1].as_ref().expect("sessions docked");
    assert_eq!(
        sessions.placement,
        Placement::Split(Zone::Left),
        "{sessions:?}"
    );

    // First run with a file and every side flag off: the pruned layout and
    // the hidden places are saved; the second run shows the panes.
    let path = scratch_file("fresh-start");
    {
        let mut rig = rig_with(|_, app| app.sidebar.visible = false, Some(path.clone()));
        rig.harness.run();
    }
    assert_eq!(show_log_and_sidebar(Some(path)), designed);
}

#[test]
fn test_dock_old_format_layout_file_loads() {
    let path = scratch_file("old-format");
    let mut old = default_layout();
    let mut hidden = codemason_gui::dock::HiddenPanes::new();
    codemason_gui::dock::toggle_pane(&mut old, &mut hidden, PaneId::Teams).expect("hide teams");
    // The format before hidden places were saved: the bare layout.
    save_layout(&path, &old).expect("write old format");

    let (layout, hidden) = load_dock(&path);
    assert_eq!(layout, old);
    assert!(hidden.is_empty());
    let rig = rig_with(|_, _| {}, Some(path));
    assert!(!rig.harness.state().layout().contains(&PaneId::Teams));
}

#[test]
fn test_dock_layout_file_saves_changes() {
    let path = scratch_file("saves-changes");
    let mut rig = rig_with(|_, _| {}, Some(path.clone()));
    rig.harness.run();
    focus_prompt(&mut rig.harness);
    assert!(!load_layout(&path).contains(&PaneId::Log));

    alt(&mut rig.harness, egui::Key::L, "l");
    rig.harness.run();

    assert!(load_layout(&path).contains(&PaneId::Log));
}
