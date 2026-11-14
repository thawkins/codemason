//! Tests that the shell runs app-core's background housekeeping pass
//! (`App::housekeeping`) every frame, so results that background jobs
//! deposit (here a finished compaction) are applied with no user input, and
//! that it keeps a wake scheduled for the next pass.
//!
//! GUI assertions use egui_kittest accessibility queries only. The test owns
//! a tokio runtime it does not enter while the harness runs, because the
//! shell drives the App with `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use codemason_agent::message::Message;
use codemason_app_core::app::{App, HOUSEKEEPING_INTERVAL};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The session the test's messages belong to.
const SESSION: &str = "session-housekeeping";

/// A shell over `app` with sources attached, and the runtime driving it.
fn rig(configure: impl FnOnce(&mut App)) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
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

/// Move the App's housekeeping gate back so the next frame's pass is due
/// (the test hook in place of waiting `HOUSEKEEPING_INTERVAL`).
fn make_housekeeping_due(app: &mut App) {
    app.jobs_last_poll = Instant::now()
        .checked_sub(HOUSEKEEPING_INTERVAL + Duration::from_millis(1))
        .expect("clock");
}

#[test]
fn test_shell_applies_a_finished_compaction_without_input() {
    let (_runtime, mut harness) = rig(|app| {
        app.session.messages = vec![Message::user_text(SESSION, "before compaction")];
    });
    harness.run();
    harness.get_by_label_contains("You: before compaction");

    // A compaction task finishes in the background: it only fills the slot.
    {
        let app = harness.state_mut().app_mut();
        app.session.compact_in_progress = true;
        *app.session.compact_result.lock().expect("compact slot") =
            Some(Ok(vec![Message::user_text(SESSION, "compacted history")]));
        make_housekeeping_due(app);
    }
    // No key, click or event: just the next frame.
    harness.run();

    harness.get_by_label_contains("You: compacted history");
    assert!(
        harness
            .query_by_label_contains("You: before compaction")
            .is_none()
    );
    let app = harness.state().app();
    assert!(!app.session.compact_in_progress);
    assert!(app.session.compact_result.lock().expect("slot").is_none());
}

#[test]
fn test_shell_schedules_a_wake_for_the_next_housekeeping_pass() {
    let (_runtime, mut harness) = rig(|_| {});
    harness.run();

    let delay = harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(
        delay <= HOUSEKEEPING_INTERVAL,
        "an idle shell must wake within {HOUSEKEEPING_INTERVAL:?}, got {delay:?}"
    );
}

#[test]
fn test_shell_logic_alone_runs_housekeeping_and_timers() {
    // eframe calls only `logic` while the window is minimised or hidden: no
    // frame is drawn, yet background results and timers must still land.
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        app.session.compact_in_progress = true;
        *app.session.compact_result.lock().expect("compact slot") =
            Some(Ok(vec![Message::user_text(
                SESSION,
                "compacted while hidden",
            )]));
        make_housekeeping_due(&mut app);
        app.status = "slash command done".to_string();
        app.arm_status_expiry();
        app.status_set_at = Instant::now().checked_sub(Duration::from_secs(10));
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let mut shell = Shell::new(app).with_sources(events, logs, runtime.handle().clone());
    let ctx = egui::Context::default();

    shell.logic(&ctx);

    let app = shell.app();
    assert!(!app.session.compact_in_progress, "housekeeping ran");
    assert_eq!(app.session.messages.len(), 1);
    assert_eq!(app.status, "ready", "the status timer ran");
    assert!(
        ctx.has_requested_repaint(),
        "a wake is kept for the next pass, so logic keeps running while hidden"
    );
}
