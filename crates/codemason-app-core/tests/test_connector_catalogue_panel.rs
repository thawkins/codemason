//! Render tests for the connector-catalogue browse panel (`/connectors claude`).
//!
//! The panel is the interactive peer of `/plugins claude` for the connector
//! catalogue. These drive the real renderer and the real key routing through a
//! `TestBackend` and read the painted buffer back, so no catalogue endpoint is
//! contacted: the browser state is filled directly.

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;
use codemason_connectors::{
    CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};

#[path = "support/mod.rs"]
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
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    let browser = app.connector_store.as_mut().expect("panel open");
    browser.set_installed(installed.iter().map(|s| (*s).to_string()).collect());
    browser.set_entries(entries, skipped);
    app
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

#[tokio::test]
async fn escape_on_an_empty_query_dismisses_the_panel() {
    let mut app = open(sample(), &[], 0);
    crate::term_events::handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(
        app.connector_store.is_none(),
        "Esc on an empty query closes the panel"
    );
}

#[tokio::test]
async fn enter_on_an_empty_result_set_records_a_notice_and_installs_nothing() {
    let mut app = open(Vec::new(), &[], 0);
    crate::term_events::handle_key(&mut app, key(KeyCode::Enter)).await;
    let browser = app.connector_store.as_ref().expect("panel still open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("no result highlighted")
    );
}

#[tokio::test]
async fn enter_on_an_installed_row_is_refused() {
    // The catalogue sorts by id (FR-046), so row 0 is `github`; mark it installed so
    // Enter on the highlighted row is refused.
    let mut app = open(sample(), &["github"], 0);
    crate::term_events::handle_key(&mut app, key(KeyCode::Enter)).await;
    let browser = app.connector_store.as_ref().expect("panel still open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("connector github is already installed")
    );
}

// ---------------------------------------------------------------------------
// The empty-query dismiss leaves the area cleared on the next frame
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Launch category filter (`/connectors claude --category <name>`)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-044 - the marketplace column and the enrichment loading hint
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The pinned, dark-on-light column-heading row
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The navigator column: title bands, entry counts, and the Installed row
// ---------------------------------------------------------------------------

#[test]
fn the_installed_pseudo_category_filters_to_the_store_scan() {
    // Selecting the `Installed` row shows only the connectors present in the store
    // scan, whatever category they declare - including one that declares none.
    let mut uncategorised = descriptor("local-thing", "");
    uncategorised.marketplace = String::new();
    let mut app = open(
        vec![
            descriptor("github", "developer"),
            descriptor("slack", "communication"),
            uncategorised,
        ],
        &["local-thing"],
        0,
    );
    let browser = app.connector_store.as_mut().expect("panel open");
    // Row 1 is the `Installed` pseudo-category.
    assert_eq!(browser.category_row_label(1), "Installed");
    assert!(browser.category_select_row(1));
    assert_eq!(
        browser.filtered.len(),
        1,
        "only the installed connector survives the filter"
    );
    assert_eq!(browser.all[browser.filtered[0]].id.as_str(), "local-thing");
}

#[test]
fn a_declared_category_named_installed_never_forms_a_second_row() {
    // The `Installed` name is reserved: a catalogue that declares a category
    // spelled `installed` collapses onto the one pinned pseudo-category row.
    let app = open(vec![descriptor("odd", "installed")], &[], 0);
    let browser = app.connector_store.as_ref().expect("panel open");
    // `ALL` and the pinned `Installed` row, and nothing more.
    assert_eq!(browser.category_row_count(), 2);
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(browser.category_row_label(1), "Installed");
}

// ---------------------------------------------------------------------------
// The bottom search box and its clear button
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The row template: accent columns, the zebra band, and full-edge description
// ---------------------------------------------------------------------------
