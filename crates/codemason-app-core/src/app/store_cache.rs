//! In-memory cache of marketplace indexes (plugin stores and the connector catalogue).
//!
//! This is a pure state model: it owns no threads and does no I/O. A fetch job
//! calls [`StoreCache::begin`] to obtain a generation, then reports back through
//! the `fill_*`/`push_*`/`finish_*`/`fail` methods; any report carrying a
//! generation that is no longer current is ignored as stale.
//!
//! Dependencies: `codemason_plugins` (store entries, manifest updates) and
//! `codemason_connectors` (catalogue descriptors).

use std::sync::{Arc, Mutex};

use codemason_connectors::ConnectorDescriptor;
use codemason_connectors::catalogue_enrich::ManifestUpdate as ConnectorManifestUpdate;
use codemason_plugins::{ManifestUpdate as PluginManifestUpdate, StoreEntry, StoreKind};

/// Identifies one cached index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreCacheKey {
    /// A plugin store.
    Plugins(StoreKind),
    /// The connector catalogue.
    Connectors,
}

/// A fetched index plus its enrichment progress.
#[derive(Debug, Clone)]
pub struct CachedIndex<E, U> {
    /// Generation of the fetch that produced this index.
    pub generation: u64,
    /// Listing entries.
    pub entries: Vec<E>,
    /// Count of entries skipped as malformed.
    pub skipped: usize,
    /// Manifest updates received so far, in arrival order.
    pub updates: Vec<U>,
    /// True once enrichment finished.
    pub enriched: bool,
    /// Marketplaces that could not be fetched.
    pub failed_marketplaces: Vec<String>,
    /// Non-fatal warnings from the fetch.
    pub warnings: Vec<String>,
}

/// A fetch that failed while the slot kept showing an earlier index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshError {
    /// Generation of the failed fetch.
    pub generation: u64,
    /// Human-readable cause.
    pub cause: String,
    /// True once the failure was surfaced to the user.
    pub shown: bool,
}

/// How far an open browser has consumed a cache slot: the generation of the
/// index it shows and how many of that index's manifest updates it applied.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheView {
    /// Generation of the index (or failure) the browser shows.
    pub generation: u64,
    /// Count of the index's manifest updates already applied.
    pub applied_updates: usize,
}

/// What a slot currently holds.
#[derive(Debug, Clone)]
pub enum SlotData<E, U> {
    /// Nothing fetched yet.
    Empty,
    /// A fetched index.
    Ready(CachedIndex<E, U>),
    /// The last fetch failed and there is no earlier data.
    Failed {
        /// Generation of the failed fetch.
        generation: u64,
        /// Human-readable cause.
        cause: String,
    },
}

/// One cache slot: its data and the generation of any fetch in flight.
#[derive(Debug, Clone)]
pub struct CacheSlot<E, U> {
    /// Current contents.
    pub data: SlotData<E, U>,
    /// Generation of the in-flight fetch, if any.
    pub in_flight: Option<u64>,
    /// The warnings last surfaced to the user for this slot, so a re-fetch
    /// that reports the same warnings does not post them again.
    pub posted_warnings: Vec<String>,
    /// The last fetch that failed over a `Ready` index; cleared when a later
    /// fetch fills the slot.
    pub refresh_error: Option<RefreshError>,
}

impl<E, U> Default for CacheSlot<E, U> {
    fn default() -> Self {
        Self {
            data: SlotData::Empty,
            in_flight: None,
            posted_warnings: Vec::new(),
            refresh_error: None,
        }
    }
}

/// Slot for a plugin store.
pub type PluginSlot = CacheSlot<StoreEntry, PluginManifestUpdate>;
/// Slot for the connector catalogue.
pub type ConnectorSlot = CacheSlot<ConnectorDescriptor, ConnectorManifestUpdate>;

/// The whole cache: one slot per marketplace.
#[derive(Debug, Default)]
pub struct StoreCache {
    codex: PluginSlot,
    claude: PluginSlot,
    connectors: ConnectorSlot,
    next_generation: u64,
}

/// A cache shared between the UI thread and fetch jobs.
pub type SharedStoreCache = Arc<Mutex<StoreCache>>;

impl<E, U> CacheSlot<E, U> {
    fn begin(&mut self, generation: u64) {
        self.in_flight = Some(generation);
    }

    fn cancel(&mut self, generation: u64) {
        if self.in_flight == Some(generation) {
            self.in_flight = None;
        }
    }

    fn needs_fetch(&self, force: bool) -> bool {
        if self.in_flight.is_some() {
            return false;
        }
        match &self.data {
            SlotData::Empty | SlotData::Failed { .. } => true,
            SlotData::Ready(ix) => force || !ix.failed_marketplaces.is_empty(),
        }
    }

    /// Replaces the data with `ready` when `generation` is the in-flight one.
    fn fill(&mut self, generation: u64, ready: CachedIndex<E, U>) {
        if self.in_flight == Some(generation) {
            self.data = SlotData::Ready(ready);
            self.in_flight = None;
            self.refresh_error = None;
        }
    }

    fn push_update(&mut self, generation: u64, update: U) {
        if let SlotData::Ready(ix) = &mut self.data
            && ix.generation == generation
        {
            ix.updates.push(update);
        }
    }

    fn finish_enrichment(&mut self, generation: u64) {
        if let SlotData::Ready(ix) = &mut self.data
            && ix.generation == generation
        {
            ix.enriched = true;
        }
    }

    fn fail(&mut self, generation: u64, cause: String) {
        if self.in_flight != Some(generation) {
            return;
        }
        self.in_flight = None;
        if matches!(self.data, SlotData::Ready(_)) {
            self.refresh_error = Some(RefreshError {
                generation,
                cause,
                shown: false,
            });
        } else {
            self.data = SlotData::Failed { generation, cause };
        }
    }

