# Marketplace Browser GUI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fetch the three marketplace indexes once at startup into an in-memory cache, and replace the narrow TUI-style store dialog with two wide three-pane Material dialogs: the Plugin Store and Connectors.

**Architecture:** app-core gains a `StoreCache` (one slot per index) that background jobs fill: fetch, then stream enrichment updates. The browsers read from the cache. A single `poll_store_cache` replaces the four per-panel fetch/enrichment polls. The GUI dialog becomes a `stores/` module: a shared frame (header, filter rail, list, details, footer) plus dialog-specific row and details renderers, built from `codemason_gui::material`.

**Tech Stack:** Rust 2024, egui 0.37 (forked), egui_kittest, tokio, cargo nextest.

**Spec:** `docs/superpowers/specs/2026-11-22-marketplace-browser-gui-design.md` (read it first; section numbers below refer to it).

## Global Constraints

- Run `cargo fmt` after every `.rs` edit; `cargo fmt --check` and `cargo clippy -p <crate>` are clean before each commit.
- Tests live in `crates/<crate>/tests/test_*.rs`, with one `#[path = "test_x.rs"] mod test_x;` line in that crate's `tests/all.rs`. Tests use no network and never write into the repo.
- No `.unwrap()`/`.expect()` on production paths; no new `unsafe`; no inline `#[cfg(test)]` modules.
- Docblocks on every new public fn and module.
- GUI: Material first. Missing components go into `codemason_gui::material`, each with a test.
- Keyboard behaviour (`input::handle_key`) is unchanged.
- Keep each fetch's existing threading: plugin fetch and enrichment use `tokio::runtime::Handle::try_current()` + `spawn_blocking` (skipped when there is no runtime); the connector fetch uses `std::thread::spawn`; connector enrichment uses `spawn_blocking`.
- Dialog size: 85% of `ctx.content_rect()`, minimum 960 x 600, clamped to the window when it is smaller. Filter rail 180 pt; details pane 300 pt.
- Accessible labels `CATEGORIES_LABEL`, `MARKETPLACES_LABEL`, `RESULTS_LABEL`, `INSTALL`, `CLEAR_SEARCH`, `CLOSE` keep their values.
- Prefetch runs only from `App::start_background_services` (called from `run_startup`, which only the GUI launch uses), never from headless or one-shot paths.

## Review Focus

1. **Prefetch still pending when a browser opens, then it fails:** the panel must move from Loading to the Failed state with Retry, never stay on Loading. Owned by Task 3 (`test_store_cache_open::pending_then_failed_shows_failed`).
2. **Enrichment updates for one store arriving while the other store is open** (Codex/Claude switch): updates go into the right slot only and never reach the open browser of the other kind. Task 3 (`switch_kind_ignores_other_kind_updates`).
3. **A window smaller than 960 x 600:** the dialog clamps to the window and the list and details scroll; nothing is cut off. Task 6 (`test_store_dialog_gui::small_window_clamps`).
4. **Long names, long descriptions, more than 3 tags:** the row ellipsises the description, shows at most 3 tag chips, and never scrolls horizontally. Task 6 (`long_row_does_not_overflow`).
5. **Zero results after filtering:** the list shows "No matches", and Install (header and details) is disabled. Task 6 (`no_matches_disables_install`).

---

### Task 1: `StoreCache` state model (app-core)

**Files:**
- Create: `crates/codemason-app-core/src/app/store_cache.rs`
- Modify: `crates/codemason-app-core/src/app/mod.rs` (declare `pub mod store_cache;`, re-export the types)
- Modify: `crates/codemason-plugins` and `crates/codemason-connectors` `ManifestUpdate`: add `#[derive(Clone)]` if it is missing
- Test: `crates/codemason-app-core/tests/test_store_cache.rs`

