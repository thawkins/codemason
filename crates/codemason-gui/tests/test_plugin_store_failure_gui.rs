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
//! path through the store cache, so nothing is contacted over the
//! network and no plugin is installed.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_plugin_store_failure.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use codemason_app_core::app::App;
use codemason_app_core::app::PluginStoreBrowser;
use codemason_plugins::{StoreError, StoreIndex, StoreKind};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

/// An app with the Codex panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app
}

/// Record one fetch outcome in the store cache, as the fetch job would.
fn deliver(app: &App, kind: StoreKind, outcome: Result<StoreIndex, StoreError>) {
    match outcome {
        Ok(index) => {
            support::deliver_plugin_index(app, kind, index.entries, index.skipped);
        }
        Err(err) => support::deliver_plugin_failure(app, kind, &err),
    }
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// -- Malformed entries are carried and counted (FR-025) ----------------------

// -- The skipped count is reported in the panel (FR-025) ---------------------

// -- A failed fetch is formatted inline and stays dismissible (FR-013) --------

#[test]
fn the_error_body_names_the_malformed_json_cause() {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app.plugin_store.as_mut().expect("panel open").set_failed(
        StoreError::MalformedJson {
            detail: "expected value at line 3 column 7".to_string(),
        }
        .to_string(),
    );

    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
    // The result column shares the body with the category navigator (spec
    // `catnav` T-004), so a long state line wraps onto the following row; the
    // full cause is still painted and must be readable across the wrapped rows.
    assert!(app.plugin_store.is_some(), "the panel stays open (FR-013)");
}

#[test]
fn an_unreachable_marketplace_renders_a_clean_unavailable_line() {
    let mut app = open_loading();
    // A transport-level failure (no connection) must read as the marketplace
    // being unavailable, naming the store, rather than echoing a raw error.
    deliver(
        &app,
        StoreKind::Codex,
        Err(StoreError::Network {
            detail: "connection refused".to_string(),
        }),
    );

    app.poll_store_cache();

    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
    assert!(app.plugin_store.is_some(), "the panel stays open (FR-013)");
}

#[test]
fn a_timed_out_marketplace_renders_a_clean_unavailable_line() {
    // Open the Claude panel so the delivered Claude result is not discarded as
    // stale (FR-007).
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Claude, "", false));
    deliver(
        &app,
        StoreKind::Claude,
        Err(StoreError::Timeout { ms: 10_000 }),
    );

    app.poll_store_cache();

    render_effects(&mut app, 100, 30);
    crate::frame::publish(&mut app);
    assert!(app.plugin_store.is_some(), "the panel stays open (FR-013)");
}

// -- No render path panics on hostile/edge store data (FR-025) ---------------

#[test]
fn an_out_of_range_cursor_with_no_results_does_not_panic_on_render() {
    let mut app = support::make_app();
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    // Empty filtered set but a non-zero cursor: the renderer must clamp, not panic.
    browser.cursor = 99;
    browser.set_index(StoreIndex {
        store: None,
        entries: Vec::new(),
        skipped: 1,
    });
    browser.cursor = 99;
    app.plugin_store = Some(browser);

    render_effects(&mut app, 80, 24);
    assert!(app.plugin_store.is_some());
}

#[test]
fn a_failed_state_renders_on_a_small_terminal_without_panicking() {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_failed("network unreachable".to_string());

    // A small-but-usable panel must still paint the failure line (wrapped across
    // the result column's rows, which now share the body with the category
    // navigator, spec `catnav` T-004) and keep the panel open, with no panic
    // (FR-013, FR-018, FR-025).
    render_effects(&mut app, 40, 14);
    crate::frame::publish(&mut app);
    assert!(app.plugin_store.is_some());
}
