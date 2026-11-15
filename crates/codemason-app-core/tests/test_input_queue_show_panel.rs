//! Tests for the scrollable queue-entry panel opened by the queue-control
//! menu's `Show` row (spec `inputqueue` `Show` row).
//!
//! Covers the panel listing every queued entry oldest-first, `Up`/`Down`
//! scrolling the block-cursor highlight, `Enter` moving the highlighted entry
//! one step toward the front of the queue, `Del` removing it, and `Esc` being
//! the only key that dismisses the panel. The panel must never mutate the
//! editable input buffer, the staged attachments, or the running turn.

use crate::term_events::CrosstermAppExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;
use codemason_app_core::app::QueuedInput;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

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

/// An app with the given entries queued and the `Show` panel open.
fn panel_with(entries: &[&str]) -> App {
    let mut app = support::make_app();
    for text in entries {
        app.session.input_queue.push_back(entry(text));
    }
    app.queue_show_open_panel();
    assert!(app.queue_show_open, "precondition: the panel opened");
    app
}

/// The queue texts, oldest first, as a plain vector.
fn queue_texts(app: &App) -> Vec<String> {
    app.session
        .input_queue
        .iter()
        .map(|e| e.text.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Opening the panel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_show_row_opens_the_panel_and_closes_the_menu() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("a"));
    app.queue_menu_open = true;
    app.queue_menu_selected = codemason_app_core::app::QUEUE_MENU_ROW_SHOW;

    app.queue_menu_activate_selected().await;

    assert!(app.queue_show_open, "the `Show` row opens the panel");
    assert!(!app.queue_menu_open, "the menu closes when the panel opens");
}

#[test]
fn test_opening_the_panel_resets_the_highlight_to_the_oldest_entry() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("a"));
    app.session.input_queue.push_back(entry("b"));
    app.queue_show_selected = 5;

    app.queue_show_open_panel();

    assert_eq!(
        app.queue_show_selected, 0,
        "the panel opens on the oldest entry"
    );
}

#[test]
fn test_opening_the_panel_sets_the_redraw_flag() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("a"));
    app.needs_redraw = false;

    app.queue_show_open_panel();

    assert!(app.needs_redraw, "the panel must paint on the next frame");
}

// ---------------------------------------------------------------------------
// Rendering: entries oldest-first with a block cursor on the highlight
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Up/Down scroll the highlight
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_down_moves_the_highlight_to_the_next_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(app.queue_show_selected, 1, "Down highlights the next entry");
    assert!(app.needs_redraw, "the move must repaint on the next frame");
}

#[tokio::test]
async fn test_up_moves_the_highlight_to_the_previous_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_show_selected, 1,
        "Up highlights the previous entry"
    );
}

#[tokio::test]
async fn test_down_stops_at_the_last_entry() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(
        app.queue_show_selected, 1,
        "Down is a no-op at the last entry"
    );
}

#[tokio::test]
async fn test_up_stops_at_the_first_entry() {
    let mut app = panel_with(&["a", "b"]);
    assert_eq!(app.queue_show_selected, 0);

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_show_selected, 0,
        "Up is a no-op at the first entry"
    );
}

// ---------------------------------------------------------------------------
// Enter moves the highlighted entry one step toward the front
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_enter_moves_the_highlighted_entry_one_step_toward_the_front() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["a", "c", "b"],
        "Enter swaps the highlighted entry with the one before it"
    );
    assert_eq!(
        app.queue_show_selected, 1,
        "the highlight follows the moved entry so a second Enter advances it again"
    );
}

#[tokio::test]
async fn test_enter_at_the_front_is_a_noop() {
    let mut app = panel_with(&["a", "b", "c"]);
    assert_eq!(app.queue_show_selected, 0);

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b", "c"],
        "the oldest entry cannot move any further forward"
    );
}

#[tokio::test]
async fn test_two_enters_walk_an_entry_to_the_front() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Enter)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["c", "a", "b"],
        "repeated Enter walks the entry to the front one step at a time"
    );
    assert_eq!(app.queue_show_selected, 0);
}

#[tokio::test]
async fn test_enter_preserves_the_order_of_the_other_entries() {
    let mut app = panel_with(&["a", "b", "c", "d"]);
    app.queue_show_selected = 3;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b", "d", "c"],
        "only the highlighted entry moves; the rest keep their order"
    );
}

