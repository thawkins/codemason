//! Tests for the plugin-store and connector-catalogue browsers reading the
//! in-memory store cache: the open paths, `poll_store_cache`, refresh and the
//! store-kind switch.
//!
//! The cache is filled directly (no fetch job, no network): a slot is marked in
//! flight before the browser opens wherever the open path must not start a
//! fetch of its own (NFR-003).

use codemason_app_core::app::{App, LogLevel, PluginStoreStatus, StoreCacheKey};
use codemason_connectors::{
    CatalogueBrowseStatus, CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId,
    ConnectorServer,
};
use codemason_plugins::{
    FetchLimits, ManifestMeta, ManifestUpdate, StoreEndpoint, StoreEntry, StoreError, StoreIndex,
    StoreIndexFetcher, StoreKind,
};

#[path = "support/mod.rs"]
mod support;

/// A fetcher that stays busy for longer than any assertion, so a started fetch
/// is still in flight when the test checks for it.
struct StalledFetcher;

impl StoreIndexFetcher for StalledFetcher {
    fn fetch_index(
        &self,
        _kind: StoreKind,
        _endpoint: &StoreEndpoint,
        _limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        std::thread::sleep(std::time::Duration::from_secs(2));
        Err(StoreError::Network {
            detail: "stalled".to_string(),
        })
    }
}

/// A store entry with no version, so a manifest update can fill it.
fn entry(id: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: String::new(),
        source: format!("git+https://github.com/openai/plugins#main:plugins/{id}"),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        category: None,
        marketplace: String::new(),
    }
}

/// A connector descriptor with one stdio server.
fn connector(id: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("id"),
        name: format!("{id} connector"),
        description: String::new(),
        category: "data".to_string(),
        tags: Vec::new(),
        source: String::new(),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![ConnectorServer {
            id: "main".to_string(),
            transport: "stdio".to_string(),
            command: Some("echo".to_string()),
            args: Vec::new(),
            env: Default::default(),
            url: None,
            headers: Default::default(),
        }],
        unsupported: Vec::new(),
    }
}

/// A manifest update carrying a version for `id`.
fn version_update(id: &str, version: &str) -> ManifestUpdate {
    ManifestUpdate {
        id: id.to_string(),
        meta: ManifestMeta {
            version: Some(version.to_string()),
            description: None,
        },
    }
}

/// Mark `key` in flight and return the generation.
fn begin(app: &App, key: StoreCacheKey) -> u64 {
    app.store_cache.lock().expect("store cache").begin(key)
}

/// Fill the plugin slot for `kind` with `entries` and return the generation.
fn fill_plugins(
    app: &App,
    kind: StoreKind,
    entries: Vec<StoreEntry>,
    failed_marketplaces: Vec<String>,
) -> u64 {
    let generation = begin(app, StoreCacheKey::Plugins(kind));
    app.store_cache.lock().expect("store cache").fill_plugins(
        kind,
        generation,
        entries,
        0,
        failed_marketplaces,
        Vec::new(),
    );
    generation
}

/// The in-flight generation of the plugin slot for `kind`.
fn plugin_in_flight(app: &App, kind: StoreKind) -> Option<u64> {
    app.store_cache
        .lock()
        .expect("store cache")
        .plugin(kind)
        .in_flight
}

/// The version of entry `id` in the open plugin browser.
fn version_of(app: &App, id: &str) -> String {
    app.plugin_store
        .as_ref()
        .expect("panel open")
        .all
        .iter()
        .find(|e| e.id == id)
        .expect("entry")
        .version
        .clone()
}

#[test]
fn ready_slot_fills_on_open() {
    let mut app = support::make_app();
    fill_plugins(
        &app,
        StoreKind::Codex,
        vec![entry("a"), entry("b")],
        Vec::new(),
    );

    app.open_plugin_store(StoreKind::Codex, "", false);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.all.len(), 2);
    assert_eq!(plugin_in_flight(&app, StoreKind::Codex), None);
}

#[test]
fn pending_slot_shows_loading_then_fills() {
    let mut app = support::make_app();
    let generation = begin(&app, StoreCacheKey::Plugins(StoreKind::Codex));

    app.open_plugin_store(StoreKind::Codex, "", false);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel open").status,
        PluginStoreStatus::Loading
    );

    app.store_cache.lock().expect("store cache").fill_plugins(
        StoreKind::Codex,
        generation,
        vec![entry("a")],
        0,
        Vec::new(),
        Vec::new(),
    );
    app.poll_store_cache();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.all.len(), 1);
}

#[test]
fn pending_then_failed_shows_failed() {
    let mut app = support::make_app();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let generation = begin(&app, key);
    app.open_plugin_store(StoreKind::Codex, "", false);

    app.store_cache.lock().expect("store cache").fail(
        key,
        generation,
        "store unreachable".to_string(),
    );
    app.poll_store_cache();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert!(
        matches!(&browser.status, PluginStoreStatus::Failed(detail) if detail.contains("store unreachable")),
        "{:?}",
        browser.status
    );
}

