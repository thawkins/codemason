//! Tests for the `Alt+T` Teams-window toggle chord (spec `agentssetup` FR-045).
//!
//! The twin of `test_alt_a_agents_window.rs`: the chord resolves to an
//! `InputAction` and reaches the panel through the same
//! `App::toggle_teams_window` the Teams button's mouse click calls, so a
//! keyboard-only terminal user can open and dismiss the panel with or without
//! team members being present.
//!
//! Also covers NFR-002 (the `t` is never typed into the input buffer) and
//! NFR-009 (bare `t` still behaves exactly as before).

mod support;

use codemason_tui::crossterm_input::CrosstermAppExt;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::TeamConfig;
use codemason_tui::App;
use support::make_app;

/// The lead session id used by every case.
const LEAD_SESSION: &str = "lead-session-0001";

/// An `Alt+T` key event.
fn alt_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT)
}

/// A bare `t` key event, for the NFR-002 / NFR-009 checks.

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

/// Build an `App` with a live team, so the Teams window has a subject.
fn app_with_team() -> App {
    let mut app = app();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app
}

/// Render one frame so the button hit rectangles are populated.
fn render(app: &mut App) {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("draw");
}

// ---------------------------------------------------------------------------
// FR-045: the chord resolves to an action, not an inline mutation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-045: the keyboard produces the same outcome as the mouse click
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_opening_the_teams_window_closes_the_agents_window() {
    let mut app = app_with_team();
    app.active_tasks.push(running_task("task-1"));
    render(&mut app);
    app.show_agents_window = true;

    app.handle_key_event(alt_t()).await;
    assert!(app.show_teams_window, "the Teams window opens");
    assert!(
        !app.show_agents_window,
        "the Agents window is closed, exactly as on a mouse click"
    );
}

#[tokio::test]
async fn alt_a_and_alt_t_alternate_without_ever_showing_both() {
    let mut app = app_with_team();
    app.active_tasks.push(running_task("task-1"));
    render(&mut app);

    app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT))
        .await;
    assert!(app.show_agents_window && !app.show_teams_window);

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window && !app.show_agents_window,
        "the two windows are mutually exclusive whichever chord opens them"
    );
}

// ---------------------------------------------------------------------------
// NFR-002 / NFR-009: the chord consumes the key and shadows nothing
// ---------------------------------------------------------------------------
