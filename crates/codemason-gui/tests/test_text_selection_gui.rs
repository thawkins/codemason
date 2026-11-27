//! Tests for `test_text_selection.rs`
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_text_selection.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use codemason_agent::team::{TeamConfig, TeamMember};
use codemason_app_core::app::App;

/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

// ---------- Selection creation ----------

// ---------- Selection dragging ----------

/// Render one frame so the renderer records the window hit areas and the
/// painted-row click map (spec `agentui` A3: rows resolve through the
/// render-recorded map, not hand-set coordinates).
fn render(app: &mut App) {
    crate::frame::publish(app);
}

#[test]
fn test_opening_active_agents_row_output_view_from_the_keyboard() {
    // The row-mouse model is retired: the sub-agent overlay opens through the
    // cursor (Down + Enter), the same destination chain the old row click
    // drove (FR-034).
    let mut app = make_app();
    app.session.session_id = Some("lead-s1".to_string());
    app.show_agents_window = true;
    app.active_tasks.push(codemason_agent::task::TaskEntry {
        id: "task-12345678".to_string(),
        parent_session_id: "lead-s1".to_string(),
        child_session_id: "child-s1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: codemason_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: codemason_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    });
    render(&mut app);
    app.move_agents_cursor(1);
    app.agents_cursor_open_output();
    assert!(app.output_view.is_some());
    assert_eq!(
        app.session.selected_agent_session_id.as_deref(),
        Some("child-s1")
    );
}

#[test]
fn test_opening_teams_row_output_view_from_the_keyboard() {
    // The teammate overlay opens through the cursor (Down + Enter), the same
    // destination chain the old row click drove (FR-038).
    let mut app = make_app();
    app.session.session_id = Some("lead-s1".to_string());
    app.show_teams_window = true;
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-s1"));
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("tm-s1".to_string());
    app.session.team_members.push(member);

    render(&mut app);
    app.move_teams_cursor(1);
    app.teams_cursor_focus();
    assert_eq!(
        app.session
            .focused_teammate
            .as_ref()
            .map(|(_, id)| id.as_str()),
        Some("tm-001")
    );
    let view = app.output_view.as_ref().expect("the overlay must open");
    match &view.target {
        codemason_app_core::app::OutputViewTarget::TeamMember { session_id, .. } => {
            assert_eq!(session_id.as_deref(), Some("tm-s1"));
        }
        other => panic!("expected a TeamMember target, got {other:?}"),
    }
}

// ---------- Selection normalization ----------

// ---------- Right-click context menu ----------

// ---------- extract_text_from_lines ----------

// ---------- Scrollbar click does NOT start selection ----------

// ---------- New click replaces old selection ----------

// ---------- Regression: Alt+I dismisses log panel, stale selection panics ----------
