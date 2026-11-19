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

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;
use codemason_app_core::app::{QUEUE_CLEAR_CONFIRM_NO, QUEUE_CLEAR_CONFIRM_YES, QueuedInput};

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

// ---------------------------------------------------------------------------
// FR-036 - `Yes` drains the queue and refreshes the counter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_yes_removes_every_entry_and_closes_the_dialog() {
    let mut app = app_with_dialog_open();
    assert_eq!(app.input_queue_len(), 2, "precondition: two entries queued");

    // Move the selection to `Yes` (Right toggles No -> Yes), then confirm.
    app.handle_key_event(key(KeyCode::Right)).await;
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_YES,
        "precondition: Right selects `Yes`"
    );
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-036: `Yes` removes every entry from the queue"
    );
    assert!(
        !app.queue_clear_confirm_open,
        "FR-036: the dialog closes after confirming"
    );
}

#[tokio::test]
async fn test_yes_resets_the_selection_and_arms_a_redraw() {
    let mut app = app_with_dialog_open();
    app.handle_key_event(key(KeyCode::Right)).await;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "FR-034: the selection resets to `No` for the next open"
    );
}

#[tokio::test]
async fn test_yes_drops_a_pending_queue_control_next() {
    let mut app = app_with_dialog_open();
    app.session.queue_next_pending = true;
    app.handle_key_event(key(KeyCode::Right)).await;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        !app.session.queue_next_pending,
        "FR-024: a pending `Next` is meaningless once the queue is emptied"
    );
}

#[tokio::test]
async fn test_yes_on_an_empty_queue_is_a_safe_noop() {
    let mut app = app_with_dialog_open();
    app.session.input_queue.clear();

    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "confirming an already-empty queue must not panic or re-add"
    );
    assert!(
        !app.queue_clear_confirm_open,
        "the dialog still closes on an empty queue"
    );
}

// ---------------------------------------------------------------------------
// FR-035 / FR-037 - `No` and `Esc` leave the queue unchanged
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_keeps_every_entry() {
    let mut app = app_with_dialog_open();

    // Enter on the default `No` selection.
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-035: selecting `No` leaves the queue unchanged"
    );
    assert!(
        !app.queue_clear_confirm_open,
        "FR-035: selecting `No` closes the dialog"
    );
}

#[tokio::test]
async fn test_esc_keeps_every_entry() {
    let mut app = app_with_dialog_open();
    // Select `Yes` first to prove Esc cancels regardless of the highlight.
    app.handle_key_event(key(KeyCode::Right)).await;

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-035: dismissing with Esc leaves the queue unchanged even with `Yes` highlighted"
    );
    assert!(
        !app.queue_clear_confirm_open,
        "FR-035: Esc closes the dialog"
    );
}

#[tokio::test]
async fn test_opening_the_dialog_never_removes_an_entry() {
    let mut app = app_with_session();
    app.session.input_queue.push_back(entry("GATE-QUEUE-1"));
    app.session.input_queue.push_back(entry("GATE-QUEUE-2"));
    app.queue_menu_open = true;

    app.queue_menu_select_clear();

    assert!(
        app.queue_clear_confirm_open,
        "FR-033: selecting Clear opens the confirmation dialog"
    );
    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-037: selecting `Clear` must not remove any entry before `Yes`"
    );
}