#[test]
fn cached_updates_applied_on_open() {
    let mut app = support::make_app();
    let generation = fill_plugins(&app, StoreKind::Codex, vec![entry("a")], Vec::new());
    app.store_cache
        .lock()
        .expect("store cache")
        .push_plugin_update(StoreKind::Codex, generation, version_update("a", "2.0.0"));

    app.open_plugin_store(StoreKind::Codex, "", false);

    assert_eq!(version_of(&app, "a"), "2.0.0");
    assert!(app.plugin_store.as_ref().expect("panel open").enriching);
}

#[test]
fn streamed_update_applied_once() {
    let mut app = support::make_app();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let generation = fill_plugins(&app, StoreKind::Codex, vec![entry("a")], Vec::new());
    app.open_plugin_store(StoreKind::Codex, "", false);

    app.store_cache
        .lock()
        .expect("store cache")
        .push_plugin_update(StoreKind::Codex, generation, version_update("a", "2.0.0"));
    app.poll_store_cache();
    app.poll_store_cache();

    let view = app.plugin_store_view.expect("view recorded");
    assert_eq!(view.generation, generation);
    assert_eq!(
        view.applied_updates, 1,
        "the update is applied exactly once"
    );
    assert_eq!(version_of(&app, "a"), "2.0.0");

    app.store_cache
        .lock()
        .expect("store cache")
        .finish_enrichment(key, generation);
    app.poll_store_cache();
    assert!(!app.plugin_store.as_ref().expect("panel open").enriching);
}

#[test]
fn switch_kind_ignores_other_kind_updates() {
    let mut app = support::make_app();
    fill_plugins(&app, StoreKind::Claude, vec![entry("a")], Vec::new());
    let codex = fill_plugins(&app, StoreKind::Codex, vec![entry("a")], Vec::new());
    app.open_plugin_store(StoreKind::Claude, "", false);

    app.store_cache
        .lock()
        .expect("store cache")
        .push_plugin_update(StoreKind::Codex, codex, version_update("a", "9.9.9"));
    app.poll_store_cache();

    assert_eq!(version_of(&app, "a"), "", "the Claude browser is unchanged");

    app.switch_plugin_store_kind(StoreKind::Codex);
    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.kind, StoreKind::Codex);
    assert_eq!(version_of(&app, "a"), "9.9.9");
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_forces_fetch() {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(std::sync::Arc::new(StalledFetcher));
    fill_plugins(&app, StoreKind::Codex, vec![entry("a")], Vec::new());

    app.open_plugin_store(StoreKind::Codex, "", true);

    assert!(plugin_in_flight(&app, StoreKind::Codex).is_some());
    assert_eq!(
        app.plugin_store.as_ref().expect("panel open").status,
        PluginStoreStatus::Ready,
        "the cached index shows while the refresh runs"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_open_store_forces_fetch() {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(std::sync::Arc::new(StalledFetcher));
    fill_plugins(&app, StoreKind::Codex, vec![entry("a")], Vec::new());
    app.open_plugin_store(StoreKind::Codex, "", false);
    assert_eq!(plugin_in_flight(&app, StoreKind::Codex), None);

    app.refresh_open_store();

    assert!(plugin_in_flight(&app, StoreKind::Codex).is_some());
}

#[test]
fn warnings_shown_once() {
    let mut app = support::make_app();
    let key = StoreCacheKey::Connectors;
    let generation = begin(&app, key);
    app.store_cache
        .lock()
        .expect("store cache")
        .fill_connectors(
            generation,
            vec![connector("linear")],
            0,
            vec!["a configured connector marketplace was skipped: http://x.example".to_string()],
        );
    let warnings = |app: &App| {
        app.log_entries
            .iter()
            .filter(|e| e.level == LogLevel::Warn && e.message.contains("marketplace was skipped"))
            .count()
    };

    app.open_connector_catalogue("", CategoryFilter::all(), false);
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(browser.status, CatalogueBrowseStatus::Ready);
    assert_eq!(warnings(&app), 1);

    app.close_connector_catalogue();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    assert_eq!(
        warnings(&app),
        1,
        "the warning is posted on the first open only"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_failure_refetches_on_open() {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(std::sync::Arc::new(StalledFetcher));
    fill_plugins(
        &app,
        StoreKind::Claude,
        vec![entry("official")],
        vec!["claude-plugins-community".to_string()],
    );

    app.open_plugin_store(StoreKind::Claude, "", false);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.all.len(), 1);
    assert!(plugin_in_flight(&app, StoreKind::Claude).is_some());
}
