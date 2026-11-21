//! Tests for the connector-catalogue manifest-description enrichment wiring: the
//! off-loop worker that fills each entry's blank description from its own
//! connector manifest, and the poll that applies the streamed updates to the open
//! panel (spec `connectors` FR-044).
//!
//! The manifest source is replaced with an offline implementation, so no live
//! catalogue or manifest is contacted (NFR-003). The catalogue fetch has no
//! offline seam, so the tests fill the store cache as the fetch job does and run
//! the job's enrichment step (`enrich_catalogue_into` into the cached index);
//! `poll_store_cache` applies the streamed updates to the open panel.

use codemason_app_core::app::App;
use codemason_app_core::app::StoreCacheKey;
use codemason_connectors::catalogue_enrich::{
    ManifestMeta, ManifestSource, ManifestUpdate, enrich_catalogue_into,
};
use codemason_connectors::{
    CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};

#[path = "support/mod.rs"]
mod support;

/// An offline manifest source answering from a fixed table keyed by entry id.
struct FakeSource {
    meta_by_id: Vec<(String, ManifestMeta)>,
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &ConnectorDescriptor) -> Option<ManifestMeta> {
        self.meta_by_id
            .iter()
            .find(|(id, _)| id == entry.id.as_str())
            .map(|(_, meta)| meta.clone())
    }
}

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

/// A catalogue descriptor with a github git install source and a blank
/// description.
fn entry(id: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: "developer".to_string(),
        tags: Vec::new(),
        source: format!("git+https://github.com/acme/connectors#v1:packages/{id}"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// An app with the connector panel open and no fetch started (the catalogue
/// slot is marked in flight first, so the open starts no network fetch).
fn open_loading() -> App {
    let mut app = support::make_app();
    app.store_cache
        .lock()
        .expect("store cache")
        .begin(StoreCacheKey::Connectors);
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    app
}

/// Deliver a catalogue into the store cache and enrich it from `source`, as the
/// off-loop fetch job would; returns the slot generation.
fn deliver(app: &App, entries: Vec<ConnectorDescriptor>, source: &dyn ManifestSource) -> u64 {
    let generation = support::deliver_connectors(app, entries.clone(), 0, Vec::new());
    let cache = std::sync::Arc::clone(&app.store_cache);
    let sink = |update: ManifestUpdate| {
        cache
            .lock()
            .expect("store cache")
            .push_connector_update(generation, update);
    };
    enrich_catalogue_into(&entries, source, &sink);
    cache
        .lock()
        .expect("store cache")
        .finish_enrichment(StoreCacheKey::Connectors, generation);
    generation
}

/// Poll streamed enrichment updates until the finished enrichment clears the
/// loading hint.
async fn drain(app: &mut App) {
    for _ in 0..600 {
        app.poll_store_cache();
        if app
            .connector_store
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

#[tokio::test]
async fn enrichment_fills_the_description_the_catalogue_omitted() {
    let mut app = open_loading();
    let source = FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                description: Some("Manage Linear issues".to_string()),
                ..Default::default()
            },
        )],
    };
    deliver(&app, vec![entry("linear")], &source);

    drain(&mut app).await;

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "Manage Linear issues",
        "the blank description is filled from the manifest"
    );
    assert!(!browser.enriching, "the loading hint clears on finish");
}

#[tokio::test]
async fn enrichment_keeps_a_description_the_catalogue_already_declared() {
    let mut app = open_loading();
    let source = FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                description: Some("From the manifest".to_string()),
                ..Default::default()
            },
        )],
    };
    let mut with_description = entry("linear");
    with_description.description = "Declared by the catalogue".to_string();
    deliver(&app, vec![with_description], &source);

    drain(&mut app).await;

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "Declared by the catalogue",
        "the catalogue's declared description is not overwritten"
    );
}

#[test]
fn an_update_from_a_stale_generation_is_dropped() {
    // A re-fetch replaces the cached catalogue with a new generation, so an update
    // tagged with the earlier generation must not be applied to the current
    // entries.
    let mut app = open_loading();
    let stale = support::deliver_connectors(&app, vec![entry("linear")], 0, Vec::new());
    app.poll_store_cache();
    // A re-fetch lands before the first fetch's enrichment reports.
    support::deliver_connectors(&app, vec![entry("linear")], 0, Vec::new());
    {
        let mut cache = app.store_cache.lock().expect("store cache");
        cache.push_connector_update(
            stale,
            ManifestUpdate {
                id: "linear".to_string(),
                meta: ManifestMeta {
                    description: Some("Manage Linear issues".to_string()),
                    ..Default::default()
                },
            },
        );
        cache.finish_enrichment(StoreCacheKey::Connectors, stale);
    }

    for _ in 0..3 {
        app.poll_store_cache();
    }

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "",
        "a stale-generation update is not applied to the current entries"
    );
}
