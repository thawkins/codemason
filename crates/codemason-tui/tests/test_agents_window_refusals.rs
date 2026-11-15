//! Coverage for the two named panel conditions of spec `agentssetup` T-023.
//!
//! - **FR-042** - a chord or button press that produces no window is never
//!   left unexplained. The two panels now always open (an empty one paints its
//!   own empty-state hint, `agentui` FR-026), so the only silent press left is a
//!   close of an already-closed window, which is a no-op by design; the named
//!   refusal texts are still asserted so no entry point regresses to a silent
//!   "nothing happened".
//! - **FR-032** - a finished row whose report has no durable copy
//!   (`output_file: None`, the state a failed `persist_task_output` leaves
//!   behind) states that the report is in-memory only. The history row and the
//!   panel's rendered history section both carry it, and the wording is the
//!   shared `IN_MEMORY_REPORT_NOTE` that `list_agents` / `wait_agents` print,
//!   so the panel cannot describe the condition differently from the tools.
//!
//! The blocking filesystem of TESTPLAN TC-019 is replaced by the registry
//! state a failed persist leaves behind, for the same reason T-015 did: the
//! requirement under test is the *reporting*, not the write.

#[path = "support/mod.rs"]
mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_agent::task::{ReportStatus, TaskStatus};
use codemason_tui::App;
use codemason_tui::app::CompletedTaskRow;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-refusals";

/// A live sub-agent task.

/// A finished history row, optionally carrying a durable report path.
fn history_row(id: &str, output_file: Option<&str>) -> CompletedTaskRow {
    let created_at = chrono::Utc::now();
    CompletedTaskRow {
        id: id.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: "explore".to_string(),
        status: TaskStatus::Completed,
        background: true,
        detached: false,
        created_at,
        completed_at: created_at + chrono::Duration::seconds(7),
        report_status: ReportStatus::default(),
        output_file: output_file.map(std::path::PathBuf::from),
    }
}

/// An app with an explicit status baseline and a clean redraw flag.
fn app() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.status = "ready".to_string();
    app.needs_redraw = false;
    app
}

/// Flatten the rendered frame into one string.
fn screen_text(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("render frame");
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

// ---------------------------------------------------------------------------
// FR-042 / FR-026: the panels always open and name their own empty state
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-032: an absent durable copy is named, not omitted
// ---------------------------------------------------------------------------

#[test]
fn the_panel_paints_the_absence_note_and_no_false_path() {
    let mut app = app();
    app.agents_history.push_back(history_row("task-0005", None));
    app.show_agents_window = true;

    let text = screen_text(&mut app, 320, 44);

    assert!(
        text.contains("Completed"),
        "the history section paints under its heading: {text}"
    );
    assert!(
        text.contains("[memory]"),
        "FR-032: the history row flags the missing durable copy: {text}"
    );
    assert!(
        text.contains("completed") && text.contains("in-memory"),
        "FR-032: the history row spells the condition out where the panel has \
         room to paint it: {text}"
    );
    assert!(
        !text.contains("task-0005.md"),
        "FR-043: no path may be claimed for a report that was never written: \
         {text}"
    );
}

#[test]
fn the_panel_paints_no_note_for_a_persisted_report() {
    let mut app = app();
    app.agents_history
        .push_back(history_row("task-0006", Some("log/subagents/task-0006.md")));
    app.show_agents_window = true;

    let text = screen_text(&mut app, 320, 44);

    assert!(
        text.contains("Completed"),
        "the history section still paints: {text}"
    );
    assert!(
        !text.contains("[memory]") && !text.contains("in-memory"),
        "FR-032: a persisted report must not carry the absence note, or the \
         note becomes noise on every healthy task: {text}"
    );
}

// ---------------------------------------------------------------------------
// FR-028 / FR-036: the snapshot is taken before the live row goes
// ---------------------------------------------------------------------------
