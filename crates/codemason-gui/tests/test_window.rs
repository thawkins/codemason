//! Tests for the window lifecycle (`codemason_gui::platform::window`): the
//! close confirmation while an agent turn runs, and the quit steps a close
//! shares with `/quit`.
//!
//! GUI assertions use egui_kittest accessibility queries. A close request is
//! injected as the `ViewportEvent::Close` eframe delivers for the close
//! button, Cmd+Q or the OS. The test owns a tokio runtime it does not enter
//! while the harness runs, because the shell drives the App with
//! `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use codemason_app_core::app::App;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::platform::window::{CANCEL_QUIT, CONFIRM_QUIT, QUIT_TITLE};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A shell over `app` with sources attached, and the runtime driving it.
fn rig(configure: impl FnOnce(&mut App)) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some("session-window".to_string());
        configure(&mut app);
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app).with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

/// Queue the close request eframe delivers when the user closes the window.
fn request_close(harness: &mut Harness<'static, Shell>) {
    harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
}

/// The commands the last frame sent to the root window.
fn commands(harness: &Harness<'static, Shell>) -> Vec<egui::ViewportCommand> {
    harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|output| output.commands.clone())
        .unwrap_or_default()
}

/// Whether the quit confirmation is shown.
fn prompt_shown(harness: &Harness<'static, Shell>) -> bool {
    harness
        .query_by_role_and_label(Role::Dialog, QUIT_TITLE)
        .is_some()
}

/// A per-test scratch directory under `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_window")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: a fresh directory per run
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn test_close_request_with_running_turn_prompts() {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let (_runtime, mut harness) = rig(move |app| {
        app.session.is_processing = true;
        app.session.cancel_flag = Some(flag);
    });
    harness.run();

    // The close is held and the confirmation shown; the turn keeps running.
    request_close(&mut harness);
    harness.step();
    harness.step();
    assert!(prompt_shown(&harness), "a running turn asks before closing");
    assert!(harness.state().app().is_running);
    assert!(!cancel.load(Ordering::Relaxed), "nothing cancelled yet");

    // A second close request while it is shown keeps one dialog.
    request_close(&mut harness);
    harness.step();
    assert!(commands(&harness).contains(&egui::ViewportCommand::CancelClose));
    harness.get_by_role_and_label(Role::Dialog, QUIT_TITLE);

    // Cancel keeps the window and the turn.
    harness.get_by_label(CANCEL_QUIT).click();
    harness.step();
    harness.step();
    assert!(!prompt_shown(&harness));
    assert!(harness.state().app().is_running);
    assert!(!cancel.load(Ordering::Relaxed));
    assert!(!commands(&harness).contains(&egui::ViewportCommand::Close));

    // Asked again and confirmed: the quit path runs (the turn is cancelled)
    // and the window is closed.
    request_close(&mut harness);
    harness.step();
    harness.step();
    harness.get_by_label(CONFIRM_QUIT).click();
    harness.step();
    // The click quits in the frame's drawing half; the next frame's logic
    // runs the exit steps and closes the window.
    harness.step();
    assert!(!harness.state().app().is_running, "confirming quits");
    assert!(
        cancel.load(Ordering::Relaxed),
        "the running turn is cancelled"
    );
    assert!(commands(&harness).contains(&egui::ViewportCommand::Close));

    // The close that follows goes through without asking again.
    request_close(&mut harness);
    harness.step();
    assert!(!commands(&harness).contains(&egui::ViewportCommand::CancelClose));
    assert!(!prompt_shown(&harness));
}

#[test]
fn test_close_request_idle_closes_immediately() {
    let dir = scratch("idle");
    let history = dir.join("history");
    let path = history.clone();
    let (_runtime, mut harness) = rig(move |app| {
        // A typed line whose debounced flush has not run yet.
        app.history_file_path = Some(path);
        app.input_history = vec!["typed before closing".to_string()];
        app.history_dirty = true;
        app.history_save_deadline =
            Some(std::time::Instant::now() + std::time::Duration::from_hours(1));
    });
    harness.run();

    request_close(&mut harness);
    harness.step();

    assert!(!prompt_shown(&harness), "no turn runs, so no question");
    assert!(!harness.state().app().is_running, "the close quits the app");
    assert!(!commands(&harness).contains(&egui::ViewportCommand::CancelClose));
    let saved = std::fs::read_to_string(&history).expect("history saved on quit");
    assert!(saved.contains("typed before closing"), "got {saved:?}");
}

#[test]
fn test_quit_command_with_running_turn_closes_without_prompt() {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let (runtime, mut harness) = rig(move |app| {
        app.session.is_processing = true;
        app.session.cancel_flag = Some(flag);
    });
    harness.run();

    // `/quit` is its own confirmation, as Ctrl+C then Ctrl+D is.
    runtime.block_on(harness.state_mut().app_mut().execute_slash_command("/quit"));
    harness.step();
    assert!(commands(&harness).contains(&egui::ViewportCommand::Close));
    assert!(
        cancel.load(Ordering::Relaxed),
        "the quit path cancels the turn"
    );

    // eframe turns the Close into a close request: no dialog, no cancel.
    request_close(&mut harness);
    harness.step();
    harness.step();
    assert!(!prompt_shown(&harness));
    assert!(!commands(&harness).contains(&egui::ViewportCommand::CancelClose));
}