#[tokio::test]
async fn test_enter_sets_the_redraw_flag() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.needs_redraw, "the reorder must repaint the panel");
    assert!(app.queue_show_open, "Enter must not dismiss the panel");
}

// ---------------------------------------------------------------------------
// Del removes the highlighted entry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_del_removes_the_highlighted_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Delete)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["a", "c"],
        "Del removes only the highlighted entry"
    );
    assert_eq!(app.input_queue_len(), 2);
    assert!(app.needs_redraw, "the removal must repaint the panel");
    assert!(app.queue_show_open, "Del must not dismiss the panel");
}

#[tokio::test]
async fn test_del_keeps_the_highlight_in_range() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Delete)).await;

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b"],
        "the last entry is removed"
    );
    assert_eq!(
        app.queue_show_selected, 1,
        "the highlight clamps to the new last entry"
    );
}

#[tokio::test]
async fn test_del_closes_the_panel_when_the_queue_becomes_empty() {
    let mut app = panel_with(&["only"]);

    app.handle_key_event(key(KeyCode::Delete)).await;

    assert_eq!(app.input_queue_len(), 0, "the entry is removed");
    assert!(
        !app.queue_show_open,
        "an empty queue leaves nothing to show, so the panel closes"
    );
}

#[tokio::test]
async fn test_del_on_an_empty_queue_is_a_noop() {
    let mut app = panel_with(&[]);
    app.queue_show_selected = 3;

    app.handle_key_event(key(KeyCode::Delete)).await;

    assert_eq!(app.input_queue_len(), 0, "nothing is removed");
    assert!(app.queue_show_open, "the panel stays open");
}

// ---------------------------------------------------------------------------
// Esc is the only key that dismisses the panel
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_esc_dismisses_the_panel() {
    let mut app = panel_with(&["a", "b"]);

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(!app.queue_show_open, "Esc dismisses the panel");
    assert_eq!(
        queue_texts(&app),
        vec!["a", "b"],
        "dismissing leaves the queue unchanged"
    );
}

#[tokio::test]
async fn test_esc_resets_the_highlight_for_the_next_open() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert_eq!(
        app.queue_show_selected, 0,
        "dismissing resets the highlight"
    );
}

#[tokio::test]
async fn test_change_keys_keep_the_panel_open() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Enter)).await;
    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Up)).await;

    assert!(
        app.queue_show_open,
        "only Esc may dismiss the panel; Ent, Down, and Up keep it open"
    );
}

#[tokio::test]
async fn test_plain_character_is_swallowed_without_closing_the_panel() {
    let mut app = panel_with(&["a", "b"]);
    app.session.input = "PROBE".to_string();
    app.session.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Char('z'))).await;

    assert!(
        app.queue_show_open,
        "a printable character must not dismiss the panel"
    );
    assert_eq!(
        app.session.input, "PROBE",
        "the character must not reach the input buffer"
    );
}

// ---------------------------------------------------------------------------
// The panel never touches the draft, attachments, or the running turn
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_panel_never_mutates_the_draft_or_attachments() {
    let mut app = panel_with(&["a", "b"]);
    app.session.input = "KEEP".to_string();
    app.session.input_cursor = app.input_len_chars();
    app.session
        .pending_attachments
        .push(std::path::PathBuf::from("/tmp/keep.png"));

    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    app.handle_key_event(key(KeyCode::Delete)).await;
    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(app.session.input, "KEEP", "the draft must be preserved");
    assert_eq!(app.session.input_cursor, app.input_len_chars());
    assert_eq!(
        app.session.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/keep.png")],
        "staged attachments must be preserved"
    );
}

#[tokio::test]
async fn test_panel_leaves_the_running_turn_untouched() {
    let mut app = panel_with(&["a", "b"]);
    app.session.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.session.cancel_flag = Some(flag.clone());

    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.session.is_processing, "the running turn must not stop");
    assert!(
        !flag.load(Ordering::Relaxed),
        "the panel must not cancel the running turn"
    );
}

#[tokio::test]
async fn test_menu_esc_does_not_open_the_panel() {
    let mut app = support::make_app();
    app.session.input_queue.push_back(entry("a"));
    app.handle_key_event(alt(KeyCode::Char('q'))).await;
    assert!(app.queue_menu_open, "precondition: the menu is open");

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(!app.queue_menu_open, "Esc dismisses the menu");
    assert!(
        !app.queue_show_open,
        "dismissing the menu must not open the panel"
    );
}
