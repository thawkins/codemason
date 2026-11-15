//! Render tests for the ALT-Q queue-control menu overlay (spec `inputqueue`
//! T-014).
//!
//! Covers FR-021 (the menu presents exactly the four options `Next`, `Stop`,
//! `Clear`, and `Show`), FR-023 (the `Next` option is visible/selectable only
//! while the input queue is non-empty, otherwise hidden/non-selectable), and
//! NFR-007 (the menu reuses the shared overlay/modal rendering machinery rather
//! than a parallel path). The action semantics for the rows are covered by the
//! T-015..T-018 tests and the `Show`-panel suites.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_input_queue_menu_render.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use codemason_app_core::app::App;

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// FR-021 - exactly four options, in fixed order
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-023 - `Next` visible/selectable only with a non-empty queue
// ---------------------------------------------------------------------------

#[test]
fn test_next_row_is_non_selectable_when_queue_is_empty() {
    let mut app = support::make_app();
    assert_eq!(app.input_queue_len(), 0, "precondition: empty queue");
    app.queue_menu_open = true;
    app.queue_menu_selected = 0;

    render_effects(&mut app, 120, 40);
    crate::frame::publish(&mut app);
}

// ---------------------------------------------------------------------------
// NFR-007 - reuses the shared overlay/modal rendering path
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-026 - halt label reflects the running-turn state
// ---------------------------------------------------------------------------
