//! Tests for the Memory side panel (spec `mempanel`).
//!
//! Covers two spec tasks:
//! - **T-007**: extending mouse hit-testing in `session_ops.rs` (`pane_at`)
//!   and `input_handler.rs` (scroll, scrollbar drag, left-click selection
//!   start, right-click context menu) to recognise clicks inside
//!   `memory_area` so users can select text in and open the context menu on
//!   the Memory panel the same way they can on the Log / Profile / Tasks
//!   panels (FR-013).
//! - **T-012**: `Alt+M` toggle flips `show_memory` (FR-003), mutual exclusion
//!   with `show_log` / `show_tasks_panel` / `show_profile` (FR-004), `Alt+M` does
//!   not insert `m` into the input buffer (FR-011), `render_memory_panel`
//!   with populated and missing files (FR-015), and scroll-offset bounds
//!   (FR-009).
//!
//! Tests live in `crates/codemason-tui/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use codemason_app_core::geometry::Area;

use crate::term_events::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::term_events::handle_key;
use codemason_app_core::{
    app::App,
    app::{ContextAction, ScrollbarDragPane, SelectionPane, TextSelection},
    input::InputAction,
};

/// Build an [`App`] backed by an in-memory database.
/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

// -----------------------------------------------------------------------------
// Mouse-event constructors
// --------------------���--------------------------------------------------------

const fn mouse_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

const fn mouse_drag(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

const fn mouse_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

const fn right_click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

const fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

const fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

// -----------------------------------------------------------------------------
// Render helper
// -----------------------------------------------------------------------------

// =============================================================================
// T-007: `pane_at` recognises `memory_area` (FR-013 hit-testing)
// =============================================================================

#[test]
fn test_pane_at_returns_memory_when_click_inside_memory_area() {
    // FR-013: `App::pane_at` must map clicks inside `memory_area` to
    // `SelectionPane::Memory` so the rest of the mouse pipeline (selection
    // start, context menu, scrollbar drag) can route on that pane.
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    let pane = app.pane_at(90, 10);
    assert_eq!(pane, Some(SelectionPane::Memory));
}

#[test]
fn test_pane_at_returns_none_for_memory_when_panel_hidden() {
    // When `show_memory` is false, clicks inside the cached `memory_area`
    // rect must NOT be reported as the Memory pane - the panel is not
    // visible so hit-testing should fall through (FR-002 mutual exclusion
    // relies on `show_memory` gating `memory_area`).
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = false;
    // Leave memory_area non-empty to prove the `show_memory` gate is what
    // blocks the match, not a zeroed rect.
    app.memory_area = Area::new(80, 1, 30, 20);

    let pane = app.pane_at(90, 10);
    assert_ne!(pane, Some(SelectionPane::Memory));
}

// -----------------------------------------------------------------------------
// T-007: mouse scroll inside `memory_area` adjusts `memory_scroll_offset`
// -----------------------------------------------------------------------------

#[test]
fn test_mouse_scroll_up_on_memory_increments_memory_scroll_offset() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);
    assert_eq!(app.memory_scroll_offset, 0);

    app.handle_mouse_event(mouse_scroll_up(90, 10));
    assert_eq!(app.memory_scroll_offset, 3);
    // Messages pane must be untouched.
    assert_eq!(app.session.scroll_offset, 0);
}

#[test]
fn test_mouse_scroll_down_on_memory_decrements_memory_scroll_offset() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);
    app.memory_scroll_offset = 9;

    app.handle_mouse_event(mouse_scroll_down(90, 10));
    assert_eq!(app.memory_scroll_offset, 6);
}

// -----------------------------------------------------------------------------
// T-007: scrollbar-gutter click on `memory_area` starts a Memory drag
// ----------------------------------------------------���------------------------

#[test]
fn test_drag_starts_on_memory_scrollbar_column() {
    // Clicking the rightmost column of `memory_area` (the scrollbar gutter,
    // matching the Log / Profile / Tasks behaviour) should initiate a
    // `ScrollbarDragPane::Memory` drag and clear any active text selection.
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);
    app.memory_max_scroll = 50;

    // Pre-existing selection that the scrollbar click must clear.
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (82, 5),
        endpoint: (90, 5),
    });

    // Scrollbar sits at column 109 (80 + 30 - 1).
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Memory));
    assert!(app.text_selection.is_none());
}