**Interfaces (produces):**
```rust
pub enum StoreCacheKey { Plugins(StoreKind), Connectors }            // Copy, Eq, Hash, Debug
pub struct CachedIndex<E, U> {
    pub generation: u64, pub entries: Vec<E>, pub skipped: usize,
    pub updates: Vec<U>, pub enriched: bool,
    pub failed_marketplaces: Vec<String>, pub warnings: Vec<String>,
    pub warnings_shown: bool,
}
pub enum SlotData<E, U> { Empty, Ready(CachedIndex<E, U>), Failed { generation: u64, cause: String } }
pub struct CacheSlot<E, U> { pub data: SlotData<E, U>, pub in_flight: Option<u64> }
pub type PluginSlot = CacheSlot<StoreEntry, codemason_plugins::ManifestUpdate>;
pub type ConnectorSlot = CacheSlot<ConnectorDescriptor, codemason_connectors::catalogue_enrich::ManifestUpdate>;
pub struct StoreCache { /* codex, claude: PluginSlot; connectors: ConnectorSlot; next_generation: u64 */ }
impl StoreCache {
    pub fn plugin(&self, kind: StoreKind) -> &PluginSlot;
    pub fn connectors(&self) -> &ConnectorSlot;
    pub fn begin(&mut self, key: StoreCacheKey) -> u64;                       // new generation, sets in_flight, keeps data
    pub fn cancel(&mut self, key: StoreCacheKey, generation: u64);            // clears in_flight when it matches (spawn failed)
    pub fn needs_fetch(&self, key: StoreCacheKey, force: bool) -> bool;
    pub fn fill_plugins(&mut self, kind: StoreKind, generation: u64, entries: Vec<StoreEntry>, skipped: usize, failed_marketplaces: Vec<String>, warnings: Vec<String>);
    pub fn fill_connectors(&mut self, generation: u64, entries: Vec<ConnectorDescriptor>, skipped: usize, warnings: Vec<String>);
    pub fn push_plugin_update(&mut self, kind: StoreKind, generation: u64, update: codemason_plugins::ManifestUpdate);
    pub fn push_connector_update(&mut self, generation: u64, update: codemason_connectors::catalogue_enrich::ManifestUpdate);
    pub fn finish_enrichment(&mut self, key: StoreCacheKey, generation: u64);
    pub fn fail(&mut self, key: StoreCacheKey, generation: u64, cause: String);
    pub fn invalidate(&mut self, key: StoreCacheKey);                         // data = Empty, in_flight = None
}
pub type SharedStoreCache = Arc<std::sync::Mutex<StoreCache>>;
```
Rules: every `fill_*`, `push_*`, `finish_*` and `fail` call whose `generation` is not the slot's current `in_flight` (or, for push/finish, not `Ready.generation`) is ignored as stale. `fill_*` replaces `data` with `Ready` and clears `in_flight`. `fail` keeps an existing `Ready` (a failed refresh does not throw away good data) and only sets `Failed` when the data was `Empty` or `Failed`. `needs_fetch` returns false while `in_flight` is set; otherwise it returns true for `force`, `Empty`, `Failed`, or `Ready` with non-empty `failed_marketplaces`.

- [ ] **Step 1: Write failing tests** in `test_store_cache.rs`: `begin_then_fill_is_ready`, `stale_fill_ignored` (fill with an old generation leaves the slot unchanged), `fail_keeps_existing_ready`, `fail_on_empty_is_failed`, `needs_fetch_rules` (Empty → true; in-flight → false even with force; Ready → false; Ready + force → true; Ready with `failed_marketplaces = ["x"]` → true; Failed → true), `updates_append_in_order`, `invalidate_empties_slot`, `kinds_are_independent` (filling Codex leaves Claude Empty).
- [ ] **Step 2:** `cargo nextest run -p codemason-app-core -E 'test(test_store_cache::)'`: expect a compile failure (module missing).
- [ ] **Step 3:** Implement `store_cache.rs` per the interfaces above.
- [ ] **Step 4:** Run the same command: PASS.
- [ ] **Step 5:** Commit `feat(app-core): in-memory store index cache model`.

### Task 2: Background fetch + enrichment jobs into the cache, and the startup prefetch

