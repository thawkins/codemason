//! Deep coverage for `codemason_tui::input` key handling and the
//! `App` keyboard/mouse dispatch in `src/app/input_handler.rs`.
//!
//! Everything here drives the public entry points only -
//! `App::handle_key_event`, `App::handle_mouse_event`,
//! `codemason_tui::input::handle_key` - and constructs `crossterm::event` values
//! directly, exactly as the terminal event loop does. No production logic is
//! changed, no network call is awaited and no real TTY is used.
//!
//! Focus areas (the arms the existing suites do not reach):
//!   * the provider-setup dialog state machine for every `ProviderSetupStep`
//!     variant, entered through `App::handle_key_event` so the modal dispatch
//!     in `input::handle_key` is exercised too,
//!   * the router cluster panel and its model-picker sub-dialog,
//!   * the telemetry multi-field form (`input::telemetry_field`),
//!   * the general editing / navigation ladder in `input::handle_key`,
//!   * mouse routing: wheel scrolling of every side panel, drag-select, the
//!     right-click context menu's copy/cut rows, and the plugin-store and
//!     connector-catalogue panels' click/wheel columns,
//!   * the history / config-save pickers and the memory-delete confirmation.

mod support;

use codemason_tui::crossterm_input::CrosstermAppExt;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// A plain (no-modifier) key press.

/// A key press with the given modifiers.

/// A mouse event with no modifiers.
const fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// Render one frame so the layout caches every pane/panel area and scroll
/// maximum the mouse hit-testing reads back.
fn render(app: &mut codemason_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("draw");
}

/// Index of `pid` in `PROVIDER_LIST`.

/// A model-picker entry with `id` and `thinking` levels.

/// An `EnterKey` setup step for `provider_id` with the key text `key_text`.

/// A `GitLabSetup` step focused on `active_field`.

/// A `TelemetrySetup` step focused on `active_field`.

/// A `SetupRouter` step over one selected provider.

/// Persist `ids` as the discovered model list for `provider_id` so
/// `App::models_for_provider` returns them without a network call.

// ===========================================================================
// Provider setup: SelectProvider through the public dispatcher
// ===========================================================================

// ===========================================================================
// Provider setup: EnterKey editing through the public dispatcher
// ===========================================================================

// ===========================================================================
// Provider setup: SelectModel / SelectThinkingLevel
// ===========================================================================

// ===========================================================================
// Provider setup: GitLab form and validation spawn
// ===========================================================================

// ===========================================================================
// Provider setup: telemetry multi-field form
// ===========================================================================

// ===========================================================================
// Provider setup: router cluster panel and its model picker
// ===========================================================================

// ===========================================================================
// input::handle_key - the general editing / navigation ladder
// ===========================================================================

// ===========================================================================
// Mouse: wheel scrolling of every pane
// ===========================================================================

// ===========================================================================
// Mouse: drag selection, release, scrollbar drag
// ===========================================================================

// ===========================================================================
// Mouse + context menu: copy and cut from a pane
// ===========================================================================

/// A log pane holding two copy-able rows.

// ===========================================================================
// History picker and config-save picker key routing
// ===========================================================================

// ===========================================================================
// Memory delete confirmation
// ===========================================================================

// ===========================================================================
// Plugin-store and connector-catalogue mouse columns
// ===========================================================================

#[test]
fn plugin_store_mouse_clicks_navigator_results_and_clear_button() {
    use codemason_plugins::StoreKind;
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "que", false);
    render(&mut app, 120, 40);

    let nav = app.plugin_store_nav_area;
    let results = app.plugin_store_result_area;
    let clear = app.plugin_store_clear_area;

    // Clicking a navigator row focuses and selects it.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), nav.x, nav.y));
    assert_eq!(
        app.plugin_store.as_ref().expect("open").focus,
        codemason_tui::app::StorePane::Categories
    );
    // Wheel over the navigator and the results scrolls them.
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x, nav.y));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x, nav.y));
    if results.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            results.x,
            results.y,
        ));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, results.x, results.y));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, results.x, results.y));
    }
    // The search-box clear button wipes the live filter.
    if clear.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            clear.x,
            clear.y,
        ));
        assert_eq!(app.plugin_store.as_ref().expect("open").query.len(), 0);
    }
    // Anything else over the modal is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 119, 39));
}

#[test]
fn connector_catalogue_mouse_clicks_navigator_and_results() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", codemason_connectors::CategoryFilter::all(), false);
    render(&mut app, 110, 30);

    let nav = app.connector_store_nav_area;
    let results = app.connector_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), nav.x, nav.y));
    assert_eq!(
        app.connector_store.as_ref().expect("open").focus,
        codemason_connectors::CataloguePane::Categories,
        "a navigator click focuses the category column"
    );
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x, nav.y));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x, nav.y));
    if results.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            results.x,
            results.y,
        ));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, results.x, results.y));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, results.x, results.y));
    }
    // A wheel/click over the modal but outside both columns is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 109, 29));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 109, 29));
}
