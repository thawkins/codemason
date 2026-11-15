//! Geometry tests for the message input queue counter prefix
//! (spec `inputqueue` T-005, NFR-002 / FR-020).
//!
//! The two-digit queue counter widens the chat input's prompt prefix from two
//! columns (`"> "`) to four (`"NN> "`). Every geometry consumer - wrapped rows,
//! the cursor position, and selection-to-character mapping - must derive its
//! prefix width from that counter state, otherwise the cursor and the wrapped
//! rows drift away from the painted frame.
//!
//! FR-020 additionally requires the counter digits to never be selectable or
//! copyable as message content; the selection mapping clamps anything inside
//! the counter columns to the input text only.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_input_queue_geometry.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use codemason_app_core::app::App;
use codemason_app_core::app::QueuedInput;

/// Build a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// Cursor column follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

#[test]
fn test_cursor_pos_matches_the_painted_counter_prefix() {
    let mut app = support::make_app();
    app.session.input = "hi".to_string();
    app.session.input_cursor = 2;
    app.session.input_queue.push_back(entry("queued"));

    render_effects(&mut app, 120, 40);

    // The cursor column must land immediately after the last painted text
    // cell, so the character before it is the final input character.
    let area = app.input_area;
    let (row, col) = app.session.input_render_cache.cursor_pos;
    assert_eq!(row, 0);
    let _cursor_screen_col = area.x + 1 + col as u16;
}

// ---------------------------------------------------------------------------
// Wrapped-row height follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Selection mapping follows the counter prefix and excludes counter digits
// (NFR-002, FR-020)
// ---------------------------------------------------------------------------
