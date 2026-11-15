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

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::geometry::Area;

use codemason_app_core::app::{
    ContextAction, ContextMenuState, QueuedInput, SelectionPane, TextSelection,
};

/// Build a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Cursor column follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Wrapped-row height follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Selection mapping follows the counter prefix and excludes counter digits
// (NFR-002, FR-020)
// ---------------------------------------------------------------------------

/// Build a Cut context menu over the Input pane so `execute_context_action`
/// routes the selection through `input_selection_char_range`.
fn cut_menu() -> ContextMenuState {
    ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Input,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    }
}

#[test]
fn test_selection_maps_through_the_counter_prefix() {
    let mut app = support::make_app();
    app.input_area = Area::new(0, 22, 40, 3);
    app.session.input = "abcdef".to_string();
    app.session.input_cursor = 6;
    app.session.input_queue.push_back(entry("queued"));
    // Painted first row is `01> abcdef`, so inner columns 6..8 are `c` and `d`.
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (1 + 6, 23),
        endpoint: (1 + 7, 23),
    });
    app.context_menu = Some(cut_menu());

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(
        app.session.input, "abef",
        "cutting the counter-prefixed row must remove only `cd` (NFR-002)"
    );
}

#[test]
fn test_selection_over_counter_digits_is_not_message_content() {
    let mut app = support::make_app();
    app.input_area = Area::new(0, 22, 40, 3);
    app.session.input = "abcdef".to_string();
    app.session.input_cursor = 6;
    app.session.input_queue.push_back(entry("queued"));
    // Select the entire `01> ` counter prefix (inner columns 0..4).
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (1, 23),
        endpoint: (1 + 3, 23),
    });
    app.context_menu = Some(cut_menu());

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(
        app.session.input, "abcdef",
        "the queue counter is never selectable or cut as message content (FR-020)"
    );
}