**Files:**
- Modify: `crates/codemason-app-core/src/app/state.rs` (App field `pub store_cache: SharedStoreCache`, initialised to the default)
- Modify: `crates/codemason-app-core/src/app/session_ops.rs` (fetch jobs)
- Modify: `crates/codemason-app-core/src/app/startup.rs` (`start_background_services` calls `prefetch_store_indexes`)
- Test: `crates/codemason-app-core/tests/test_store_cache_fetch.rs`

**Interfaces:**
- Consumes: Task 1 types.
- Produces:
  - `fn fetch_merged_plugin_index(fetcher: &dyn StoreIndexFetcher, kind: StoreKind, endpoints: &[(StoreEndpoint, String)], limits: &FetchLimits) -> (Result<StoreIndex, StoreError>, Vec<String>)`: the existing merge loop moved out of `spawn_plugin_store_fetch_with_stores`, with the same dedup and the same "error only if nothing answered" rule. The `Vec<String>` holds the names of marketplaces that failed.
  - `pub fn start_store_fetch(&mut self, key: StoreCacheKey)`: no-op when `!needs_fetch(key, true)` because a fetch is in flight. Calls `begin`, spawns the job, and calls `cancel` if it cannot spawn (no runtime for plugins). Plugin job: fetch → `fill_plugins` (warnings = refused-endpoint messages; error → `fail(key, gen, err.panel_detail(kind))`) → `enrich_into(..)` with a sink that calls `push_plugin_update` → `finish_enrichment`. Connector job: when `!config.is_enabled()`, `fail(.., "connectors.enabled = false")`; otherwise `search_installable_catalogue_full` → `fill_connectors` (warnings = the existing `refused_marketplace` message text, the same string `poll_connector_catalogue_result` posts today) → `enrich_catalogue_into` → `push_connector_update` → `finish_enrichment`. The job locks the cache only for each short call, never across a fetch.
  - `spawn_plugin_store_fetch_with_stores(kind, stores)` keeps its signature (tests use it) but routes into the cache job.
  - `pub fn prefetch_store_indexes(&mut self)`: starts Codex, Claude and Connectors (skipping Connectors when it is disabled; no failure is recorded or shown).

- [ ] **Step 1: Write failing tests** in a `#[tokio::test(flavor = "multi_thread")]` setup with `set_plugin_store_fetcher(FixtureStoreFetcher)` and `set_manifest_source(<in-memory>)`: `prefetch_fills_codex_slot` (await the slot becoming `Ready` with the fixture entries, polling with a 5 s deadline); `enrichment_updates_land_in_slot` (`updates.len()` equals the number of fixture manifests, then `enriched == true`); `fetch_error_marks_failed` (fixture returns an error → `Failed { cause }` containing the `panel_detail` text); `partial_failure_records_marketplace` (two endpoints, one failing → `Ready` and `failed_marketplaces == [<name>]`); `no_runtime_does_not_leave_in_flight` (plain `#[test]`: `start_store_fetch` → `in_flight == None`).
- [ ] **Step 2:** `cargo nextest run -p codemason-app-core -E 'test(test_store_cache_fetch::)'`: FAIL.
- [ ] **Step 3:** Implement the interfaces above; wire `prefetch_store_indexes()` at the end of `start_background_services`.
- [ ] **Step 4:** Run the same command: PASS.
- [ ] **Step 5:** Commit `feat(app-core): prefetch marketplace indexes at startup`.

### Task 3: Browsers read from the cache; one poll replaces four

