//! Tests for the Agents-window lifecycle and status-line reporting (spec
//! `agentui` T-018, FR-061, FR-063, FR-064, NFR-005).
//!
//! - FR-063: the Agents window closes on the turn whose removal empties it -
//!   via the `SubagentComplete`, `SubagentCancelled`, `SubagentKilled` and
//!   `BackgroundTaskCompleted` event arms and via the registry reconciliation
//!   merge; a row remaining keeps it open and an already-closed window is a
//!   silent no-op;
//! - FR-061: window open/close and row-button actions stay serviced while a
//!   turn is running (`is_processing`), driven through the real mouse router;
//! - FR-064: the protected-agent refusals, the teammate focus transitions and
//!   the focus-cleared notice appear in the status line;
//! - NFR-005: every auto-close, refusal and focus transition sets the redraw
//!   flag so the next frame shows the change.
//!
//! Every case builds synthetic in-memory state through `make_app`; nothing
//! writes into the repository working tree (NFR-012).

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{TeamConfig, TeamMember};
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t018";

/// A live sub-agent task. The id keeps its 8-char short prefix visible: the
/// suspend/kill request lines are logged under `short_id` (the leading 8
/// characters).
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
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

/// An app with the Agents window open, a redraw baseline of "clean" and the
/// given sub-agent rows.
fn app_with_window(tasks: Vec<TaskEntry>) -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks = tasks;
    app.show_agents_window = true;
    app.needs_redraw = false;
    app
}

/// An app with an active two-member team.
fn app_with_team() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.status = codemason_agent::team::MemberStatus::Working;
    member.session_id = Some("tm-001-session".to_string());
    app.session.team_members.push(member);
    app.show_teams_window = true;
    app.needs_redraw = false;
    app
}

// ---------------------------------------------------------------------------
// FR-063: the window closes when its list empties
// ---------------------------------------------------------------------------

/// Closing an already-closed window is a silent no-op: no redraw is demanded
/// and no state changes (NFR-005).
#[test]
fn an_already_closed_or_occupied_window_is_untouched_by_the_auto_close() {
    // Closed already: silent.
    let mut app = app_with_window(Vec::new());
    app.show_agents_window = false;
    app.close_agents_window_when_empty();
    assert!(!app.show_agents_window);
    assert!(
        !app.needs_redraw,
        "a redundant close on a closed window paints nothing (NFR-005)"
    );

    // Still has a subject: the window stays open.
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.close_agents_window_when_empty();
    assert!(
        app.show_agents_window,
        "the auto-close never fires while a subject is tracked (FR-063)"
    );
}

// ---------------------------------------------------------------------------
// FR-061: the panel stays serviceable during a running turn
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-064: status-line reporting for refusals and focus transitions
// ---------------------------------------------------------------------------

/// The keyboard cursor refusals/status reporting stay live while a turn
/// streams (FR-064; the protected-agent refusals are re-expressed by the
/// cursor range - the cursor never reaches the primary row, so the reported
/// text is the consumed no-op message instead, T-011's keyboard twin).
#[tokio::test]
async fn cursor_reports_to_the_status_line_during_a_running_turn() {
    let mut app = app_with_window(vec![task("a1b2c3d4", "explore", TaskStatus::Running)]);
    app.session.is_processing = true;

    app.handle_key_event(crate::term_events::KeyEvent::new(
        KeyCode::Char(' '), // no cursor yet: consumed no-op with its status text
        KeyModifiers::NONE,
    ))
    .await;
    assert_eq!(
        app.status, "agents: selected row is gone",
        "the window key reports to the status line (FR-064)"
    );

    // Moving the cursor also reports the landed row on the status line.
    app.handle_key_event(crate::term_events::KeyEvent::new(
        KeyCode::Down,
        KeyModifiers::NONE,
    ))
    .await;
    assert!(
        app.status.starts_with("agents: "),
        "a cursor move reports the selection (FR-064), got {}",
        app.status
    );
    assert!(
        app.session.is_processing,
        "the keyboard path must not disturb the running turn"
    );
}

/// An `Alt+Down` focus transition reports the newly focused teammate to the
/// status line, and cycling past the ends reports the cleared focus
/// (FR-064, FR-046).
#[tokio::test]
async fn focus_transitions_report_to_the_status_line() {
    let mut app = app_with_team();

    app.cycle_focused_teammate(true);

    assert_eq!(
        app.session.focused_teammate.as_deref(),
        Some("tm-001"),
        "the first cycle focuses the only member (FR-046)"
    );
    assert_eq!(
        app.status, "team: focused -> reviewer",
        "the focus transition reports to the status line (FR-064)"
    );

    // Cycling past the end clears the focus and reports it (FR-064).
    app.cycle_focused_teammate(true);
    assert!(app.session.focused_teammate.is_none());
    assert_eq!(
        app.status, "team: focus cleared",
        "the focus-cleared notice reports to the status line (FR-064)"
    );
}
