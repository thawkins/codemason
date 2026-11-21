//! Tests for the background fetch and enrichment jobs that fill the in-memory
//! store cache, and for the startup prefetch.
//!
//! The production HTTPS fetcher and manifest source are replaced with offline
//! fixtures, so no live store or manifest is contacted (NFR-003). The connector
//! catalogue is disabled through `CODEMASON_CONFIG_CONTENT` wherever the
//! prefetch would otherwise reach for it.

// Rust 2024 makes `std::env::set_var`/`remove_var` unsafe; this is the one
// sanctioned test use (AGENTS-LANGUAGE.md, Core Code Rules).
#![allow(unsafe_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;

use codemason_app_core::app::{App, PluginSlot, SlotData, StoreCacheKey};
use codemason_config::{MarketplaceEntry, PluginStoresConfig};
use codemason_plugins::{
    FixtureStoreFetcher, ManifestMeta, ManifestSource, StoreEntry, StoreError, StoreKind,
};

#[path = "support/mod.rs"]
mod support;

/// The added marketplace URL used by the partial-failure test; never served.
const COMMUNITY_URL: &str = "https://raw.githubusercontent.com/anthropics/claude-plugins-community/main/.claude-plugin/marketplace.json";

/// An offline manifest source that answers every entry with a version.
struct EveryEntrySource;

impl ManifestSource for EveryEntrySource {
    fn manifest_meta(&self, _entry: &StoreEntry, _kind: StoreKind) -> Option<ManifestMeta> {
        Some(ManifestMeta {
            version: Some("9.9.9".to_string()),
            description: None,
        })
    }
}

/// A store-index document whose entries all have manifest-derivable sources.
fn git_index(ids: &[&str]) -> Vec<u8> {
    let plugins: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "name": id,
                "version": "1.0.0",
                "source": format!("git+https://github.com/openai/plugins#main:plugins/{id}"),
            })
        })
        .collect();
    serde_json::to_vec(&json!({ "plugins": plugins })).expect("index json")
}

/// An app with the given fetcher and the always-answering manifest source.
fn app_with(fetcher: FixtureStoreFetcher) -> App {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(Arc::new(fetcher));
    app.set_manifest_source(Arc::new(EveryEntrySource));
    app
}

/// A snapshot of the plugin slot for `kind`.
fn slot(app: &App, kind: StoreKind) -> PluginSlot {
    app.store_cache
        .lock()
        .expect("store cache")
        .plugin(kind)
        .clone()
}

/// Poll the slot for `kind` until `done` holds, failing after 5 s.
async fn wait_for(app: &App, kind: StoreKind, done: impl Fn(&PluginSlot) -> bool) -> PluginSlot {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = slot(app, kind);
        if done(&current) {
            return current;
        }
        assert!(
            Instant::now() < deadline,
            "slot never reached the expected state: {current:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the config env for the whole test
async fn prefetch_fills_codex_slot() {
    let _lock = crate::global_env_lock();
    let previous = std::env::var_os("CODEMASON_CONFIG_CONTENT");
    // SAFETY: the binary-wide env lock is held.
    unsafe {
        std::env::set_var(
            "CODEMASON_CONFIG_CONTENT",
            r#"{"connectors":{"enabled":false}}"#,
        )
    };
    let mut app = app_with(FixtureStoreFetcher::new().with_default_endpoints());

    app.prefetch_store_indexes();
    let codex = wait_for(&app, StoreKind::Codex, |s| {
        matches!(s.data, SlotData::Ready(_))
    })
    .await;
    let connectors = app
        .store_cache
        .lock()
        .expect("store cache")
        .connectors()
        .clone();

    // SAFETY: the binary-wide env lock is held.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("CODEMASON_CONFIG_CONTENT", value),
            None => std::env::remove_var("CODEMASON_CONFIG_CONTENT"),
        }
    }
    let SlotData::Ready(index) = codex.data else {
        unreachable!("waited for Ready");
    };
    let ids: Vec<&str> = index.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["codex-weather", "codex-time"]);
    assert!(
        matches!(connectors.data, SlotData::Empty) && connectors.in_flight.is_none(),
        "a disabled connector catalogue is skipped without recording a failure: {connectors:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn enrichment_updates_land_in_slot() {
    let fetcher = FixtureStoreFetcher::new()
        .with_index(StoreKind::Codex.default_url(), git_index(&["a", "b", "c"]));
    let mut app = app_with(fetcher);

    app.spawn_plugin_store_fetch_with_stores(StoreKind::Codex, &PluginStoresConfig::default());
    let codex = wait_for(
        &app,
        StoreKind::Codex,
        |s| matches!(&s.data, SlotData::Ready(ix) if ix.enriched),
    )
    .await;

    let SlotData::Ready(index) = codex.data else {
        unreachable!("waited for Ready");
    };
    assert_eq!(index.updates.len(), 3);
    assert!(index.enriched);
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_error_marks_failed() {
    let mut app = app_with(FixtureStoreFetcher::new());

    app.spawn_plugin_store_fetch_with_stores(StoreKind::Codex, &PluginStoresConfig::default());
    let codex = wait_for(&app, StoreKind::Codex, |s| {
        matches!(s.data, SlotData::Failed { .. })
    })
    .await;

    let expected = StoreError::Network {
        detail: format!(
            "no offline fixture registered for {}",
            StoreKind::Codex.default_url()
        ),
    }
    .panel_detail(StoreKind::Codex);
    let SlotData::Failed { cause, .. } = codex.data else {
        unreachable!("waited for Failed");
    };
    assert!(cause.contains(&expected), "{cause:?} carries {expected:?}");
    assert!(codex.in_flight.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn partial_failure_records_marketplace() {
    let fetcher = FixtureStoreFetcher::new()
        .with_index(StoreKind::Claude.default_url(), git_index(&["official"]));
    let mut app = app_with(fetcher);
    let stores = PluginStoresConfig {
        marketplaces: vec![MarketplaceEntry {
            store: "claude".to_string(),
            name: "claude-plugins-community".to_string(),
            url: Some(COMMUNITY_URL.to_string()),
            source: "anthropics/claude-plugins-community".to_string(),
        }],
        ..PluginStoresConfig::default()
    };

    app.spawn_plugin_store_fetch_with_stores(StoreKind::Claude, &stores);
    let claude = wait_for(&app, StoreKind::Claude, |s| {
        matches!(s.data, SlotData::Ready(_))
    })
    .await;

    let SlotData::Ready(index) = claude.data else {
        unreachable!("waited for Ready");
    };
    assert_eq!(index.failed_marketplaces, ["claude-plugins-community"]);
    assert_eq!(index.entries.len(), 1);
}

#[test]
fn no_runtime_does_not_leave_in_flight() {
    let mut app = app_with(FixtureStoreFetcher::new().with_default_endpoints());

    app.start_store_fetch(StoreCacheKey::Plugins(StoreKind::Codex));

    assert_eq!(slot(&app, StoreKind::Codex).in_flight, None);
}