**Files:**
- Modify: `crates/codemason-app-core/src/app/session_ops.rs` (`open_plugin_store_category`, `open_connector_catalogue`, new poll and actions)
- Modify: `crates/codemason-app-core/src/app/housekeeping.rs` (call `poll_store_cache()` in place of `poll_plugin_store_result`, `poll_store_enrichment`, `poll_connector_catalogue_result` and `poll_connector_enrichment`)
- Modify: `crates/codemason-app-core/src/app/state.rs` (fields `plugin_store_view` and `connector_store_view`; remove the result/enrichment slots and generation counters this task orphans)
- Test: `crates/codemason-app-core/tests/test_store_cache_open.rs`; update the existing tests that drove the removed slots (`grep -rln "plugin_store_result\|store_enrich_result\|connector_catalogue_result\|connector_enrich_result" crates/*/tests`) so they fill `app.store_cache` and call `poll_store_cache()` instead. Do not weaken their assertions.

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces:
  - `pub struct CacheView { pub generation: u64, pub applied_updates: usize }` (Default)
  - App fields: `pub plugin_store_view: Option<CacheView>`, `pub connector_store_view: Option<CacheView>`; both reset to `None` on every open/close.
  - `pub fn poll_store_cache(&mut self)`: for the open plugin browser (key = its kind) and the open connector browser:
    - `Ready` with a generation different from the view's: `set_index` / `set_entries`, `apply_pending_category`, set `view = {generation, 0}`. The first time the slot's `warnings` are shown, post them where they are posted today and set `warnings_shown`. Mark `enriching = !enriched`.
    - Then apply `updates[view.applied_updates..]` with the browser's `apply_manifest_update`, advance the view, and call `finish_enrichment()` once the slot is `enriched`.
    - `Failed` with no `in_flight`: `plugin_store_failed` / `connector_catalogue_failed` with the cause.
    - Otherwise leave the browser as it is (Loading).
  - Open paths: build the browser as today, then `if cache.needs_fetch(key, refresh) { start_store_fetch(key) }`, then `poll_store_cache()` immediately, so a `Ready` slot fills on the open frame.
  - `pub fn refresh_open_store(&mut self)`: force a fetch for whichever browser is open (`needs_fetch(key, true)`).
  - `pub fn switch_plugin_store_kind(&mut self, kind: StoreKind)`: `open_plugin_store(kind, "", false)`.

