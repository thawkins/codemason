//! Tests for the Agents table body (spec `agentui` T-003).
//!
//! Covers the column header (FR-009), the primary row (FR-010), the
//! ancestor-indented sub-agent tree (FR-011), sub-agent row content (FR-012),
//! the `[C]` / `[T]` badges and their derivation (FR-013, FR-062), the status
//! presentation (FR-014), the pinned `[stop]` / `[x]` control columns (FR-015),
//! the background shell section (FR-016), the single glyph source (NFR-003) and
//! character-safe truncation (NFR-004).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_agents_window_table.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_app_core::app::App;

use support::make_app;

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_agents_window = true;
    crate::frame::publish(app);
}

/// Build a [`TaskEntry`] for the Agents table.
fn task(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background,
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

/// Build a background shell task view.
fn bg_task(id: &str, command: &str, status: &str) -> codemason_app_core::app::BgTaskView {
    codemason_app_core::app::BgTaskView {
        id: id.to_string(),
        session_id: "s1".to_string(),
        command: command.to_string(),
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// An app whose primary session is `s1` and whose agent is `general`.
fn app_with_primary() -> App {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app
}

// -- FR-009: header ---------------------------------------------------------

// -- FR-010: primary row ----------------------------------------------------

// -- FR-011: tree indentation ----------------------------------------------

/// The number of two-column ancestor segments painted before the branch glyph.

// -- FR-012: sub-agent row content ----------------------------------------

// -- FR-013 / FR-062: badges ----------------------------------------------

// -- FR-014: status presentation ------------------------------------------

// -- FR-015: control columns ----------------------------------------------

// (The old `test_control_hit_rects_match_the_painted_controls` was removed:
// the hit-rect state vectors are gone; its replacement,
// `test_render_publishes_row_maps_and_the_subagent_cursor_list`, pins the
// same alignment invariant against the published cursor row maps.)

/// A no-cursor render publishes the sub-agent row map: every painted row
/// carries its click key (empty on the header, primary and background rows,
/// the task id on a sub-agent row), and the keyboard cursor's ordinal list is
/// exactly the sub-agent slice of it (the old hit-rect vectors are gone -
/// the cursor reads these vectors instead).
#[test]
fn test_render_publishes_row_maps_and_the_subagent_cursor_list() {
    let mut app = app_with_primary();
    app.active_tasks.push(task(
        "run-0001",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        true,
    ));
    app.active_tasks.push(task(
        "done-0002",
        "s1",
        "s3",
        "explore",
        TaskStatus::Completed,
        true,
    ));
    app.bg_tasks.push(bg_task("bg-0001", "sleep 30", "running"));
    app.bg_tasks.push(bg_task("bg-0002", "true", "completed"));

    render_effects(&mut app, 160, 44);
    let _area = app.active_agents_area;

    // The row map is index-aligned with the painted rows: header, primary,
    // two sub-agent rows, background heading and two background rows.
    assert_eq!(
        app.agents_row_task_ids.len(),
        7,
        "one row key per painted row"
    );
    assert_eq!(app.agents_row_task_ids[0], "", "the header opens nothing");
    assert_eq!(
        app.agents_row_task_ids[1], "primary",
        "the primary row keeps its sentinel key"
    );
    assert_eq!(app.agents_row_task_ids[2], "run-0001");
    assert_eq!(app.agents_row_task_ids[3], "done-0002");
    assert_eq!(
        app.agents_row_task_ids[4], "",
        "the background heading opens nothing"
    );
    assert_eq!(
        app.agents_row_task_ids[5], "",
        "a background row is not cursorable"
    );
    assert_eq!(app.agents_row_task_ids[6], "");

    // The cursor ordinal list covers sub-agent rows only, in paint order.
    assert_eq!(
        app.agents_subagent_task_ids,
        vec!["run-0001".to_string(), "done-0002".to_string()],
        "the cursor range must name the sub-agent rows in paint order"
    );

    // A rendered (but cursor-less) frame paints the controls on the live rows.

    // A completed task paints no controls on its row.
}

// -- FR-016: background shell section -------------------------------------

// -- NFR-003: single glyph source ----------------------------------------

// -- NFR-004: character-safe truncation ----------------------------------

// -- FR-020: the row paints the real execution contract --------------------

// -- FR-012: waiter visibility in the Agents panel (T-014) --------------------

// -- FR-026: empty state -----------------------------------------------------
