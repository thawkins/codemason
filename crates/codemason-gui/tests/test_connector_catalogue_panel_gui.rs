//! Render tests for the connector-catalogue browse panel (`/connectors claude`).
//!
//! The panel is the interactive peer of `/plugins claude` for the connector
//! catalogue. These drive the real renderer and the real key routing through a
//! `TestBackend` and read the painted buffer back, so no catalogue endpoint is
//! contacted: the browser state is filled directly.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_connector_catalogue_panel.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;
use codemason_connectors::{
    CatalogueBrowser, CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId,
    ConnectorServer,
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

fn sample() -> Vec<ConnectorDescriptor> {
    vec![
        descriptor("google-drive", "productivity"),
        descriptor("github", "developer"),
        descriptor("slack", "communication"),
    ]
}

/// An app with the panel open and the given entries/installed set loaded. The
/// fetch is never attempted (no async reactor), so the entries are installed
/// directly.
fn open(entries: Vec<ConnectorDescriptor>, installed: &[&str], skipped: usize) -> App {
    let mut app = support::make_app();
    // Fill the browser directly: opening the real catalogue would start a live
    // fetch thread on an empty cache.
    let browser = app.connector_store.insert(CatalogueBrowser::new("", false));
    browser.set_installed(installed.iter().map(|s| (*s).to_string()).collect());
    browser.set_entries(entries, skipped);
    app
}

/// An app with the panel open on `category`, populated from `entries` exactly as
/// [`App::poll_store_cache`] fills it after an off-loop fetch:
/// the entries land first and the launch category is applied against the real
/// category set.
fn open_with_category(entries: Vec<ConnectorDescriptor>, category: CategoryFilter) -> App {
    let mut app = support::make_app();
    let browser = app.connector_store.insert(CatalogueBrowser::new("", false));
    browser.set_pending_category(category);
    browser.set_entries(entries, 0);
    browser.apply_pending_category();
    app
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// The panel is a bordered, titled modal with Unicode box-drawing borders
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rows, the block cursor, and the installed marker
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Explicit state lines instead of a blank list
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Key routing while the panel is open
// ---------------------------------------------------------------------------

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ---------------------------------------------------------------------------
// The empty-query dismiss leaves the area cleared on the next frame
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Launch category filter (`/connectors claude --category <name>`)
// ---------------------------------------------------------------------------

#[test]
fn a_launch_category_the_catalogue_lacks_is_refused_instead_of_emptying_the_panel() {
    let mut app = open_with_category(sample(), CategoryFilter::parse("nosuch"));
    // The refusal is reported, and the panel keeps every row visible rather than
    // rendering an empty list that looks like a catalogue with no matches.
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(browser.selected_category_label(), "ALL");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("unknown category `nosuch`")
    );
    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
}

#[tokio::test]
async fn c_clears_the_category_filter_back_to_all() {
    let mut app = open_with_category(sample(), CategoryFilter::parse("developer"));
    // With `developer` active the panel shows only the matching row; `c` is the
    // navigator's clear key: it restores the unfiltered `ALL` view rather than
    // stepping to the next category (the navigator rows do the stepping).
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(browser.selected_category_label(), "developer");
    }
    crate::term_events::handle_key(&mut app, key(KeyCode::Char('c'))).await;
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(
        browser.selected_category_label(),
        "ALL",
        "`c` clears the active category back to ALL"
    );
    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
    crate::frame::publish(&mut app);
}

#[tokio::test]
async fn tab_routes_up_down_to_the_category_navigator() {
    let mut app = open(sample(), &[], 0);
    // Result rows focused by default: Down moves the result cursor and leaves
    // the category alone.
    crate::term_events::handle_key(&mut app, key(KeyCode::Down)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(browser.cursor, 1, "Down moved the result cursor");
        assert_eq!(browser.selected_category_label(), "ALL");
    }
    // Tab hands focus to the navigator: Down applies the highlighted category on
    // the same keypress. Navigator row 1 is the `Installed` pseudo-category, so
    // the first declared category (`communication`) is row 2.
    crate::term_events::handle_key(&mut app, key(KeyCode::Tab)).await;
    crate::term_events::handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(
        app.connector_store
            .as_ref()
            .expect("panel open")
            .selected_category_label(),
        "Installed",
        "row 1 is the Installed pseudo-category"
    );
    crate::term_events::handle_key(&mut app, key(KeyCode::Down)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(
            browser.selected_category_label(),
            "communication",
            "Down over the navigator applies the highlighted category"
        );
    }
    // Enter over the navigator applies the category and hands focus back to the
    // result list; the next Down moves the result cursor again.
    crate::term_events::handle_key(&mut app, key(KeyCode::Enter)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(
            browser.focus,
            codemason_connectors::CataloguePane::Results,
            "Enter over the navigator returns focus to the results"
        );
        assert_eq!(
            browser.selected_category_label(),
            "communication",
            "the applied category sticks"
        );
    }
    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
    crate::frame::publish(&mut app);
}

// ---------------------------------------------------------------------------
// FR-044 - the marketplace column and the enrichment loading hint
// ---------------------------------------------------------------------------

/// A descriptor whose `marketplace` names the source it came from.
fn attributed(id: &str, marketplace: &str) -> ConnectorDescriptor {
    let mut entry = descriptor(id, "developer");
    entry.marketplace = marketplace.to_string();
    entry
}

// ---------------------------------------------------------------------------
// The pinned, dark-on-light column-heading row
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The navigator column: title bands, entry counts, and the Installed row
// ---------------------------------------------------------------------------

#[test]
fn the_marketplace_navigator_filters_the_result_rows() {
    // The lower navigator applies a marketplace filter on selection, composing with
    // the query and the category.
    let mut app = open(
        vec![
            attributed("community-one", "acme"),
            attributed("community-two", "acme"),
            attributed("official-one", "official"),
        ],
        &[],
        0,
    );
    render_effects(&mut app, 140, 30);
    let _marketplace = app.connector_store_marketplace_area;

    // Row 1 of the marketplace navigator is the first derived marketplace (`acme`).

    let browser = app.connector_store.as_mut().expect("panel open");
    assert!(browser.marketplace_select_row(1));
    assert_eq!(browser.active_marketplace(), "acme");
    assert_eq!(browser.filtered.len(), 2, "only acme rows survive");

    // Clearing the marketplace filter restores the full set, leaving the category
    // untouched.
    let browser = app.connector_store.as_mut().expect("panel open");
    browser.marketplace_clear();
    assert_eq!(browser.active_marketplace(), "ALL");
    assert_eq!(browser.filtered.len(), 3);
}

// ---------------------------------------------------------------------------
// The bottom search box and its clear button
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The row template: accent columns, the zebra band, and full-edge description
// ---------------------------------------------------------------------------
