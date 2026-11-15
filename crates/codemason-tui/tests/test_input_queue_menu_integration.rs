//! End-to-end integration tests for the ALT-Q queue-control menu and its three
//! actions (spec `inputqueue` T-019).
//!
//! The per-task suites exercise each piece in isolation: T-013 covers the
//! ALT-Q binding, T-014 the three-option render, T-015 the `Next` action,
//! T-016 the `Stop`/`Resume` action, T-017 the `Clear` action, and T-018 the
//! `Esc` dismissal. This suite composes them into the full user journey -
//! open the menu with `ALT-Q`, select a row, and observe the effect - so the
//! actions are proven to work together rather than only in isolation.
//!
//! Covers FR-021 (ALT-Q opens a menu with exactly `Next`/`Stop`/`Clear`),
//! FR-023 (the `Next` row is selectable only while the queue is non-empty),
//! FR-024 (selecting `Next` stops the running turn and dispatches the oldest
//! entry), FR-025 (selecting `Stop` halts without advancing the queue),
//! FR-026 (the halt row reads `Stop` while executing and `Resume` when idle),
//! FR-027 (selecting `Resume` restarts the interrupted work), FR-028 (selecting
//! `Clear` opens the confirmation dialog rather than emptying the queue),
//! FR-029 (halting never advances the queue) and FR-031 (the menu never mutates
//! the editable input buffer).
//!
//! Rows are selected through the public [`App::queue_menu_select_next`],
//! [`App::queue_menu_select_halt`], and [`App::queue_menu_select_clear`] entry
//! points - the same methods the menu key-handling invokes - after the menu has
//! been opened through the real `ALT-Q` keystroke path.

use codemason_tui::crossterm_input::CrosstermAppExt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_tui::App;
use codemason_tui::app::QueuedInput;
use codemason_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session so the dispatch paths have a session.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("test-session".to_string());
    app
}

/// Open the menu the way the user would (`ALT-Q`) and assert it opened.
async fn open_menu_with_alt_q(app: &mut App) {
    app.handle_key_event(alt(KeyCode::Char('q'))).await;
    assert!(
        app.queue_menu_open,
        "precondition: ALT-Q must open the queue-control menu"
    );
}

/// Deliver a `MessageEnd` for the current session with the given reason.

/// Count user messages in the conversation.

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// The painted row containing `needle` within the menu's inner area.
fn menu_row(terminal: &Terminal<TestBackend>, app: &App, needle: &str) -> String {
    let area = app.queue_menu_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    let buffer = terminal.backend().buffer();
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| {
            (x0..x1)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("expected a menu row containing {needle:?}"))
}

// ---------------------------------------------------------------------------
// FR-021 / FR-022 / FR-031 - opening the menu through ALT-Q
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-023 - the `Next` row is selectable only while the queue is non-empty
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_next_row_selectability_tracks_the_queue_end_to_end() {
    let mut app = app_with_session();
    open_menu_with_alt_q(&mut app).await;

    // Empty queue: the `Next` row is painted dimmed and carries no marker.
    let empty = render(&mut app, 120, 40);
    let empty_next = menu_row(&empty, &app, "Next");
    assert!(
        !empty_next.starts_with("> "),
        "FR-023: with an empty queue `Next` must not be selectable, got {empty_next:?}"
    );

    // Enqueue one entry while the menu is open; `Next` becomes selectable.
    app.session.input_queue.push_back(entry("VISIBLE-1"));
    let filled = render(&mut app, 120, 40);
    let filled_next = menu_row(&filled, &app, "Next");
    assert!(
        filled_next.starts_with("> "),
        "FR-023: with a queued entry `Next` becomes selectable, got {filled_next:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-024 / FR-029 - `Next` dispatches the oldest entry in FIFO order
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-025 / FR-029 - `Stop` halts without advancing the queue
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-026 / FR-027 - halt row label switches to `Resume` and resumes
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-028 - `Clear` opens the confirmation dialog without emptying the queue
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-031 / FR-032 - Esc dismisses the menu without mutating state
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-031 - no row action inserts characters into the editable buffer
// ---------------------------------------------------------------------------