    fn invalidate(&mut self) {
        self.data = SlotData::Empty;
        self.in_flight = None;
        self.refresh_error = None;
    }

    fn mark_warnings_shown(&mut self, warnings: &[String]) {
        self.posted_warnings = warnings.to_vec();
    }

    fn mark_refresh_error_shown(&mut self, generation: u64) {
        if let Some(error) = self.refresh_error.as_mut()
            && error.generation == generation
        {
            error.shown = true;
        }
    }
}

fn index<E, U>(
    generation: u64,
    entries: Vec<E>,
    skipped: usize,
    failed_marketplaces: Vec<String>,
    warnings: Vec<String>,
) -> CachedIndex<E, U> {
    CachedIndex {
        generation,
        entries,
        skipped,
        updates: Vec::new(),
        enriched: false,
        failed_marketplaces,
        warnings,
    }
}

impl StoreCache {
    fn plugin_mut(&mut self, kind: StoreKind) -> &mut PluginSlot {
        match kind {
            StoreKind::Codex => &mut self.codex,
            StoreKind::Claude => &mut self.claude,
        }
    }

    /// Returns the slot for a plugin store.
    pub fn plugin(&self, kind: StoreKind) -> &PluginSlot {
        match kind {
            StoreKind::Codex => &self.codex,
            StoreKind::Claude => &self.claude,
        }
    }

    /// Returns the connector catalogue slot.
    pub fn connectors(&self) -> &ConnectorSlot {
        &self.connectors
    }

    /// Starts a fetch for `key`: allocates a new generation and marks it in
    /// flight, keeping any existing data. Returns the generation.
    pub fn begin(&mut self, key: StoreCacheKey) -> u64 {
        self.next_generation += 1;
        let generation = self.next_generation;
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).begin(generation),
            StoreCacheKey::Connectors => self.connectors.begin(generation),
        }
        generation
    }

    /// Clears the in-flight marker when it matches `generation` (the fetch
    /// job could not be spawned).
    pub fn cancel(&mut self, key: StoreCacheKey, generation: u64) {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).cancel(generation),
            StoreCacheKey::Connectors => self.connectors.cancel(generation),
        }
    }

    /// Whether a fetch should be started for `key`. Never while one is in
    /// flight; otherwise when forced, empty, failed, or partially failed.
    pub fn needs_fetch(&self, key: StoreCacheKey, force: bool) -> bool {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin(kind).needs_fetch(force),
            StoreCacheKey::Connectors => self.connectors.needs_fetch(force),
        }
    }

    /// Stores a fetched plugin index; ignored when `generation` is stale.
    pub fn fill_plugins(
        &mut self,
        kind: StoreKind,
        generation: u64,
        entries: Vec<StoreEntry>,
        skipped: usize,
        failed_marketplaces: Vec<String>,
        warnings: Vec<String>,
    ) {
        let ready = index(generation, entries, skipped, failed_marketplaces, warnings);
        self.plugin_mut(kind).fill(generation, ready);
    }

    /// Stores a fetched connector catalogue; ignored when `generation` is stale.
    pub fn fill_connectors(
        &mut self,
        generation: u64,
        entries: Vec<ConnectorDescriptor>,
        skipped: usize,
        warnings: Vec<String>,
    ) {
        let ready = index(generation, entries, skipped, Vec::new(), warnings);
        self.connectors.fill(generation, ready);
    }

    /// Appends a plugin manifest update; ignored when stale.
    pub fn push_plugin_update(
        &mut self,
        kind: StoreKind,
        generation: u64,
        update: PluginManifestUpdate,
    ) {
        self.plugin_mut(kind).push_update(generation, update);
    }

    /// Appends a connector manifest update; ignored when stale.
    pub fn push_connector_update(&mut self, generation: u64, update: ConnectorManifestUpdate) {
        self.connectors.push_update(generation, update);
    }

    /// Marks enrichment complete for `key`; ignored when stale.
    pub fn finish_enrichment(&mut self, key: StoreCacheKey, generation: u64) {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).finish_enrichment(generation),
            StoreCacheKey::Connectors => self.connectors.finish_enrichment(generation),
        }
    }

    /// Records a failed fetch; ignored when stale. An existing `Ready` index is
    /// kept and the failure is recorded as the slot's
    /// [`CacheSlot::refresh_error`].
    pub fn fail(&mut self, key: StoreCacheKey, generation: u64, cause: String) {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).fail(generation, cause),
            StoreCacheKey::Connectors => self.connectors.fail(generation, cause),
        }
    }

    /// Records that `warnings` were surfaced to the user for `key`, so a later
    /// index reporting the same warnings does not post them again.
    pub fn mark_warnings_shown(&mut self, key: StoreCacheKey, warnings: &[String]) {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).mark_warnings_shown(warnings),
            StoreCacheKey::Connectors => self.connectors.mark_warnings_shown(warnings),
        }
    }

    /// Records that the refresh failure at `generation` for `key` was surfaced
    /// to the user; ignored when stale.
    pub fn mark_refresh_error_shown(&mut self, key: StoreCacheKey, generation: u64) {
        match key {
            StoreCacheKey::Plugins(kind) => {
                self.plugin_mut(kind).mark_refresh_error_shown(generation);
            }
            StoreCacheKey::Connectors => self.connectors.mark_refresh_error_shown(generation),
        }
    }

    /// Drops the data and any in-flight marker for `key`.
    pub fn invalidate(&mut self, key: StoreCacheKey) {
        match key {
            StoreCacheKey::Plugins(kind) => self.plugin_mut(kind).invalidate(),
            StoreCacheKey::Connectors => self.connectors.invalidate(),
        }
    }
}
