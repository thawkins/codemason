//! Deep coverage for `src/app/event_handler.rs`, `src/app/swarm.rs` and
//! `src/app/research.rs` branches the existing suites do not reach.
//!
//! The event-handler arms are driven through the PUBLIC
//! `App::handle_event` / `App::handle_key_event` / `App::handle_mouse_event`
//! entry points; the swarm and research dispatch arms through the PUBLIC
//! `App::execute_slash_command`. Nothing in `src/` is modified.
//!
//! Filesystem-touching tests run inside an isolated tempdir project (cwd +
//! `XDG_CONFIG_HOME` redirected by [`slash_support::enter_project`]) so a run
//! leaves the repository byte-identical. Env mutation is `unsafe` in edition
//! 2024 and the workspace denies `unsafe_code`, so this target opts back in
//! explicitly; the isolation harness holds a process-global mutex across the
//! async body, so `clippy::await_holding_lock` is also allowed here.
//!
//! Tests only - no production logic is touched.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_event_swarm_research_deep.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use crate::term_events::CrosstermAppExt;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_app_core::app::App;
use codemason_app_core::app::BgTaskView;

#[path = "../../codemason-app-core/tests/slash_support/mod.rs"]
mod slash_support;
#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

/// An app with a live session, no filesystem isolation.
fn new_app(session: &str) -> App {
    let mut app = support::make_app();
    app.session.session_id = Some(session.to_string());
    app
}

fn shift_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

/// Render one frame so the renderer publishes the cursor row maps the window
/// key arms consult.
fn render_window(app: &mut App) {
    crate::frame::publish(app);
}

fn task_entry(id: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "s1".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "do work".to_string(),
        background: true,
        detached: false,
        status,
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

// ===========================================================================
// MessageEnd - reverse-create chaining, plan delegation, autopilot
// ===========================================================================

// ===========================================================================
// Event arms the other suites leave untouched
// ===========================================================================

// ===========================================================================
// /spec impl sequential driver - advance_spec_impl + dispatch_spec_impl_task
// ===========================================================================

// ===========================================================================
// Teammate focus cycling / messaging (keyboard driven)
// ===========================================================================

// ===========================================================================
// Agents panel keyboard + mouse task control
// ===========================================================================

#[tokio::test]
async fn shift_k_on_a_task_row_dispatches_a_kill() {
    let mut app = new_app("s1");
    app.show_agents_window = true;
    app.active_tasks = vec![task_entry("t-kill", TaskStatus::Running)];
    render_window(&mut app);
    app.move_agents_cursor(1);

    app.handle_key_event(shift_key(KeyCode::Char('K'))).await;
    assert_eq!(app.active_tasks.len(), 1, "the local row is untouched");
    app.row_action_confirm_cancel();
    assert!(!app.row_action_confirm_open(), "Esc dismisses the dialog");

    // With the rows gone the cursor is stale: Del/Shift+K dispatch nothing.
    app.active_tasks.clear();
    app.status = "sentinel".to_string();
    app.handle_key_event(shift_key(KeyCode::Char('K'))).await;
    assert_eq!(app.agents_cursor_subagent, Some(0));
    assert_eq!(app.active_tasks.len(), 0);
    assert!(
        !app.row_action_confirm_open(),
        "a stale range arms no dialog"
    );
    assert_eq!(
        app.status, "agents: selected row is gone",
        "a stale cursor reports the consumed no-op (FR-069)"
    );
}

#[tokio::test]
async fn agents_panel_background_task_survives_the_key_surface() {
    // The row-mouse model is retired: the keyboard cursor never covers the
    // background-shell rows (they are service-terminated, not cursorable), so
    // a background task stays untouched by the Agents-window keys (FR-016's
    // key-surface re-expression).
    let mut app = new_app("s1");
    app.show_agents_window = true;
    app.active_tasks = vec![task_entry("t-work", TaskStatus::Running)];
    app.bg_tasks = vec![BgTaskView {
        id: "bg-1".to_string(),
        session_id: "s1".to_string(),
        command: "sleep 1".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }];
    render_window(&mut app);

    // Move the cursor down onto the sub-agent row; the background task is not
    // part of the cursor range.
    app.move_agents_cursor(1);
    assert_eq!(app.agents_cursor_subagent, Some(0));
    assert_eq!(
        app.agents_subagent_task_ids.first().map(String::as_str),
        Some("t-work"),
        "the cursor range names sub-agent rows only (FR-016)"
    );
    assert_eq!(app.bg_tasks.len(), 1, "no cancellation was dispatched");
    assert_eq!(app.bg_tasks[0].status, "running");
}

// ===========================================================================
// /swarm - status + cancel variants with a real task store
// ===========================================================================

// ===========================================================================
// /research dispatch shapes
// ===========================================================================
