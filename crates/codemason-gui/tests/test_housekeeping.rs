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

/// The delay of the next repaint the last frame asked for.
fn repaint_delay(harness: &Harness<'static, Shell>) -> Duration {
    harness.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
}

#[test]
fn test_idle_shell_requests_no_repaint() {
    let (_runtime, mut harness) = rig(|app| {
        app.status_set_at = None;
        // The sidebar's request-count sweep is a periodic step (below).
        app.sidebar.visible = false;
    });
    harness.run();

    assert!(
        !harness.state().app().has_pending_work(),
        "nothing outstanding"
    );
    assert_eq!(
        repaint_delay(&harness),
        Duration::MAX,
        "an idle shell sleeps until input or the next timer"
    );

    // With the sidebar shown, the only wake is its next request-count sweep.
    {
        let app = harness.state_mut().app_mut();
        app.sidebar.visible = true;
        app.last_request_sweep = Some(Instant::now());
    }
    // Showing the pane repaints at once; then the shell settles.
    harness.run();
    let delay = repaint_delay(&harness);
    assert!(
        delay > HOUSEKEEPING_INTERVAL && delay <= Duration::from_secs(20),
        "the sweep wakes, not the 200 ms pass: {delay:?}"
    );
}

#[test]
fn test_pending_job_keeps_waking_and_its_result_lands_without_input() {
    let (_runtime, mut harness) = rig(|app| {
        app.status_set_at = None;
        app.sidebar.visible = false;
        app.session.messages = vec![Message::user_text(SESSION, "before compaction")];
    });
    harness.run();
    assert_eq!(repaint_delay(&harness), Duration::MAX);

    // A compaction job starts: it holds its result slot while it runs.
    let slot = {
        let app = harness.state_mut().app_mut();
        app.session.compact_in_progress = true;
        std::sync::Arc::clone(&app.session.compact_result)
    };
    make_housekeeping_due(harness.state_mut().app_mut());
    harness.step();
    assert!(harness.state().app().has_pending_work());
    assert!(
        repaint_delay(&harness) <= HOUSEKEEPING_INTERVAL,
        "a running job keeps the housekeeping wake, got {:?}",
        repaint_delay(&harness)
    );

    // The job finishes: it fills the slot and lets go of it. The next wake
    // applies the result with no input, and the shell goes back to sleep.
    *slot.lock().expect("compact slot") =
        Some(Ok(vec![Message::user_text(SESSION, "compacted history")]));
    drop(slot);
    make_housekeeping_due(harness.state_mut().app_mut());
    harness.step();
    harness.get_by_label_contains("You: compacted history");
    harness.run();
    assert!(!harness.state().app().has_pending_work());
    assert_eq!(repaint_delay(&harness), Duration::MAX);
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
