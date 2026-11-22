//! Tests for marketplace add/remove clearing the in-memory store index cache.

use codemason_app_core::app::{
    SlotData, StoreCacheKey, connector_marketplace_mutates, marketplace_mutation_target,
};
use codemason_plugins::{StoreEntry, StoreKind};

#[path = "support/mod.rs"]
mod support;

#[test]
fn mutation_target_parses() {
    assert_eq!(
        marketplace_mutation_target("codex add https://x"),
        Some(StoreKind::Codex)
    );
    assert_eq!(
        marketplace_mutation_target("claude remove foo"),
        Some(StoreKind::Claude)
    );
    assert_eq!(marketplace_mutation_target("show"), None);
    assert_eq!(marketplace_mutation_target("codex list"), None);
    assert_eq!(marketplace_mutation_target(""), None);
}

#[test]
fn connector_mutation_parses() {
    assert!(connector_marketplace_mutates("claude add https://x"));
    assert!(connector_marketplace_mutates("add https://x"));
    assert!(connector_marketplace_mutates("claude remove foo"));
    assert!(!connector_marketplace_mutates("show"));
    assert!(!connector_marketplace_mutates("claude list"));
    assert!(!connector_marketplace_mutates(""));
}

#[test]
fn marketplace_changed_invalidates_ready_slot() {
    let mut app = support::make_app();
    let kind = StoreKind::Codex;
    let key = StoreCacheKey::Plugins(kind);
    {
        let mut cache = app.store_cache.lock().expect("store cache");
        let generation = cache.begin(key);
        cache.fill_plugins(
            kind,
            generation,
            vec![StoreEntry {
                id: "a".to_string(),
                ..StoreEntry::default()
            }],
            0,
            Vec::new(),
            Vec::new(),
        );
        assert!(matches!(cache.plugin(kind).data, SlotData::Ready(_)));
    }
    app.marketplace_changed(key);
    let cache = app.store_cache.lock().expect("store cache");
    assert!(matches!(cache.plugin(kind).data, SlotData::Empty));
}