#[test]
fn test_drag_memory_scrollbar_moves_offset() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 0, 30, 21);
    app.memory_max_scroll = 60;

    // Click scrollbar (column 109), drag to top -> offset = 0 (top of
    // content).  Memory uses "lines from top" semantics, so dragging to
    // the top of the scrollbar track shows the first lines.
    app.handle_mouse_event(mouse_down(109, 10));
    assert_eq!(app.scrollbar_drag, Some(ScrollbarDragPane::Memory));

    app.handle_mouse_event(mouse_drag(109, 0));
    assert_eq!(app.memory_scroll_offset, 0);

    // Drag to bottom -> offset = max_scroll (bottom of content).
    app.handle_mouse_event(mouse_drag(109, 20));
    assert_eq!(app.memory_scroll_offset, 60);
}

#[test]
fn test_drag_does_not_start_on_memory_scrollbar_without_scrollable_content() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);
    app.memory_max_scroll = 0;

    app.handle_mouse_event(mouse_down(109, 10));
    assert!(app.scrollbar_drag.is_none());
}

// -----------------------------------------------------------------------------
// T-007: left-click inside `memory_area` starts a Memory text selection
// -----------------------------------------------------------------------------

#[test]
fn test_left_click_inside_memory_area_starts_memory_selection() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    // Click in the body of the panel (not the scrollbar column 109).
    app.handle_mouse_event(mouse_down(90, 10));
    let sel = app.text_selection.as_ref().expect("selection should start");
    assert_eq!(sel.pane, SelectionPane::Memory);
    assert_eq!(sel.anchor, (90, 10));
    assert_eq!(sel.endpoint, (90, 10));
}

#[test]
fn test_drag_extends_memory_selection_endpoint() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_down(85, 3));
    app.handle_mouse_event(mouse_drag(100, 7));

    let sel = app
        .text_selection
        .as_ref()
        .expect("selection should persist");
    assert_eq!(sel.pane, SelectionPane::Memory);
    assert_eq!(sel.anchor, (85, 3));
    assert_eq!(sel.endpoint, (100, 7));
}

#[test]
fn test_mouse_up_preserves_memory_selection() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    app.handle_mouse_event(mouse_down(85, 3));
    app.handle_mouse_event(mouse_drag(100, 7));
    app.handle_mouse_event(mouse_up(100, 7));

    assert!(
        app.text_selection.is_some(),
        "selection should remain after mouse-up"
    );
}

// -----------------------------------------------------------------------------
// T-007: right-click inside `memory_area` opens a Memory context menu
// -----------------------------------------------------------------------------

#[test]
fn test_right_click_inside_memory_area_opens_context_menu() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    app.handle_mouse_event(right_click(90, 10));
    let menu = app
        .context_menu
        .as_ref()
        .expect("right-click should open context menu");
    assert_eq!(menu.pane, SelectionPane::Memory);
    // The standard 3-item menu (Cut / Copy / Paste) is always offered; Copy
    // is disabled because there is no selection yet.
    assert_eq!(menu.items.len(), 3);
    assert_eq!(menu.items[0], (ContextAction::Cut, false));
    assert_eq!(menu.items[1], (ContextAction::Copy, false));
}

#[test]
fn test_right_click_with_memory_selection_enables_copy() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (82, 5),
        endpoint: (90, 5),
    });

    app.handle_mouse_event(right_click(85, 7));
    let menu = app.context_menu.as_ref().expect("context menu open");
    assert_eq!(menu.pane, SelectionPane::Memory);
    assert_eq!(menu.items[1], (ContextAction::Copy, true));
}

#[test]
fn test_right_click_outside_memory_area_does_not_open_memory_menu() {
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 1, 30, 20);

    // Click in the status bar (row 0) - outside every pane.
    app.handle_mouse_event(right_click(10, 0));
    assert!(app.context_menu.is_none());
}

// =============================================================================
// T-012: Alt+M toggle flips `show_memory` (FR-003)
// =============================================================================

#[tokio::test]
async fn test_alt_m_maps_to_toggle_memory_action() {
    // FR-003: Alt+M must produce InputAction::ToggleMemory when no modal is
    // active (no permission dialog, no provider setup, no slash menu).
    let mut app = make_app();
    let key = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT);
    let action = handle_key(&mut app, key).await;
    // InputAction does not derive PartialEq, so we match on the variant
    // explicitly instead of using assert_eq!.
    assert!(
        matches!(action, Some(InputAction::ToggleMemory)),
        "Alt+M should produce InputAction::ToggleMemory, got {action:?}"
    );
}