- [ ] **Step 1: Write failing tests** (fill `app.store_cache` directly; no network): `ready_slot_fills_on_open` (status `Ready`, `all.len()` equals the cached count, no fetch started: `in_flight == None`); `pending_slot_shows_loading_then_fills` (set `in_flight`, open → `Loading`, fill + poll → `Ready`); `pending_then_failed_shows_failed`; `cached_updates_applied_on_open` (the cached `ManifestUpdate` sets the version on the open browser's entry); `streamed_update_applied_once` (push one update, poll twice → applied exactly once); `switch_kind_ignores_other_kind_updates` (Claude open, push a Codex update, poll → the Claude browser is unchanged); `refresh_forces_fetch` (`open_plugin_store(.., refresh = true)` on a Ready slot sets `in_flight`); `warnings_shown_once` (connector warning posted on the first open only); `partial_failure_refetches_on_open` (Ready + `failed_marketplaces` → open shows the entries and sets `in_flight`).
- [ ] **Step 2:** `cargo nextest run -p codemason-app-core -E 'test(test_store_cache_open::)'`: FAIL.
- [ ] **Step 3:** Implement; remove the orphaned slots, generations and their polls; update the existing tests that used them.
- [ ] **Step 4:** `cargo nextest run -p codemason-app-core`: all PASS. Save the output to `target/temp/app-core-nextest.txt` and read failures from that file rather than re-running.
- [ ] **Step 5:** Commit `feat(app-core): store browsers read the startup index cache`.

### Task 4: Marketplace add/remove clears the cache

**Files:**
- Modify: `crates/codemason-app-core/src/app/plugin.rs` (after a `marketplace` subcommand runs)
- Modify: `crates/codemason-app-core/src/app/connector.rs` (the same for `/connectors marketplace`)
- Modify: `crates/codemason-app-core/src/app/session_ops.rs` (`marketplace_changed`)
- Test: `crates/codemason-app-core/tests/test_store_cache_marketplace.rs`

**Interfaces:**
- `pub fn marketplace_mutation_target(rest: &str) -> Option<StoreKind>` in `plugin.rs`: `Some(kind)` for `<codex|claude> add|remove ...`, `None` for `show`, `list` and anything else.
- `pub fn connector_marketplace_mutates(rest: &str) -> bool` in `connector.rs`: true for `add|remove ...`.
- `pub fn marketplace_changed(&mut self, key: StoreCacheKey)`: `invalidate(key)`, then `start_store_fetch(key)`.
- After the command's own handler returns (whatever the result), call `marketplace_changed` when the helper matches.

- [ ] **Step 1: Failing tests:** `mutation_target_parses` (`"codex add https://x"` → Codex; `"claude remove foo"` → Claude; `"show"`, `"codex list"`, `""` → None); `connector_mutation_parses`; `marketplace_changed_invalidates_ready_slot` (no runtime: after the call the slot data is `Empty`).
- [ ] **Step 2:** `cargo nextest run -p codemason-app-core -E 'test(test_store_cache_marketplace::)'`: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command: PASS.
- [ ] **Step 5:** Commit `feat(app-core): refetch a store index after marketplace add/remove`.

### Task 5: Material additions: badge and drawer item with count

**Files:**
- Create: `crates/codemason-gui/src/material/badge.rs`
- Modify: `crates/codemason-gui/src/material/sheet.rs` (add `drawer_item_with_count`)
- Modify: `crates/codemason-gui/src/material/mod.rs`
- Test: `crates/codemason-gui/tests/test_material_badge.rs`, plus a case in the existing sheet/drawer material test file (or a new `test_material_drawer_count.rs`)

**Interfaces (produces):**
- `pub enum BadgeTone { Success, Primary, Secondary, Tertiary, Error, Neutral }`
- `pub fn badge(ui: &mut egui::Ui, palette: &Palette, tone: BadgeTone, label: &str) -> Response`: an 18 pt high, 8 pt radius pill with `label_small` text, using `<tone>_container` / `on_<tone>_container` roles (Success uses the palette's existing success/positive role if `m3` has one; otherwise `tertiary_container`). Accessible as `Role::Label` with `label`.
- `pub fn drawer_item_with_count(ui: &mut egui::Ui, palette: &Palette, label: &str, count: usize, selected: bool) -> Response`: `drawer_item` with the count right-aligned in `on_surface_variant`, and the accessible label `"{label} ({count})"` (matching today's row text, so existing GUI tests that look for it keep working).

- [ ] **Step 1: Failing tests** (`support_material` harness): `badge_has_label_and_contrast` (each tone's text and fill pass `contrast_ratio >= AA_CONTRAST` for every bundled theme); `drawer_count_label` (an accessible node named `"Testing (19)"` exists and reports selected when it is selected).
- [ ] **Step 2:** `cargo nextest run -p codemason-gui -E 'test(test_material_badge::) + test(test_material_drawer_count::)'`: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Same command: PASS.
- [ ] **Step 5:** Commit `feat(gui): material badge and counted drawer item`.

### Task 6: Shared three-pane frame and the Plugin Store dialog

**Files:**
- Delete: `crates/codemason-gui/src/dialogs/stores.rs`
- Create: `crates/codemason-gui/src/dialogs/stores/mod.rs` (public label constants, `show_plugin_store`, `show_connector_store` entry points, the `Click` enum, click dispatch)
- Create: `crates/codemason-gui/src/dialogs/stores/frame.rs` (sizing, header, filter rail, list scaffold, details scaffold, footer, status states)
- Create: `crates/codemason-gui/src/dialogs/stores/snapshot.rs` (structured snapshot)
- Create: `crates/codemason-gui/src/dialogs/stores/plugin.rs` (plugin rows and details)
- Modify: `crates/codemason-app-core` browsers: make `category_entry_count` / `marketplace_entry_count` (or their equivalents) `pub` if they are not already, on both `PluginStoreBrowser` and `CatalogueBrowser`
- Test: `crates/codemason-gui/tests/test_store_dialog_gui.rs`; update `test_plugin_store_*_gui.rs` for the new title and row text

**Interfaces:**
- Consumes: Task 3 (`refresh_open_store`, `switch_plugin_store_kind`), Task 5 (`badge`, `drawer_item_with_count`).
- Produces (`snapshot.rs`):
  ```rust
  pub struct FilterRow { pub label: String, pub count: usize }
  pub struct RowSnapshot { pub name: String, pub subtitle: String, pub description: String,
                           pub tags: Vec<String>, pub installed: bool, pub detail: Detail }
  pub enum Detail { Plugin(PluginDetail), Connector(ConnectorDetail) }
  pub struct PluginDetail { pub version: String, pub marketplace: String, pub source: String, pub homepage: Option<String> }
  pub struct ConnectorDetail { pub category: String, pub marketplace: String, pub auth: ConnectorAuthShape,
                               pub scopes: Vec<String>, pub credential: Option<String>, pub servers: Vec<ServerSnapshot>, pub source: String }
  pub struct ServerSnapshot { pub id: String, pub transport: String, pub target: String } // url, or command + args joined by spaces
  pub enum ListState { Loading, Empty, Failed(String), Ready }
  pub struct StoreSnapshot { pub title: &'static str, pub plugin_kind: Option<StoreKind>, pub query: String,
                             pub shown: usize, pub total: usize, pub state: ListState, pub focus: Pane,
                             pub categories: Vec<FilterRow>, pub category_cursor: usize,
                             pub marketplaces: Vec<FilterRow>, pub marketplace_cursor: usize,
                             pub rows: Vec<RowSnapshot>, pub cursor: usize, pub footer: Option<String> }
  ```
  `StoreSnapshot::plugin(&PluginStoreBrowser)` / `::connector(&CatalogueBrowser)`. Plugin subtitle: `"{version} - {marketplace}"` (version omitted when it is blank). Connector subtitle: the category.
- `Click` gains `RowInstall(usize)`, `Refresh`, `Retry`, `SwitchKind(StoreKind)`. `RowInstall(i)` sets the cursor to `i`, focuses Results and sends Enter. `Refresh` and `Retry` call `refresh_open_store`.
- Copy: titles `"Plugin Store"` and `"Connectors"` (the `CONNECTORS_TITLE` value changes; `plugin_store_title` is removed); switcher options `["Codex", "Claude"]`; search placeholder `"Search plugins..."` / `"Search connectors..."`; footer `"{shown} of {total}"` plus `" - search \"{query}\""` when the query is non-empty; status text `"Loading the store index..."`, `"Loading the catalogue..."`, `"The store has no entries"`, `"The catalogue has no entries"`, `"No matches"`, Failed `"{cause}"` with a `"Retry"` button; footer action `"Refresh"`; tooltips: search `"Type to search - Esc clears, then closes"`, panes `"Tab switches pane - Up/Down move - Enter installs"`.
- Layout as in spec 5.1-5.2: `material_modal`, with `width` from the size rule; the header row (title, segmented switcher for plugins, `outlined_value` showing the query, a close `icon_button`); the body has a fixed height of `dialog_height - header - footer`; each column has its own `ScrollArea`; the focused pane gets a 2 pt `primary` outline. Rows are 64 pt with a letter avatar (`secondary_container` circle), a one-line ellipsised description, at most 3 `chip_dense` tags, an Installed `badge(Success)`, and a trailing `button`: Filled on the cursor row, Tonal on the others, Outlined + disabled "Installed" when installed. Details: name (`title_large`), subtitle, description (wrapped), the detail fields, and a Filled Install. The homepage is a text `button` that calls `ctx.open_url`.

- [ ] **Step 1: Failing tests** (`test_store_dialog_gui.rs`, driving `frame::publish` with a filled `PluginStoreBrowser`): `plugin_dialog_is_wide` (the dialog node is at least 960 pt wide in a 1400x900 window, and its width ≈ 0.85 × the window width); `small_window_clamps` (800x500 window → the dialog fits inside it); `row_shows_version_and_marketplace`; `details_show_homepage_and_source`; `row_install_installs_that_row` (click the trailing Install on row 2 → cursor == 2 and an Enter is routed); `switcher_changes_kind` (click "Claude" → `app.plugin_store.kind == Claude`); `refresh_button_starts_fetch`; `long_row_does_not_overflow` (a 300-char description and 6 tags → row width ≤ list width, 3 chips drawn); `no_matches_disables_install`; `failed_state_has_retry`.
- [ ] **Step 2:** `cargo nextest run -p codemason-gui -E 'test(test_store_dialog_gui::)'`: FAIL.
- [ ] **Step 3:** Implement frame, snapshot and plugin renderer; `show_connector_store` temporarily keeps drawing through the new frame with a minimal connector row (Task 7 completes it).
- [ ] **Step 4:** `cargo nextest run -p codemason-gui -E 'test(store) + test(plugin_store) + test(connector_catalogue)'`: all PASS (existing tests updated only for the title and row text).
- [ ] **Step 5:** Commit `feat(gui): wide three-pane Plugin Store dialog`.

### Task 7: Connectors dialog

**Files:**
- Create: `crates/codemason-gui/src/dialogs/stores/connector.rs`
- Modify: `crates/codemason-gui/src/dialogs/stores/mod.rs`, `snapshot.rs`
- Test: `crates/codemason-gui/tests/test_connector_dialog_gui.rs`; update `test_connector_catalogue_*_gui.rs` for the new title and row text

**Interfaces:**
- Consumes: Task 6 frame and snapshot; Task 5 `badge`.
- Sign-in badge label → tone: `None` → "No sign-in" / Neutral; `Env` → "Env" / Primary; `Token` → "Token" / Secondary; `Oauth` → "OAuth" / Tertiary; `Password` → "Password" / Error.
- Sign-in sentence (exact copy): None "No sign-in needed"; Env "Reads its credential from an environment variable"; Token "Needs an API token, kept in the credential store"; Oauth "You'll sign in in your browser after installing"; Password "Needs a username and password".
- Row: avatar, name, category subtitle, description, sign-in badge, `"1 server"` / `"{n} servers"` chip, Installed badge. No switcher, no version.
- Details sections, in order: Sign-in (badge + sentence), "Requested scopes" (chips; the section is hidden when empty), "Credential" (the name; hidden when `None`; never a value), "MCP servers" (one outlined card per server: id, transport chip, target), "Tags", "Source", Install.

- [ ] **Step 1: Failing tests:** `connector_dialog_has_no_switcher`; `row_shows_auth_badge_and_server_count` (an OAuth connector with 2 servers → nodes "OAuth" and "2 servers"); `details_show_scopes_credential_servers`; `scopes_hidden_when_empty`; `credential_value_never_shown` (the descriptor's credential name is shown and no env/header value appears in the rendered text); `each_auth_shape_has_sentence` (all 5 shapes).
- [ ] **Step 2:** `cargo nextest run -p codemason-gui -E 'test(test_connector_dialog_gui::)'`: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo nextest run -p codemason-gui`: all PASS (save output to `target/temp/gui-nextest.txt`).
- [ ] **Step 5:** Commit `feat(gui): Connectors dialog with sign-in, scopes and servers`.

### Task 8: Docs and whole-change verification

**Files:**
- Modify: `QUICKSTART.md` / `SPEC.md` where they describe the store browsers or `--refresh` (only the relevant lines)
- Modify: `CHANGELOG.md` (an Unreleased entry: the startup cache and the redesigned dialogs)

- [ ] **Step 1:** Update the docs.
- [ ] **Step 2:** `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings`: clean.
- [ ] **Step 3:** `timeout 1500 cargo nextest run --workspace --no-fail-fast > target/temp/nextest-full.txt 2>&1`; read the summary from the file and fix any failure (including ones not caused by this change, in a separate commit).
- [ ] **Step 4:** `cargo test --doc -p codemason-app-core -p codemason-gui`: PASS.
- [ ] **Step 5:** Manual: `cargo run`, wait about 10 s, then open `/plugins codex`, `/plugins claude` and `/connectors claude`. Each opens wide and populated with no Loading state; the Codex↔Claude switch is instant; Refresh shows Loading only while it re-fetches.
- [ ] **Step 6:** Commit `docs: marketplace browser redesign and startup index cache`.
