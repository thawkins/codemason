//! Status-bar indicators of the GUI ([`codemason_gui::panels::status_bar`]).
//!
//! Ported from the terminal UI's status-bar rendering tests when the TUI was
//! removed (`test_autopilot_alt_o`, `test_codeindex_indicators`,
//! `test_backend_command` in the former `codemason-tui`): the same App state
//! must surface the same indicators in the GUI's status bar. Each test drives
//! the whole [`Shell`] frame and finds the indicator by its label.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::App;
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window: the runtime it was built on and the harness.
struct Rig {
    _runtime: tokio::runtime::Runtime,
    harness: Harness<'static, Shell>,
}

/// A shell over an App `configure` prepared, drawn once.
fn rig(configure: impl FnOnce(&tokio::runtime::Runtime, &mut App)) -> Rig {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    configure(&runtime, &mut app);
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(64);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    // The core draws glyphs (status icons, box drawing) that egui's bundled
    // fonts lack; these tests are about the status bar.
    let harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 800.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    let mut rig = Rig {
        _runtime: runtime,
        harness,
    };
    rig.harness.run();
    rig
}

/// Press Alt+O through app-core's key handler (the autopilot toggle).
fn press_alt_o(runtime: &tokio::runtime::Runtime, app: &mut App) {
    runtime.block_on(app.handle_key(KeyInput {
        key: Key::Char('o'),
        mods: Modifiers::ALT,
    }));
}

#[test]
fn test_status_bar_shows_autopilot_off_before_enabling() {
    let rig = rig(|_, app| assert!(!app.session.autopilot_enabled));
    rig.harness.get_by_label("AUTO [x]");
    assert!(rig.harness.query_by_label("AUTO [ok]").is_none());
}

#[test]
fn test_status_bar_shows_autopilot_on_after_alt_o() {
    let rig = rig(|runtime, app| {
        press_alt_o(runtime, app);
        assert!(app.session.autopilot_enabled, "Alt+O enables autopilot");
    });
    rig.harness.get_by_label("AUTO [ok]");
    assert!(rig.harness.query_by_label("AUTO [x]").is_none());
}

#[test]
fn test_status_bar_counts_down_the_autopilot_time_limit() {
    // Alt+O arms a 5000 s limit; a frame drawn moments after the start stamp
    // is still inside the first second, so the full 01:23:20 shows.
    let rig = rig(press_alt_o);
    rig.harness.get_by_label("[01:23:20]");
}

#[test]
fn test_status_bar_omits_the_countdown_without_a_time_limit() {
    let rig = rig(|runtime, app| {
        runtime.block_on(app.execute_slash_command("/autopilot on"));
        assert!(app.session.autopilot_enabled);
        assert!(app.session.autopilot_time_limit_secs.is_none());
    });
    rig.harness.get_by_label("AUTO [ok]");
    assert!(
        rig.harness.query_by_label_contains("[00:").is_none()
            && rig.harness.query_by_label_contains("[01:").is_none(),
        "an unbounded autopilot run has no countdown"
    );
}

#[test]
fn test_status_bar_shows_code_index_busy_indicators() {
    let rig = rig(|_, app| {
        app.code_index_busy = true;
        app.code_index_graph_busy = true;
    });
    rig.harness.get_by_label("BUSY:cdx-idx");
    rig.harness.get_by_label("BUSY:cdx-graph");
}

#[test]
fn test_status_bar_hides_code_index_indicators_when_idle() {
    let rig = rig(|_, app| {
        app.code_index_busy = false;
        app.code_index_graph_busy = false;
    });
    assert!(rig.harness.query_by_label("BUSY:cdx-idx").is_none());
    assert!(rig.harness.query_by_label("BUSY:cdx-graph").is_none());
}

#[test]
fn test_status_bar_surfaces_the_default_local_backend() {
    let rig = rig(|_, app| {
        assert_eq!(
            app.session.active_backend, "local",
            "FR-019 default backend"
        );
    });
    rig.harness.get_by_label("Backend:local");
}

#[test]
fn test_status_bar_shows_a_spinner_only_while_a_turn_runs() {
    use codemason_gui::panels::status_bar::BUSY_LABEL;
    use egui::accesskit::Role;

    let idle = rig(|_, _| {});
    assert!(idle.harness.query_by_label(BUSY_LABEL).is_none());

    // `rig` runs the harness to rest: the spinner must not keep it awake by
    // asking for an immediate repaint every frame.
    let busy = rig(|_, app| app.session.is_processing = true);
    busy.harness
        .get_by_role_and_label(Role::ProgressIndicator, BUSY_LABEL);
}
