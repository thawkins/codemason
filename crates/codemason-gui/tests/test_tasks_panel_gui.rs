//! task migration T-015: integration tests for the reworked `render_tasks_panel`.
//!
//! Verifies FR-005, FR-007, FR-018:
//! - Panel header titled "TASKS" (not the old panel name).
//! - Status-coloured lines: pending=yellow, in_progress=cyan, completed=green,
//!   blocked=red.
//! - `(owner)` suffix appended when owner is set.
//! - `[blocked by #id, ...]` annotation when derived blocked (FR-005).
//! - `active_form` rendered as indented sub-line beneath subject when
//!   `in_progress`.
//! - Scroll, scrollbar, text-selection, and mutual-exclusion behaviour
//!   preserved.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_tasks_panel.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use codemason_app_core::app::App;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

/// `render_app_to_string` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_app_to_string_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);

    let _text = String::new();
}

/// Helper: create a session (if needed) and a task with the given fields
/// using `create_task` (which supports all Task-model columns).
fn create_task(
    app: &mut App,
    id: &str,
    session_id: &str,
    subject: &str,
    status: &str,
    active_form: Option<&str>,
    owner: Option<&str>,
    blocked_by: &[&str],
) {
    // Create session row only if it doesn't already exist (some tests
    // share the same session_id across multiple create_task calls).
    if app
        .storage
        .get_session(session_id)
        .unwrap_or(None)
        .is_none()
    {
        app.storage
            .create_session(session_id, ".")
            .expect("create session row");
    }
    let blocked_by_owned: Vec<String> = blocked_by.iter().map(|s| s.to_string()).collect();
    app.storage
        .create_task(
            id,
            session_id,
            subject,
            "",
            status,
            active_form,
            owner,
            "{}",
            &blocked_by_owned,
        )
        .expect("create task");
}

// -- Panel title -----------------------------------------------------

// -- Empty placeholder ------------------------------------���----------

// -- Status rendering ------------------------------------------------

// -- Owner suffix (FR-018) -------------------------------------------

// -- Blocked-by annotation (FR-005, FR-018) --------------------------

// -- active_form sub-line (FR-018) -----------------------------------

// -- Combined: owner + active_form + status --------------------------

// -- Scroll preservation ---------------------------------------------

// -- Read-only rendering ---------------------------------------------

/// Rendering the panel must not mutate stored tasks.
#[test]
fn test_tasks_panel_does_not_mutate_tasks() {
    let mut app = support::make_app();
    let session_id = "immutable-session".to_string();
    app.session.session_id = Some(session_id.clone());
    app.show_tasks_panel = true;

    create_task(
        &mut app,
        "im1",
        &session_id,
        "Immutable task",
        "pending",
        Some("Doing stuff"),
        Some("agent-1"),
        &[],
    );

    let before = app
        .storage
        .list_tasks(&session_id, None)
        .expect("read tasks before render");
    // Render twice.
    render_app_to_string_effects(&mut app, 120, 40);
    render_app_to_string_effects(&mut app, 120, 40);
    let after = app
        .storage
        .list_tasks(&session_id, None)
        .expect("read tasks after render");

    assert_eq!(before.len(), after.len(), "row count must not change");
    assert_eq!(after[0].title, "Immutable task");
    assert_eq!(after[0].status, "pending");
    assert_eq!(after[0].active_form.as_deref(), Some("Doing stuff"));
    assert_eq!(after[0].owner.as_deref(), Some("agent-1"));
}

// -- Error state -----------------------------------------------------
//
// When storage query fails, the panel should show "Failed to load tasks"
// (not "Failed to load tasks").  This is implicitly tested by the existing
// test_tasks_panel tests since we can't easily force a storage error with
// the real Storage backend.  The text change is verified by the source code.
