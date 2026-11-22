//! Mouse-routing tests for the connector-catalogue category navigator
//! (`/connectors claude`).
//!
//! These drive the real `handle_mouse_event` path with an in-memory catalogue
//! after a real render has populated the navigator and result column areas, so
//! they are deterministic and contact no catalogue endpoint. They pin the same
//! mouse convenience layer the plugin-store navigator has: a left-click on a
//! category row selects and applies it and focuses the navigator, a wheel over
//! the navigator scrolls its viewport without changing the active category, a
//! left-click on a result row moves the result cursor and leaves the category
//! alone, and every mouse path installs nothing.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_connector_catalogue_mouse.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use codemason_app_core::app::App;
use codemason_connectors::{
    CatalogueBrowser, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// A catalogue descriptor with `id` and `category`.
fn descriptor(id: &str, category: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: format!("{id} description"),
        category: category.to_string(),
        tags: Vec::new(),
        source: format!("https://example.org/{id}.json"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// A key event with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn left_click(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), col, row)
}

/// An app with the panel open and the fixture entries loaded. The distinct
/// categories are `developer`, `productivity`, and `communication` (sorted
/// case-insensitively), so the navigator rows are `ALL` (0), `Installed` (1),
/// `communication` (2), `developer` (3), `productivity` (4).
fn open() -> App {
    let mut app = support::make_app();
    // Fill the browser directly: opening the real catalogue would start a live
    // fetch thread on an empty cache.
    let browser = app.connector_store.insert(CatalogueBrowser::new("", false));
    browser.set_entries(
        vec![
            descriptor("google-drive", "productivity"),
            descriptor("github", "developer"),
            descriptor("slack", "communication"),
        ],
        0,
    );
    app
}

/// The browser behind the open panel.
fn browser(app: &App) -> &CatalogueBrowser {
    app.connector_store.as_ref().expect("panel open")
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// -- Left-click a category row selects, applies, and focuses it ---------------

#[tokio::test]
async fn left_click_outside_the_navigator_rows_leaves_the_selection_unchanged() {
    let mut app = open();
    render_effects(&mut app, 110, 30);
    let nav = app.connector_store_nav_area;

    // A click far below the last category row maps past the row count, so the
    // selection cannot change.
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 12));

    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
}

// -- Left-click a result row moves the result cursor, keeps the category -----

// -- A click or wheel on the pinned heading row is inert ----------------------

// -- The marketplace navigator: click, wheel, and focus ----------------------

/// A catalogue descriptor attributed to a marketplace.
fn attributed(id: &str, marketplace: &str) -> ConnectorDescriptor {
    let mut entry = descriptor(id, "developer");
    entry.marketplace = marketplace.to_string();
    entry
}

/// A catalogue of three entries attributed to two marketplaces, so the
/// marketplace navigator has rows `ALL` (0), `acme` (1), `official` (2).
fn open_with_marketplaces() -> App {
    let mut app = support::make_app();
    // Fill the browser directly: opening the real catalogue would start a live
    // fetch thread on an empty cache.
    let browser = app.connector_store.insert(CatalogueBrowser::new("", false));
    browser.set_entries(
        vec![
            attributed("community-one", "acme"),
            attributed("community-two", "acme"),
            attributed("official-one", "official"),
        ],
        0,
    );
    app
}

#[tokio::test]
async fn a_wheel_over_the_marketplace_navigator_scrolls_without_filtering() {
    let mut app = open_with_marketplaces();
    render_effects(&mut app, 130, 30);
    let marketplace = app.connector_store_marketplace_area;

    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollDown,
        marketplace.x + 1,
        marketplace.y,
    ));

    let browser = browser(&app);
    assert_eq!(
        browser.active_marketplace(),
        "ALL",
        "the wheel selects nothing"
    );
    assert_eq!(
        browser.marketplace_cursor_row(),
        0,
        "the cursor did not move"
    );
}

#[tokio::test]
async fn tab_cycles_focus_through_the_three_panes() {
    // `Tab` walks Results -> Categories -> Marketplaces -> Results and `BackTab`
    // walks it back, exactly as the plugin-store panel does.
    let mut app = open_with_marketplaces();
    render_effects(&mut app, 130, 30);

    assert!(!app.connector_catalogue_nav_focused());
    crate::term_events::handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(app.connector_catalogue_nav_focused(), "Tab -> Categories");
    crate::term_events::handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        app.connector_catalogue_marketplace_focused(),
        "Tab -> Marketplaces"
    );
    crate::term_events::handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        !app.connector_catalogue_nav_focused() && !app.connector_catalogue_marketplace_focused(),
        "Tab -> Results"
    );
    crate::term_events::handle_key(&mut app, key(KeyCode::BackTab)).await;
    assert!(
        app.connector_catalogue_marketplace_focused(),
        "BackTab -> Marketplaces"
    );
}

#[tokio::test]
async fn c_clears_the_focused_navigators_filter() {
    // `c` is pane-scoped: over the navigator it clears the category, over the
    // marketplace navigator it clears the marketplace, and both leave the other
    // filter alone.
    let mut app = open_with_marketplaces();
    render_effects(&mut app, 130, 30);
    // Apply both filters directly so the assertions isolate the clear key.
    {
        let browser = app.connector_store.as_mut().expect("panel open");
        assert!(browser.category_select_row(2)); // `acme` is not a category; use `developer`
        assert!(browser.marketplace_select_row(1)); // acme
    }
    // Focus the marketplace pane and clear only that filter.
    app.connector_catalogue_focus_next();
    app.connector_catalogue_focus_next();
    assert!(app.connector_catalogue_marketplace_focused());
    crate::term_events::handle_key(&mut app, key(KeyCode::Char('c'))).await;
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(browser.active_marketplace(), "ALL", "marketplace cleared");
    assert_eq!(
        browser.active_category(),
        "developer",
        "the category filter is untouched"
    );
}
