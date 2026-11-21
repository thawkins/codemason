//! Tests for the plugin-store manifest-metadata enrichment: the off-loop worker
//! that fills each entry's version/description from the plugin's own manifest, and
//! the poll that applies the streamed updates to the open panel (spec `pluginstores`
//! FR-048).
//!
//! The store fetcher and manifest source are replaced with offline
//! implementations, so no live store or manifest is contacted (NFR-003). The
//! enrichment is driven through the real wiring: the store-cache fetch job loads
//! the index, enriches its entries into the cache, and `poll_store_cache` applies
//! the streamed updates to the open panel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use codemason_app_core::app::App;
use codemason_app_core::app::{PluginStoreBrowser, StoreCacheKey};
use codemason_config::PluginStoresConfig;
use codemason_plugins::{
    FetchLimits, ManifestMeta, ManifestSource, ManifestUpdate, StoreEndpoint, StoreEntry,
    StoreError, StoreIndex, StoreIndexFetcher, StoreKind,
};

#[path = "support/mod.rs"]
mod support;

/// An offline manifest source answering from a fixed table once `open` is set,
/// so a test can observe the panel while enrichment is still pending.
struct FakeSource {
    meta_by_id: Vec<(String, ManifestMeta)>,
    open: Arc<AtomicBool>,
}

impl FakeSource {
    /// A source that answers at once.
    fn new(meta_by_id: Vec<(String, ManifestMeta)>) -> Self {
        Self {
            meta_by_id,
            open: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &StoreEntry, _kind: StoreKind) -> Option<ManifestMeta> {
        while !self.open.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.meta_by_id
            .iter()
            .find(|(id, _)| id == &entry.id)
            .map(|(_, meta)| meta.clone())
    }
}

fn entry(id: &str, source: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: String::new(),
        source: source.to_string(),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        category: None,
        marketplace: String::new(),
    }
}

/// An app with the Codex panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app
}

/// An offline store fetcher serving a fixed entry list for every endpoint.
struct EntriesFetcher(Vec<StoreEntry>);

impl StoreIndexFetcher for EntriesFetcher {
    fn fetch_index(
        &self,
        kind: StoreKind,
        _endpoint: &StoreEndpoint,
        _limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        Ok(StoreIndex {
            store: Some(kind.token().to_string()),
            entries: self.0.clone(),
            skipped: 0,
        })
    }
}

/// Start the store-cache fetch of `entries` for the open panel, as opening the
/// store would; its job then enriches the entries from the manifest source.
fn deliver(app: &mut App, kind: StoreKind, entries: Vec<StoreEntry>) {
    app.set_plugin_store_fetcher(Arc::new(EntriesFetcher(entries)));
    app.spawn_plugin_store_fetch_with_stores(kind, &PluginStoresConfig::default());
}

/// Poll until the fetched index has landed in the open panel.
async fn wait_loaded(app: &mut App) {
    for _ in 0..600 {
        app.poll_store_cache();
        if app.plugin_store_view.is_some() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the off-loop fetch did not land");
}

/// Poll streamed enrichment updates until the finished enrichment clears the
/// loading hint.
async fn drain(app: &mut App) {
    for _ in 0..600 {
        app.poll_store_cache();
        if app.plugin_store_view.is_some()
            && app
                .plugin_store
                .as_ref()
                .map(|b| !b.enriching)
                .unwrap_or(false)
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the off-loop enrichment did not finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn enrichment_fills_the_version_and_description_the_index_omitted() {
    let mut app = open_loading();
    let open = Arc::new(AtomicBool::new(false));
    app.set_manifest_source(Arc::new(FakeSource {
        open: Arc::clone(&open),
        meta_by_id: vec![
            (
                "linear".to_string(),
                ManifestMeta {
                    version: Some("1.2.3".to_string()),
                    description: Some("Manage Linear issues".to_string()),
                },
            ),
            (
                "slack".to_string(),
                ManifestMeta {
                    version: None,
                    description: Some("Work with Slack".to_string()),
                },
            ),
        ],
    }));
    deliver(
        &mut app,
        StoreKind::Codex,
        vec![
            entry(
                "linear",
                "git+https://github.com/openai/plugins#main:plugins/linear",
            ),
            entry(
                "slack",
                "git+https://github.com/openai/plugins#main:plugins/slack",
            ),
        ],
    );

    wait_loaded(&mut app).await;
    // The panel is now enriching; a title hint would be visible.
    assert!(
        app.plugin_store.as_ref().expect("panel open").enriching,
        "the index landing starts enrichment"
    );
    open.store(true, Ordering::SeqCst);
    drain(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id == "linear")
        .expect("linear");
    assert_eq!(
        linear.version, "1.2.3",
        "the manifest version filled the row"
    );
    assert_eq!(linear.description, "Manage Linear issues");
    let slack = browser.all.iter().find(|e| e.id == "slack").expect("slack");
    assert_eq!(slack.version, "", "the manifest omitted a version");
    assert_eq!(slack.description, "Work with Slack");
    assert!(!browser.enriching, "the loading hint clears on finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn enrichment_keeps_a_version_the_index_already_declared() {
    let mut app = open_loading();
    app.set_manifest_source(Arc::new(FakeSource::new(vec![(
        "clangd".to_string(),
        ManifestMeta {
            version: Some("1.0.0".to_string()),
            description: Some("LSP".to_string()),
        },
    )])));
    let mut with_version = entry(
        "clangd",
        "git+https://github.com/anthropics/x#main:plugins/clangd",
    );
    with_version.version = "9.9.9".to_string();
    deliver(&mut app, StoreKind::Codex, vec![with_version]);

    drain(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let clangd = browser
        .all
        .iter()
        .find(|e| e.id == "clangd")
        .expect("clangd");
    assert_eq!(
        clangd.version, "9.9.9",
        "the index's declared version is not overwritten by the manifest"
    );
    assert_eq!(
        clangd.description, "LSP",
        "a blank description is still filled"
    );
}

#[test]
fn an_update_from_a_stale_generation_is_dropped() {
    // A re-fetch replaces the cached index with a new generation, so an update
    // tagged with the earlier generation must not be applied to the current
    // entries.
    let mut app = open_loading();
    let linear = || {
        entry(
            "linear",
            "git+https://github.com/openai/plugins#main:plugins/linear",
        )
    };
    let stale = support::deliver_plugin_index(&app, StoreKind::Codex, vec![linear()], 0);
    app.poll_store_cache();
    // A re-fetch lands before the first fetch's enrichment reports.
    support::deliver_plugin_index(&app, StoreKind::Codex, vec![linear()], 0);
    app.store_cache
        .lock()
        .expect("store cache")
        .push_plugin_update(
            StoreKind::Codex,
            stale,
            ManifestUpdate {
                id: "linear".to_string(),
                meta: ManifestMeta {
                    version: Some("1.2.3".to_string()),
                    description: Some("Manage Linear issues".to_string()),
                },
            },
        );
    app.store_cache
        .lock()
        .expect("store cache")
        .finish_enrichment(StoreCacheKey::Plugins(StoreKind::Codex), stale);

    for _ in 0..3 {
        app.poll_store_cache();
    }

    let browser = app.plugin_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id == "linear")
        .expect("linear");
    assert_eq!(
        linear.version, "",
        "a stale-generation update is not applied to the current entries"
    );
    assert!(
        browser.enriching,
        "a stale generation's finish does not clear the current hint"
    );
}
