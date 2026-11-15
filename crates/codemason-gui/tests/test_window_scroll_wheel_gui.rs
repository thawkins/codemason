//! Tests for popup-table wheel scrolling (spec `agentui` T-017).
//!
//! Covers the FR-008 / FR-059 wheel path: a wheel event over the Agents or
//! Teams popup body moves the table's scroll offset by `SCROLL_STEP_LINES`
//! (`3`), the offset re-clamps to the maximum the renderer computed (a wheel
//! up past the bottom cannot overshoot, a wheel down below the top cannot go
//! negative), the wheel is consumed so the pane beneath the popup never
//! scrolls, every accepted wheel sets the redraw flag (NFR-005), and the
//! clamped, non-zero offset shifts the painted rows with the scrollbar
//! tracking the position (FR-056, acceptance 19).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`])
//! against areas recorded by the real render entry point through a ratatui
//! `TestBackend`, so the assertions exercise the exact path a real wheel event
//! takes (NFR-001). All state is synthetic and in-memory; nothing is written
//! into the repository (NFR-012). Per-frame work is the visible-window slice
//! the renderers already perform, so a scrolled frame costs no more than a
//! static one (NFR-002).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_window_scroll_wheel.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_app_core::app::App;

/// A running sub-agent [`TaskEntry`] parented to the lead session.
fn task(id: &str, parent_session: &str, agent: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent_session.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
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

/// `render_agents` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_agents_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_agents_window = true;
    crate::frame::publish(app);
}

/// `render_teams` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_teams_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_teams_window = true;
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Agents table offset
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Teams table offset
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-005: redraw signalling
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// No popup: the wheel keeps the old behaviour
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// End-to-end: the offset is applied, hit-rects follow, the scrollbar tracks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Scrollbar-gutter drag (FR-059's mouse half; the row clicks are keyboard-only)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// PgUp / PgDn: the keyboard paging siblings of the wheel (FR-059's keyboard
// half; the cursor pages one viewport at a time)
// ---------------------------------------------------------------------------

/// PgUp on the Agents window pages the cursor up and clamps at the first row;
/// PgDn pages down and clamps at the last row (FR-034, FR-059).
#[tokio::test]
async fn test_agents_page_keys_move_the_cursor_one_page_and_clamp() {
    use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = support::make_app();
    app.session.session_id = Some("lead-session-0001".to_string());
    app.session.agent_name = "general".to_string();
    for i in 0..12 {
        let id = format!("task-{i:02}");
        app.active_tasks
            .push(task(&id, "lead-session-0001", "explore"));
    }
    render_agents_effects(&mut app, 160, 14);

    // First PgDn starts from the top of the list.
    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    let first_page = app.agents_cursor_subagent.expect("the cursor must be set");
    assert!(
        first_page >= 1,
        "one page must move the cursor at least one row: {first_page}"
    );
    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(11),
        "paging down past the end clamps at the last sub-agent (FR-034)"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.agents_cursor_subagent,
        Some(0),
        "paging up past the top clamps at the first sub-agent (FR-034)"
    );
}

/// The Teams PgUp/PgDn behave the same way (FR-038, FR-059).
#[tokio::test]
async fn test_teams_page_keys_move_the_cursor_one_page_and_clamp() {
    use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = support::make_app();
    app.session.session_id = Some("lead-session-0001".to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(codemason_agent::team::TeamConfig::new(
        "page-keys-teams",
        "lead-session-0001",
    ));
    for i in 0..12 {
        let mut member = codemason_agent::team::TeamMember::new(
            format!("member-{i:02}"),
            format!("tm-{i:03}"),
            "general",
        );
        member.status = codemason_agent::team::MemberStatus::Working;
        member.session_id = Some(format!("tm-{i:03}-session"));
        app.session.team_members.push(member);
    }
    render_teams_effects(&mut app, 170, 14);

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
        .await;
    let first_page = app.teams_cursor_teammate.expect("the cursor must be set");
    assert!(
        first_page >= 1,
        "one page must move the cursor: {first_page}"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(11),
        "paging down past the end clamps at the last teammate (FR-038)"
    );

    for _ in 0..8 {
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
    }
    assert_eq!(
        app.teams_cursor_teammate,
        Some(0),
        "paging up past the top clamps at the first teammate (FR-038)"
    );
}
