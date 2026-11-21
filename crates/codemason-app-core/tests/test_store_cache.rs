//! Tests for the pure `StoreCache` state model.

use codemason_app_core::app::{SlotData, StoreCache, StoreCacheKey};
use codemason_plugins::{ManifestUpdate, StoreEntry, StoreKind};

fn entry(id: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        ..StoreEntry::default()
    }
}

fn update(id: &str) -> ManifestUpdate {
    ManifestUpdate {
        id: id.to_string(),
        meta: Default::default(),
    }
}

fn fill(cache: &mut StoreCache, kind: StoreKind, generation: u64) {
    cache.fill_plugins(kind, generation, vec![entry("a")], 0, vec![], vec![]);
}

#[test]
fn begin_then_fill_is_ready() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let generation = cache.begin(key);
    assert_eq!(cache.plugin(StoreKind::Codex).in_flight, Some(generation));
    fill(&mut cache, StoreKind::Codex, generation);
    let slot = cache.plugin(StoreKind::Codex);
    assert_eq!(slot.in_flight, None);
    match &slot.data {
        SlotData::Ready(ix) => {
            assert_eq!(ix.generation, generation);
            assert_eq!(ix.entries.len(), 1);
        }
        other => panic!("expected Ready, got {other:?}"),
    }
}

#[test]
fn stale_fill_ignored() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let old = cache.begin(key);
    let new = cache.begin(key);
    assert_ne!(old, new);
    fill(&mut cache, StoreKind::Codex, old);
    let slot = cache.plugin(StoreKind::Codex);
    assert!(matches!(slot.data, SlotData::Empty));
    assert_eq!(slot.in_flight, Some(new));
}

#[test]
fn fail_keeps_existing_ready() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Claude);
    let g1 = cache.begin(key);
    fill(&mut cache, StoreKind::Claude, g1);
    let g2 = cache.begin(key);
    cache.fail(key, g2, "boom".into());
    let slot = cache.plugin(StoreKind::Claude);
    assert!(matches!(slot.data, SlotData::Ready(_)));
    assert_eq!(slot.in_flight, None);
}

#[test]
fn fail_on_empty_is_failed() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Connectors;
    let generation = cache.begin(key);
    cache.fail(key, generation, "boom".into());
    let slot = cache.connectors();
    assert!(matches!(&slot.data, SlotData::Failed { cause, .. } if cause == "boom"));
    assert_eq!(slot.in_flight, None);
}

#[test]
fn needs_fetch_rules() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    assert!(cache.needs_fetch(key, false));
    let generation = cache.begin(key);
    assert!(!cache.needs_fetch(key, true));
    fill(&mut cache, StoreKind::Codex, generation);
    assert!(!cache.needs_fetch(key, false));
    assert!(cache.needs_fetch(key, true));

    let g2 = cache.begin(key);
    cache.fill_plugins(StoreKind::Codex, g2, vec![], 0, vec!["x".into()], vec![]);
    assert!(cache.needs_fetch(key, false));

    let ck = StoreCacheKey::Connectors;
    let g3 = cache.begin(ck);
    cache.fail(ck, g3, "e".into());
    assert!(cache.needs_fetch(ck, false));
}

#[test]
fn updates_append_in_order() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let generation = cache.begin(key);
    fill(&mut cache, StoreKind::Codex, generation);
    cache.push_plugin_update(StoreKind::Codex, generation, update("one"));
    cache.push_plugin_update(StoreKind::Codex, generation, update("two"));
    cache.push_plugin_update(StoreKind::Codex, generation + 99, update("stale"));
    cache.finish_enrichment(key, generation);
    let SlotData::Ready(ix) = &cache.plugin(StoreKind::Codex).data else {
        panic!("expected Ready");
    };
    let ids: Vec<&str> = ix.updates.iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, ["one", "two"]);
    assert!(ix.enriched);
}

#[test]
fn invalidate_empties_slot() {
    let mut cache = StoreCache::default();
    let key = StoreCacheKey::Plugins(StoreKind::Codex);
    let generation = cache.begin(key);
    fill(&mut cache, StoreKind::Codex, generation);
    cache.begin(key);
    cache.invalidate(key);
    let slot = cache.plugin(StoreKind::Codex);
    assert!(matches!(slot.data, SlotData::Empty));
    assert_eq!(slot.in_flight, None);
}

#[test]
fn kinds_are_independent() {
    let mut cache = StoreCache::default();
    let generation = cache.begin(StoreCacheKey::Plugins(StoreKind::Codex));
    fill(&mut cache, StoreKind::Codex, generation);
    assert!(matches!(
        cache.plugin(StoreKind::Claude).data,
        SlotData::Empty
    ));
    assert!(matches!(cache.connectors().data, SlotData::Empty));
}
