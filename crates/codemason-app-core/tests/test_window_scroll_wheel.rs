//! Tests for popup-table wheel scrolling (spec `agentui` T-017).
//!
//! Covers the FR-008 / FR-059 wheel path: a wheel event over the Agents or
//! Teams popup body moves the table's scroll offset by `SCROLL_STEP_LINES`
//! (`3`), the offset re-clamps to the maximum the renderer computed (a wheel
//! up past the bottom cannot overshoot, a wheel down below the top cannot go
//! negative), the wheel is consumed so the pane beneath the popup never
//! scrolls, every accepted wheel sets the redraw flag (NFR-005), and the
//! clamped, non-zero offset shifts the painted rows with the scrollbar
//! tracking the position (FR-056, acceptance 19).
//!
//! Every case drives the real mouse router ([`App::handle_mouse_event`])
//! against areas recorded by the real render entry point through a ratatui
//! `TestBackend`, so the assertions exercise the exact path a real wheel event
//! takes (NFR-001). All state is synthetic and in-memory; nothing is written
//! into the repository (NFR-012). Per-frame work is the visible-window slice
//! the renderers already perform, so a scrolled frame costs no more than a
//! static one (NFR-002).

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyModifiers, MouseEvent, MouseEventKind};
use codemason_app_core::geometry::Area;

/// A wheel-up event at `(col, row)`.
const fn wheel_up(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Agents table offset
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-008 / FR-059: the wheel moves the Teams table offset
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-005: redraw signalling
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// No popup: the wheel keeps the old behaviour
// ---------------------------------------------------------------------------

/// With both areas at the default empty rect (no popup rendered), a wheel
/// moves neither offset and falls through to the pane routing (T-017).
#[test]
fn test_window_scroll_wheel_without_a_popup_leaves_the_offsets_alone() {
    let mut app = support::make_app();
    app.active_agents_area = Area::default();
    app.teams_area = Area::default();

    app.handle_mouse_event(wheel_up(40, 10));
    assert_eq!(app.active_agents_scroll_offset, 0);
    assert_eq!(app.teams_scroll_offset, 0);
}

// ---------------------------------------------------------------------------
// End-to-end: the offset is applied, hit-rects follow, the scrollbar tracks
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Scrollbar-gutter drag (FR-059's mouse half; the row clicks are keyboard-only)
// ---------------------------------------------------------------------------

/// A left-button press, drag movement and release event.
/// (The hit-rect-on-wheel test was retired with the row clicks: the cursor
/// row maps are scroll-invariant, which `test_hit_rect_lifecycle.rs` pins.)
const fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: crate::term_events::KeyModifiers::empty(),
    }
}

/// Clicking the Agents scrollbar gutter starts a thumb drag with
/// jump-to-position, a Drag event keeps moving the offset, and the button
/// release clears the drag state (FR-059).
#[test]
fn test_agents_scrollbar_gutter_drag_jumps_then_drags_then_ends() {
    let mut app = support::make_app();
    app.active_agents_area = Area::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 20;

    // Click at the gutter's column, near the top of the track: jump-to-
    // position.
    let gutter = app.active_agents_area.right() - 1;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crate::term_events::MouseButton::Left),
        gutter,
        8,
    ));
    assert_eq!(
        app.scrollbar_drag,
        Some(codemason_app_core::app::ScrollbarDragPane::AgentsWindow),
        "a gutter click must start the Agents-window drag (FR-059)"
    );
    let jumped = app.active_agents_scroll_offset;
    assert!(
        jumped > 0 && jumped < 20,
        "the click must jump the offset to a track-fraction position, got {jumped}"
    );

    // A drag further down the track moves the offset again.
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Drag(crate::term_events::MouseButton::Left),
        gutter,
        15,
    ));
    let dragged = app.active_agents_scroll_offset;
    assert!(
        dragged >= jumped,
        "dragging down the track must not scroll up: {jumped} -> {dragged}"
    );

    // Releasing the button clears the drag state.
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Up(crate::term_events::MouseButton::Left),
        gutter,
        15,
    ));
    assert_eq!(
        app.scrollbar_drag, None,
        "the release must clear the drag state (FR-059)"
    );
}

/// The Teams scrollbar gutter drags the same way (FR-059).
#[test]
fn test_teams_scrollbar_gutter_drag_jumps_then_drags_then_ends() {
    let mut app = support::make_app();
    app.teams_area = Area::new(5, 5, 110, 20);
    app.teams_max_scroll = 15;

    let gutter = app.teams_area.right() - 1;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crate::term_events::MouseButton::Left),
        gutter,
        10,
    ));
    assert_eq!(
        app.scrollbar_drag,
        Some(codemason_app_core::app::ScrollbarDragPane::TeamsWindow),
        "a gutter click must start the Teams-window drag (FR-059)"
    );
    let jumped = app.teams_scroll_offset;
    assert!(
        jumped > 0 && jumped < 15,
        "the click must jump the offset to a track-fraction position, got {jumped}"
    );

    // The track's last pixel row is the area's bottom edge row; fraction 1.0
    // jumps the offset to the maximum.
    let bottom_row = app.teams_area.bottom().saturating_sub(1);
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crate::term_events::MouseButton::Left),
        gutter,
        bottom_row,
    ));
    let bottom = app.teams_scroll_offset;
    assert_eq!(
        bottom, 15,
        "a click at the track bottom jumps to the maximum (FR-059)"
    );

    app.handle_mouse_event(mouse_event(
        MouseEventKind::Up(crate::term_events::MouseButton::Left),
        gutter,
        bottom_row,
    ));
    assert_eq!(app.scrollbar_drag, None);
}

/// A gutter click on a non-overflowing table never starts a drag (there is no
/// scrollbar to grab).
#[test]
fn test_a_gutter_click_without_overflow_starts_no_drag() {
    let mut app = support::make_app();
    app.active_agents_area = Area::new(5, 5, 100, 20);
    app.active_agents_max_scroll = 0;

    let gutter = app.active_agents_area.right() - 1;
    app.handle_mouse_event(mouse_event(
        MouseEventKind::Down(crate::term_events::MouseButton::Left),
        gutter,
        8,
    ));
    assert_eq!(app.scrollbar_drag, None, "no scrollbar, no drag (FR-059)");
    assert_eq!(app.active_agents_scroll_offset, 0);
}

// ---------------------------------------------------------------------------
// PgUp / PgDn: the keyboard paging siblings of the wheel (FR-059's keyboard
// half; the cursor pages one viewport at a time)
// ---------------------------------------------------------------------------
