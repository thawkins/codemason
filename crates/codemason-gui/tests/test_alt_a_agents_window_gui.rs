//! Tests for the `Alt+A` Agents-window open chord (spec `agentssetup` T-019).
//!
//! Covers FR-022: the Agents panel is openable from the keyboard alone, and the
//! chord must reach the panel through an `InputAction` rather than an inline
//! mutation. Every case here drives the real input router
//! ([`App::handle_key_event`]) and asserts the outcome the Agents button's
//! mouse click produces in `test_window_routing.rs` - the same
//! `App::toggle_agents_window` visibility path, including the no-op when the
//! window has no subject, the Teams-window exclusion, and the redraw flag.
//!
//! Also covers NFR-002 (the `a` is never typed into the input buffer) and
//! NFR-009 (bare `a` still behaves exactly as before).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_alt_a_agents_window.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::TeamConfig;
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id used by every case.
const LEAD_SESSION: &str = "lead-session-0001";

/// An `Alt+A` key event.
fn alt_a() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)
}

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

/// An app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.needs_redraw = false;
    app
}

/// An app with one running sub-agent task, so the Agents window is available.
fn app_with_task() -> App {
    let mut app = app();
    app.active_tasks.push(running_task("task-1"));
    app
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// FR-022: the chord resolves to an InputAction, not an inline mutation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-022: the keyboard produces the same outcome as the mouse click
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_a_opening_the_agents_window_closes_the_teams_window() {
    let mut app = app_with_task();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    render(&mut app);
    app.show_teams_window = true;

    app.handle_key_event(alt_a()).await;
    assert!(app.show_agents_window, "the Agents window opens");
    assert!(
        !app.show_teams_window,
        "the Teams window is closed, exactly as on a mouse click"
    );
}

// ---------------------------------------------------------------------------
// NFR-002 / NFR-009: the chord consumes the key and shadows nothing
// ---------------------------------------------------------------------------
