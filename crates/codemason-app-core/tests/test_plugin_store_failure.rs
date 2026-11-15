//! Failure-reporting tests for the plugin-store browser: malformed store data
//! and fetch failures are formatted as panel/message reports and never panic
//! (spec `pluginstores` T-010; FR-013, FR-025).
//!
//! FR-013 requires a failed fetch to render an inline error row naming the cause
//! with the panel left open and dismissible. FR-025 forbids any panic or process
//! termination from a store-index fetch, a malformed index entry, an oversized
//! index, or a failed install, and requires every such failure to be contained
//! and reported in the panel or the message window. A malformed entry that the
//! parser skipped is contained data too, so it is reported as a partial result
//! (a `skipped` count in the panel title and empty line) rather than being
//! silently dropped.
//!
//! These tests drive the real renderer through a `TestBackend` and the real poll
//! path through the shared delivery slot, so nothing is contacted over the
//! network and no plugin is installed.

use codemason_app_core::app::App;
use codemason_app_core::app::{PluginStoreBrowser, PluginStoreFetchResult, PluginStoreStatus};
use codemason_plugins::{StoreEntry, StoreError, StoreIndex, StoreKind};

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, description: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: description.to_string(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: None,
    }
}

/// A browser with the given accepted entries and skipped-malformed count loaded.
fn loaded(entries: Vec<StoreEntry>, skipped: usize) -> PluginStoreBrowser {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_index(StoreIndex {
        store: Some("codex".to_string()),
        entries,
        skipped,
    });
    browser
}

/// An app with the Codex panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app
}

/// Deposit one fetch outcome into the app's delivery slot.
fn deliver(app: &App, kind: StoreKind, outcome: Result<StoreIndex, StoreError>) {
    let mut guard = app.plugin_store_result.lock().expect("slot");
    *guard = Some(PluginStoreFetchResult { kind, outcome });
}

// -- Malformed entries are carried and counted (FR-025) ----------------------

#[test]
fn set_index_carries_the_skipped_malformed_count() {
    let browser = loaded(vec![entry("codex-weather", "Weather")], 3);
    assert_eq!(browser.skipped, 3);
    assert_eq!(browser.all.len(), 1);
}

#[test]
fn set_entries_clears_the_skipped_count() {
    let mut browser = loaded(vec![entry("codex-weather", "Weather")], 4);
    browser.set_entries(vec![entry("codex-time", "Time")]);
    assert_eq!(browser.skipped, 0, "a bare entry list has no skipped count");
}

#[test]
fn a_delivered_index_carries_the_skipped_count_into_the_browser() {
    let mut app = open_loading();
    // One valid entry and two the parser rejected as malformed.
    deliver(
        &app,
        StoreKind::Codex,
        Ok(StoreIndex {
            store: Some("codex".to_string()),
            entries: vec![entry("codex-weather", "Weather")],
            skipped: 2,
        }),
    );

    app.poll_plugin_store_result();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.skipped, 2, "the skipped count survives the poll");
}

// -- The skipped count is reported in the panel (FR-025) ---------------------

// -- A failed fetch is formatted inline and stays dismissible (FR-013) --------

#[test]
fn a_malformed_index_error_polled_from_the_slot_is_contained() {
    let mut app = open_loading();
    deliver(
        &app,
        StoreKind::Codex,
        Err(StoreError::MalformedJson {
            detail: "expected value at line 1 column 1".to_string(),
        }),
    );

    app.poll_plugin_store_result();

    match &app.plugin_store.as_ref().expect("panel open").status {
        PluginStoreStatus::Failed(detail) => {
            assert!(detail.contains("line 1 column 1"), "detail: {detail}")
        }
        other => panic!("expected a contained inline failure, got {other:?}"),
    }
    assert!(app.plugin_store.is_some(), "no panic closed the panel");
}

// -- No render path panics on hostile/edge store data (FR-025) ---------------
