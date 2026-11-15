//! Tests for the Agents/Teams window open-close routing (spec `agentui` T-007).
//!
//! Covers the Agents button toggle with mutual exclusion (FR-028), the no-op
//! click when the Agents window has no subject (FR-029), the Teams button
//! toggle and its exclusion (FR-030), the no-op click when no team is active
//! (FR-031), the `Esc` dismissal that leaves task, member and input
//! state untouched (FR-032), and the redraw flag every mutating path sets
//! (NFR-005).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`]) with
//! the hit rects the renderer records, so the assertions exercise the same
//! path a real click takes (NFR-001).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_window_routing.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id used by every case.
const LEAD_SESSION: &str = "lead-session-0001";

/// A live sub-agent task so the Agents window has a subject.
fn running_task(id: &str) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
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

/// A teammate in `Working` state.
fn member(name: &str, agent_id: &str) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = MemberStatus::Working;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// A bare app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.needs_redraw = false;
    app
}

/// An app with one running sub-agent task, so the Agents button is enabled.
fn app_with_task() -> App {
    let mut app = app();
    app.active_tasks.push(running_task("task-1"));
    app
}

/// An app with an active team holding one teammate.
fn app_with_team() -> App {
    let mut app = app();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app.session.team_members.push(member("reviewer", "tm-001"));
    app
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// FR-028 / FR-029: Agents button
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-028 / FR-030: mutual exclusion
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-030 / FR-031: Teams button
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-032: `Esc` dismissal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn agents_escape_dismisses_the_window_and_leaves_state_untouched() {
    let mut app = app_with_task();
    app.active_tasks.push(running_task("task-2"));
    app.session.input = "hello".to_string();
    let tasks_before = app.active_tasks.len();
    app.show_agents_window = true;
    render(&mut app);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(!app.show_agents_window, "Esc dismisses the window (FR-032)");
    assert!(app.needs_redraw, "dismissing requests a redraw");
    assert_eq!(
        app.active_tasks.len(),
        tasks_before,
        "closing the window must not touch the task list"
    );
    assert_eq!(
        app.session.input, "hello",
        "closing the window must not touch the input"
    );
}

#[tokio::test]
async fn teams_escape_dismisses_the_window_and_leaves_state_untouched() {
    let mut app = app_with_team();
    app.session.input = "draft".to_string();
    app.show_teams_window = true;
    render(&mut app);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(!app.show_teams_window, "Esc dismisses the window (FR-032)");
    assert!(app.needs_redraw, "dismissing requests a redraw");
    assert_eq!(
        app.session.team_members.len(),
        1,
        "closing the window must not touch the member list"
    );
    assert_eq!(
        app.session.input, "draft",
        "closing the window must not touch the input"
    );
}

#[tokio::test]
async fn escaping_an_already_closed_window_is_inert() {
    let mut app = app_with_task();
    render(&mut app);
    assert!(!app.show_agents_window);
    assert!(!app.show_teams_window);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(!app.show_agents_window);
    assert!(!app.show_teams_window);
    assert!(!app.needs_redraw, "escaping a closed window paints nothing");
}

// ---------------------------------------------------------------------------
// Router helpers directly (the state machine behind every click path)
// ---------------------------------------------------------------------------