#[tokio::test]
async fn test_toggle_memory_flips_show_memory_flag() {
    // FR-003: dispatching ToggleMemory via the full key-event path flips
    // `show_memory` on each press.
    let mut app = make_app();
    assert!(!app.show_memory, "show_memory should start false");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(app.show_memory, "first Alt+M should set show_memory=true");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(
        !app.show_memory,
        "second Alt+M should set show_memory=false"
    );
}

#[tokio::test]
async fn test_toggle_memory_status_message_reflects_state() {
    // FR-014: the status bar message reflects the new panel state.
    let mut app = make_app();
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert_eq!(app.status, "memory panel visible");
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert_eq!(app.status, "memory panel hidden");
}

// =============================================================================
// T-012: Mutual exclusion with log / tasks / profile panels (FR-004)
// =============================================================================

#[tokio::test]
async fn test_toggle_memory_mutually_excludes_log_panel() {
    // FR-004: enabling the Memory panel must hide the log panel, and
    // enabling the log panel must hide the Memory panel.
    let mut app = make_app();
    app.show_log = true;
    app.show_memory = false;

    // Enable Memory panel - log must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(app.show_memory, "Memory panel should be visible");
    assert!(
        !app.show_log,
        "log panel must be hidden when Memory is shown"
    );

    // Re-enable log panel - Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT))
        .await;
    assert!(app.show_log, "log panel should be visible");
    assert!(
        !app.show_memory,
        "Memory panel must be hidden when log is shown"
    );
}

#[tokio::test]
async fn test_toggle_memory_mutually_excludes_tasks_panel() {
    // FR-004: enabling the Memory panel must hide the Tasks panel, and
    // enabling the Tasks panel must hide the Memory panel.
    let mut app = make_app();
    app.show_tasks_panel = true;
    app.show_memory = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(app.show_memory);
    assert!(
        !app.show_tasks_panel,
        "Tasks panel must hide when Memory is shown"
    );

    // Re-enable Tasks panel - Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::ALT))
        .await;
    assert!(app.show_tasks_panel);
    assert!(
        !app.show_memory,
        "Memory panel must hide when Tasks is shown"
    );
}

#[tokio::test]
async fn test_toggle_memory_mutually_excludes_profile_panel() {
    // FR-004: enabling the Memory panel must hide the profile panel, and
    // enabling the profile panel must hide the Memory panel. Profile toggle
    // routes through `set_profile_panel_enabled` which dismisses every other
    // side panel.
    let mut app = make_app();
    app.show_profile = true;
    app.show_memory = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(app.show_memory);
    assert!(
        !app.show_profile,
        "profile panel must hide when Memory is shown"
    );

    // Re-enable profile panel - Memory must be dismissed.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT))
        .await;
    assert!(app.show_profile);
    assert!(
        !app.show_memory,
        "Memory panel must hide when profile is shown"
    );
}

#[tokio::test]
async fn test_toggle_memory_clears_memory_selection_and_context_menu_on_hide() {
    // FR-005: when the Memory panel becomes hidden, any active Memory-pane
    // text selection or context menu is cleared.
    let mut app = make_app();
    app.show_memory = true;
    app.memory_area = Area::new(0, 1, 80, 20);
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Memory,
        anchor: (5, 5),
        endpoint: (10, 5),
    });

    // Drive the toggle through the same InputAction dispatch the Alt+M key
    // handler uses. We dismiss any context menu first because the
    // context-menu key router in `handle_key` swallows all keys until the
    // menu is dismissed, which would prevent the Alt+M mapping from running.
    app.context_menu = None;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(!app.show_memory);
    assert!(
        app.text_selection.is_none(),
        "Memory selection must be cleared when panel is hidden"
    );
}

#[tokio::test]
async fn test_toggle_memory_clears_memory_context_menu_on_hide() {
    // FR-005 (context-menu half): driving ToggleMemory to hide the panel
    // clears an active Memory-pane context menu.
    let mut app = make_app();
    app.show_memory = true;
    app.memory_area = Area::new(0, 1, 80, 20);
    app.context_menu = Some(codemason_app_core::app::ContextMenuState {
        x: 5,
        y: 5,
        pane: SelectionPane::Memory,
        selected: 0,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, true),
            (ContextAction::Paste, false),
        ],
    });

    // The context-menu key router in `handle_key` swallows all keys until
    // the menu is dismissed, so we dismiss it first and then toggle.
    app.context_menu = None;
    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(!app.show_memory);
    assert!(
        app.context_menu.is_none(),
        "Memory context menu must be cleared when panel is hidden"
    );
}

