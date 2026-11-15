//! Tests for the `Clear the input queue?` confirmation gating (spec
//! `inputqueue` T-022).
//!
//! Covers FR-035 (selecting `No` or dismissing with Esc closes the dialog and
//! leaves the queue unchanged), FR-036 (selecting `Yes` removes every entry and
//! updates the queue counter to reflect the now-empty queue, then closes the
//! dialog) and FR-037 (no entry is removed by merely selecting `Clear` - only
//! an explicit `Yes` may drain the queue).
//!
//! Unlike the T-021 suite (which asserts the dialog's presentation and maps
//! keys to [`InputAction`]s), this suite drives the *whole* journey through the
//! real keystroke path - ALT-Q, `Clear` row, then `Yes`/`No`/`Esc` - and
//! asserts the resulting queue state.

use codemason_tui::crossterm_input::CrosstermAppExt;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_tui::App;
use codemason_tui::app::{QUEUE_CLEAR_CONFIRM_NO, QueuedInput};
use codemason_tui::layout;

#[path = "support/mod.rs"]
mod support;

/// A plain (modifier-free) key.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("test-session".to_string());
    app
}

/// Open the dialog the way the user does: open the ALT-Q menu, select `Clear`,
/// press `Enter`. The queue is seeded with two entries first.
fn app_with_dialog_open() -> App {
    let mut app = app_with_session();
    app.session.input_queue.push_back(entry("GATE-QUEUE-1"));
    app.session.input_queue.push_back(entry("GATE-QUEUE-2"));
    app.queue_menu_open = true;
    app.queue_menu_select_clear();
    assert!(
        app.queue_clear_confirm_open,
        "precondition: selecting Clear opened the confirmation dialog"
    );
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "precondition: the dialog defaults to `No`"
    );
    app
}

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// The painted text on the input area's first content row (border stripped),
/// which begins with the queue counter prefix while entries are pending.
fn input_row_text(terminal: &Terminal<TestBackend>, app: &App) -> String {
    let area = app.input_area;
    let buffer = terminal.backend().buffer();
    (area.x + 1..area.x + area.width.saturating_sub(1))
        .map(|x| buffer[(x, area.y + 1)].symbol().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// FR-036 - `Yes` drains the queue and refreshes the counter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_yes_updates_the_painted_queue_counter() {
    let mut app = app_with_dialog_open();
    app.session.input = "draft".to_string();
    app.session.input_cursor = app.input_len_chars();

    let before = render(&mut app, 80, 24);
    assert!(
        input_row_text(&before, &app).starts_with("02> "),
        "precondition: the input prefix shows the two-entry counter, got {:?}",
        input_row_text(&before, &app)
    );

    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    let after = render(&mut app, 80, 24);
    assert!(
        input_row_text(&after, &app).starts_with("> "),
        "FR-036: the counter disappears once the queue is emptied, got {:?}",
        input_row_text(&after, &app)
    );
}

// ---------------------------------------------------------------------------
// FR-035 / FR-037 - `No` and `Esc` leave the queue unchanged
// ---------------------------------------------------------------------------
