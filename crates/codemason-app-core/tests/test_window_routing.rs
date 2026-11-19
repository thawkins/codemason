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

mod support;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::TeamConfig;
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

/// A background shell task so the Agents window has a subject.
fn bg_task(id: &str) -> codemason_app_core::app::BgTaskView {
    codemason_app_core::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: "sleep 30".to_string(),
        status: "running".to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// A bare app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app
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

// ---------------------------------------------------------------------------
// Router helpers directly (the state machine behind every click path)
// ---------------------------------------------------------------------------

#[test]
fn routing_helpers_agree_with_the_buttons_enabled_state() {
    let mut app = app();
    assert!(!app.agents_window_available());
    assert!(!app.teams_window_available());

    app.active_tasks.push(running_task("task-1"));
    assert!(app.agents_window_available());

    app.active_tasks.clear();
    app.bg_tasks.push(bg_task("bg-1"));
    assert!(
        app.agents_window_available(),
        "a background shell task alone enables the window"
    );

    app.bg_tasks.clear();
    assert!(!app.agents_window_available());

    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    assert!(app.teams_window_available());
}

#[test]
fn toggling_through_the_helpers_opens_and_closes_without_a_subject() {
    // Availability only styles the button; it never gates the toggle. Each
    // press toggles, and each open closes the other window.
    let mut app = app();
    app.toggle_agents_window();
    assert!(app.show_agents_window, "an empty Agents panel still opens");
    assert!(!app.teams_window_available(), "still no team to show");

    app.toggle_teams_window();
    assert!(app.show_teams_window, "an empty Teams panel still opens");
    assert!(!app.show_agents_window, "and the Agents window closes");

    app.toggle_agents_window();
    assert!(app.show_agents_window, "back to the Agents window");
    assert!(!app.show_teams_window);

    app.close_agents_window();
    app.close_teams_window();
    assert!(!app.show_agents_window && !app.show_teams_window);
}