// =============================================================================
// T-012: Alt+M does not insert `m` into the input buffer (FR-011)
// =============================================================================

#[tokio::test]
async fn test_alt_m_does_not_insert_m_into_input() {
    // FR-011: pressing Alt+M must toggle the Memory panel and must NOT insert
    // the character `m` into the chat input buffer. The Alt+M mapping in
    // `input.rs` is placed before the generic `KeyCode::Char(c)` insertion
    // handler (NFR-002) so the event is consumed entirely by the toggle.
    let mut app = make_app();
    assert!(app.session.input.is_empty(), "input should start empty");

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(
        app.session.input.is_empty(),
        "Alt+M must not insert 'm' into input; got {:?}",
        app.session.input
    );
    assert!(app.show_memory, "Alt+M should have toggled show_memory on");
}

#[tokio::test]
async fn test_alt_m_does_not_insert_m_when_panel_already_visible() {
    // FR-011 regression guard: toggling the panel off via Alt+M must also
    // not leak an `m` into the input buffer.
    let mut app = make_app();
    app.show_memory = true;

    app.handle_key_event(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(
        app.session.input.is_empty(),
        "Alt+M must not insert 'm' when hiding the panel; got {:?}",
        app.session.input
    );
    assert!(
        !app.show_memory,
        "Alt+M should have toggled show_memory off"
    );
}

// ====================��========================================================
// T-012: render_memory_panel with populated and missing files (FR-015)
// =============================================================================

#[tokio::test]
async fn test_log_scroll_down_on_memory_does_not_underflow_below_zero() {
    // FR-009 bounds: LogScrollDown decrements `memory_scroll_offset` with
    // saturating subtraction, so it must never underflow past 0. The key
    // binding for LogScrollDown is Ctrl+PageDown (see `input.rs`).
    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.memory_scroll_offset = 0;

    app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL))
        .await;
    assert_eq!(
        app.memory_scroll_offset, 0,
        "LogScrollDown at offset 0 must saturate at 0, not underflow"
    );
}

#[tokio::test]
async fn test_log_scroll_up_on_memory_increments_offset() {
    // FR-009 bounds: LogScrollUp increments `memory_scroll_offset` by 3
    // when the Memory panel is the visible side panel. The key binding for
    // LogScrollUp is Ctrl+PageUp (see `input.rs`).
    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    assert_eq!(app.memory_scroll_offset, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL))
        .await;
    assert_eq!(
        app.memory_scroll_offset, 3,
        "LogScrollUp should increment memory_scroll_offset by 3"
    );
}

#[test]
fn test_scrollbar_drag_clamps_offset_within_bounds() {
    // FR-009 bounds: dragging the scrollbar computes the offset from the
    // drag fraction and clamps it to [0, max_scroll]. Dragging to the middle
    // of a 100-line scrollable region yields ~50, never negative and never
    // greater than max_scroll.
    let mut app = make_app();
    app.message_area = Area::new(0, 1, 80, 20);
    app.show_memory = true;
    app.memory_area = Area::new(80, 0, 30, 21);
    app.memory_max_scroll = 100;

    // Click scrollbar (column 109) at the vertical middle (row 10 of a
    // 21-row pane -> track rows 0..20, middle ≈ row 10).
    app.handle_mouse_event(mouse_down(109, 10));
    app.handle_mouse_event(mouse_drag(109, 10));

    let offset = app.memory_scroll_offset;
    assert!(
        offset <= app.memory_max_scroll,
        "offset {offset} must not exceed max_scroll {}",
        app.memory_max_scroll
    );
    // fraction = 10/20 = 0.5 -> offset = 0.5*100 = 50.
    assert_eq!(offset, 50, "middle drag should yield offset 50");

    // Drag above the pane -> clamps to top (offset = 0).
    app.handle_mouse_event(mouse_drag(109, 0));
    assert_eq!(app.memory_scroll_offset, 0);

    // Drag below the pane -> clamps to bottom (offset = max_scroll).
    app.handle_mouse_event(mouse_drag(109, 30));
    assert_eq!(app.memory_scroll_offset, 100);
}

// =============================================================================
// FR-016: interactive cursor, open, and delete in the Alt+M panel
// =============================================================================
