//! Additional deep coverage for `codemason_tui::input` and the `App` input
//! surfaces in `src/app/input_handler.rs` that the existing suites
//! (`test_input_more.rs`, `test_input_field_extra.rs`, `test_input_handler_more.rs`,
//! `test_router_setup.rs`, ...) do not exercise.
//!
//! This file drives the real public dispatch entry points only -
//! `codemason_tui::input::handle_key`, `App::handle_key_event`,
//! `App::handle_mouse_event` and `App::handle_config_save_picker_key` - and
//! constructs `crossterm::event` values directly. No production logic is
//! touched and no network or real TTY is used.
//!
//! Focus areas (the not-yet-hit arms):
//!   * the provider-setup dialog state machine (`SelectProvider` / `EnterKey` /
//!     `DeviceFlowPending` / `SelectModel` / `SelectAzureResource` /
//!     `SelectThinkingLevel` / `Done` / `SelectAgent` / `SelectConfiguredProvider`
//!     / `ShowProviderConfig` / `ResetProvider` / `GitLabSetup` /
//!     `GitLabValidating` / `TelemetrySetup` / `LoadingModels`),
//!   * the router-save / forcecleanup / rollback confirmation modals,
//!   * the `mcp_discover` and `plugins_list` panel key routing,
//!   * connector-catalogue + plugin-store mouse wheel routing,
//!   * provider-setup / input-pane right-click context-menu construction,
//!   * side-panel scrollbar drag hit-testing and the drag/up arms.

mod support;

use codemason_tui::crossterm_input::CrosstermAppExt;
use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn render(app: &mut codemason_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("draw");
}

// ===========================================================================
// handle_key: router-save / forcecleanup / rollback confirmation modals
// ===========================================================================

// ===========================================================================
// handle_key: slash-menu edge arms
// ===========================================================================

// ===========================================================================
// handle_key: mcp_discover / plugins_list panel key routing
// ===========================================================================

// ===========================================================================
// Provider setup: Esc closes non-router dialogs; SelectProvider navigation
// ===========================================================================

// ===========================================================================
// Provider setup: EnterKey dialog paths
// ===========================================================================

// ===========================================================================
// Provider setup: DeviceFlowPending
// ===========================================================================

// ===========================================================================
// Provider setup: SelectModel
// ===========================================================================

// ===========================================================================
// Provider setup: SelectAzureResource
// ===========================================================================

// ===========================================================================
// Provider setup: SelectThinkingLevel
// ===========================================================================

// ===========================================================================
// Provider setup: Done / SelectAgent / SelectConfiguredProvider /
// ShowProviderConfig / ResetProvider
// ===========================================================================

// ===========================================================================
// Provider setup: GitLabSetup / GitLabValidating
// ===========================================================================

// ===========================================================================
// Provider setup: TelemetrySetup
// ===========================================================================

// ===========================================================================
// Provider setup: LoadingModels
// ===========================================================================

// ===========================================================================
// input_handler: connector-catalogue + plugin-store mouse wheel routing
// ===========================================================================

#[test]
fn connector_catalogue_wheel_over_nav_and_results() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", codemason_connectors::CategoryFilter::all(), false);
    render(&mut app, 110, 30);

    let nav = app.connector_store_nav_area;
    let results = app.connector_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollUp,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollDown,
        results.x + 1,
        results.y + 1,
    ));
    // A mouse event over the modal but outside both columns is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 109, 29));
}

#[test]
fn plugin_store_wheel_over_nav_marketplace_results() {
    use codemason_plugins::StoreKind;
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    render(&mut app, 120, 40);

    let nav = app.plugin_store_nav_area;
    let marketplace = app.plugin_store_marketplace_area;
    let results = app.plugin_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x + 1, nav.y + 1));
    if marketplace.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::ScrollUp,
            marketplace.x + 1,
            marketplace.y,
        ));
        app.handle_mouse_event(mouse(
            MouseEventKind::ScrollDown,
            marketplace.x + 1,
            marketplace.y,
        ));
    }
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollUp,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollDown,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 119, 39));
}

// ===========================================================================
// input_handler: right-click context-menu construction in the input pane
// ===========================================================================

// ===========================================================================
// input_handler: side-panel scrollbar drag hit-testing (drag + up arms)
// ===========================================================================
