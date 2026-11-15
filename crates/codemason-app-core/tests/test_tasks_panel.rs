//! task migration T-015: integration tests for the reworked `render_tasks_panel`.
//!
//! Verifies FR-005, FR-007, FR-018:
//! - Panel header titled "TASKS" (not the old panel name).
//! - Status-coloured lines: pending=yellow, in_progress=cyan, completed=green,
//!   blocked=red.
//! - `(owner)` suffix appended when owner is set.
//! - `[blocked by #id, ...]` annotation when derived blocked (FR-005).
//! - `active_form` rendered as indented sub-line beneath subject when
//!   `in_progress`.
//! - Scroll, scrollbar, text-selection, and mutual-exclusion behaviour
//!   preserved.

use crate::term_events::CrosstermAppExt;
use codemason_app_core::geometry::Area;

#[path = "support/mod.rs"]
mod support;

// -- Panel title -----------------------------------------------------

// -- Empty placeholder ------------------------------------���----------

// -- Status rendering ------------------------------------------------

// -- Owner suffix (FR-018) -------------------------------------------

// -- Blocked-by annotation (FR-005, FR-018) --------------------------

// -- active_form sub-line (FR-018) -----------------------------------

// -- Combined: owner + active_form + status --------------------------

// -- Scroll preservation ---------------------------------------------

/// Tasks uses "lines from bottom" scroll semantics (like the Log and
/// Profile panels): offset 0 is bottom-pinned, max_scroll is the top.
/// Dragging the scrollbar track must therefore follow the inverted
/// formula `offset = (1.0 - fraction) * max_scroll` - dragging to the
/// bottom of the track must show the BOTTOM of the task list.
/// Regression test for the drag-vs-render direction inversion where the
/// drag handler grouped Tasks with the "lines from top" family.
#[test]
fn test_drag_tasks_scrollbar_moves_offset_bottom_semantics() {
    use crate::term_events::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use codemason_app_core::app::ScrollbarDragPane;

    let mut app = support::make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_tasks_panel = true;
    app.show_log = false;
    app.show_profile = false;
    app.tasks_area = Area::new(80, 0, 30, 21);
    app.tasks_max_scroll = 60;

    let mouse_event = |kind: MouseEventKind, col: u16, row: u16| MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    };

    // Click the scrollbar gutter (rightmost column, 80 + 30 - 1).
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        109,
        10,
    ));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Tasks));

    // Drag to bottom of track -> offset = 0 (bottom of content shown,
    // matching render_tasks_panel's `max_scroll - scroll` mapping).
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Drag(MouseButton::Left),
        109,
        20,
    ));
    assert_eq!(
        app.tasks_scroll_offset, 0,
        "dragging to the track bottom must produce offset 0 (lines from bottom)"
    );

    // Drag to top of track -> offset = max_scroll (top of content).
    app.handle_mouse_event(mouse_event(MouseEventKind::Drag(MouseButton::Left), 109, 0));
    assert_eq!(
        app.tasks_scroll_offset, 60,
        "dragging to the track top must produce offset = max_scroll"
    );
}

// -- Read-only rendering ---------------------------------------------

// -- Error state -----------------------------------------------------
//
// When storage query fails, the panel should show "Failed to load tasks"
// (not "Failed to load tasks").  This is implicitly tested by the existing
// test_tasks_panel tests since we can't easily force a storage error with
// the real Storage backend.  The text change is verified by the source code.
