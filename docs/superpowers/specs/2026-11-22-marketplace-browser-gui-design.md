# Marketplace browser GUI redesign and startup index cache

Date: 2026-11-22. Status: design, awaiting approval.
Scope: `crates/codemason-gui` (the store dialogs) and `crates/codemason-app-core`
(the index cache). Mockups: `.superpowers/brainstorm/*/content/layout.html`
(options A and B) and `layout-v2.html` (the chosen split).

## 1. Intent

Three slash commands open marketplace browsers:

- `/plugins codex` and `/plugins claude`, the plugin store (`App::plugin_store`,
  `PluginStoreBrowser`);
- `/connectors claude`, the connector catalogue (`App::connector_store`,
  `CatalogueBrowser`).

The GUI draws both with one dialog, `crates/codemason-gui/src/dialogs/stores.rs`,
which copies the TUI's layout. The user has two complaints:

1. **It looks wrong in a GUI.** A default `egui::Modal` sizes to its content, so
   the dialog comes out narrow and tall, much worse than the TUI version. Each
   result is one flat string (`name version [installed] - description`), the
   focused pane is marked with a `> ` prefix, the search query can be typed but
   is not shown as a field, and a keyboard-hint line runs along the bottom. Much
   of the data (tags, homepage, source, marketplace, connector sign-in, scopes,
   MCP servers) is never shown. None of the `codemason_gui::material` components
   are used.
2. **It is slow.** Every open starts a fresh network fetch of the index (and,
   for plugins, a per-entry manifest enrichment pass). The user waits on
   "Loading..." every time, even though index content changes rarely.

Success means opening any of the three browsers shows a wide, readable, Material
dialog that is populated on the first frame (once the startup prefetch has
finished), with no change to how the keyboard works.

## 2. Goals, non-goals and anti-goals

**Goals**

- A landscape dialog sized to the app window, built from `material` components.
- Rich result rows plus a details pane for the selected entry.
- Plugins and connectors as **separate dialogs**: the two are similar concepts
  but not identical, and each dialog shows the fields that matter to it.
- Fetch all three indexes once in the background at startup and keep them in
  memory for the whole process. Opening a browser reads the cache.

**Non-goals**

- No dynamic or periodic re-fetching. Content changes infrequently. `--refresh`
  stays the only way to re-fetch.
- No changes to install logic, the marketplace config, or the TUI's drawing.
- No new keyboard bindings or changes to existing ones.

**Anti-goals** (failure even if it technically works)

- Making the plugin store and the connector catalogue look like one dialog with
  a toggle between them.
- Startup that blocks or slows down because of the prefetch, or a prefetch error
  that shows up anywhere before the user opens a browser.
- Breaking keyboard-only use of either browser.

## 3. Constraints

- app-core owns browser state and key handling (`input::handle_key`). The GUI
  draws a snapshot and calls existing App actions, or sends keys through
  `send_key`, as it does today.
- AGENTS.md "GUI Design: Material First": use `material` (list items, chips,
  text field, buttons, card/sheet frames, `Palette::m3`). Add any missing
  component to `material` with a test; do not draw one-off styling in the
  dialog.
- Fetches are blocking: connectors use `reqwest::blocking` on a plain OS thread,
  plugins use `spawn_blocking` when a tokio runtime is present. Keep each
  existing threading choice.
- Tests go in `crates/codemason-gui/tests/` and
  `crates/codemason-app-core/tests/`, registered in `tests/all.rs`, with no
  network access (use `set_plugin_store_fetcher` / fixture fetchers) and no
  writes into the repo.
- Existing GUI tests (`test_plugin_store_*_gui.rs`,
  `test_connector_catalogue_*_gui.rs`) use the accessible labels
  `CATEGORIES_LABEL`, `MARKETPLACES_LABEL`, `RESULTS_LABEL`, `INSTALL`,
  `CLEAR_SEARCH` and `CLOSE`. Keep these labels, or update the tests in the same
  change.

## 4. Approaches considered

| | Approach | Trade-off | Outcome |
|---|---|---|---|
| A | **Three panes**: filter rail on the left (categories and marketplaces, with counts), rich result list in the centre, details pane on the right | Dense and fast for search and the keyboard; maps one-to-one onto the existing three-pane focus model | **Chosen** |
| B | Card grid with category chips, a marketplace dropdown, and a details side sheet | Better for casual browsing; fewer items on screen; arrow-key movement in a grid does not match the current Up/Down model | Rejected |

For the cache, the alternatives were (i) fetching lazily on first open and then
keeping the result, or (ii) persisting to disk with a TTL. The user chose a
**background prefetch at startup into memory**, because content changes
infrequently and the delay on first open is the pain point.

## 5. Design

### 5.1 Shared dialog frame (both browsers)

- **Size:** about 85% of the app window's content rect, minimum 960 x 600 (or the
  whole window when it is smaller), centred, with an M3 dialog surface
  (`surface-container-high`, large corner radius). Not content-sized.
- **Header bar:** title, an outlined search field showing the live query (typing
  anywhere still edits it, as today; clicking the field is optional polish), and
  a close icon button.
- **Body:** three columns:
  - **Filter rail** (about 180 pt, `surface-container`): "Categories" and
    "Marketplaces" sections of drawer-style items, each with a trailing count;
    the selected item uses `secondary-container`. Uses the existing
    `category_row_display` / `marketplace_row_display` rows (the count may need a
    structured accessor, see 5.5).
  - **Result list** (flexible width): `material` list items, as specified per
    dialog below. The selected row is highlighted, with a leading accent bar.
    Each row has a trailing Install button: filled on the selected row, tonal on
    the others, and an outlined, disabled "Installed" on installed rows.
  - **Details pane** (about 300 pt): the full entry for the cursor row and a
    large Install button.
- **Pane focus** is shown by the focused pane's header colour or outline, not a
  `> ` prefix.
- **Footer bar:** `N of M` and the active search on the left; the last install
  result, skipped-entry count, and a "Refresh" text button on the right (it
  re-fetches, the same as `--refresh`).
- **Status states** take the place of the list body: Loading (progress indicator
  plus text), Empty, Failed (error text plus a Retry button). They use the
  `material` feedback components.
- **Keyboard:** unchanged. The bottom hint line is removed and the shortcuts
  move into tooltips (search field: "type to search, Esc to clear or close";
  panes: "Tab to switch pane").
- **Mouse:** the same as today (click a filter selects it and focuses that pane,
  click a row moves the cursor, double-click or Install installs), plus the
  per-row Install button, which moves the cursor to that row and installs it.

### 5.2 Plugin Store dialog (`/plugins codex`, `/plugins claude`)

- Title "Plugin Store". The header has a segmented button **Codex | Claude**.
  Switching kind re-opens the browser for the other kind
  (`open_plugin_store(kind, "", false)`): the search and filters reset, the data
  comes from the cache, and the switch is instant.
- **Row:** a letter avatar (first letter of the name), the **name**, the
  `version - marketplace` in muted text, a one-line description (ellipsised),
  tag chips (at most 3), and an Installed badge when installed.
- **Details:** name, version, marketplace, full description, tags, source,
  homepage (a link button that opens it in the system browser), and Install.

### 5.3 Connectors dialog (`/connectors claude`)

- Title "Connectors". No switcher.
- **Row:** a letter avatar, the **name**, the category in muted text, a one-line
  description, a colour-coded **sign-in badge** from `ConnectorAuthShape`
  (OAuth, Token, Env, Password, "No sign-in"), an "N server(s)" chip, and an
  Installed badge. No version.
- **Details:** name, category, marketplace, description, and a **Sign-in**
  section with the badge and one plain sentence per shape (for example, OAuth:
  "You'll sign in in your browser after installing"). Then **Requested scopes**
  (`auth_scope` chips, shown only when non-empty), **Credential** (the
  `credential` name, never a value), **MCP servers** (one small card per
  `ConnectorServer`: id, transport chip, and the URL or the command plus args),
  tags, source, and Install.
- The button verb stays **Install**, to match app-core's wording.

### 5.4 Startup index cache (app-core)

- A new in-memory cache on `App` with one slot per index (Codex plugins, Claude
  plugins, Claude connectors). Each slot holds one of `Pending`,
  `Ready(entries)` or `Failed(cause)`.
- **Prefetch:** when the App starts (both TUI and GUI, since it lives in
  app-core), start the three fetches in the background, using each one's
  existing threading choice. Connectors are skipped when
  `connectors.enabled = false`. Startup never waits on them, and failures are
  silent until a browser opens.
- **Plugin enrichment:** the cached plugin entries are the *enriched* entries.
  Either run enrichment as part of the prefetch, or write enrichment results
  back to the cache as well as to the open browser, so a second open does not
  enrich again.
- **Open:**
  - `Ready`: fill the browser on the spot (`set_index` / `set_entries`, then
    apply the pending category) with no Loading state.
  - `Pending`: show Loading and fill the browser when that same fetch finishes;
    never start a second fetch.
  - `Failed`, or no slot (prefetch skipped): fetch now, as today, and store the
    result in the cache.
- **`--refresh`** (and the footer Refresh button) re-fetches and replaces that
  slot.
- **Installed set:** still derived from disk on every open
  (`derive_installed_set` / `derive_installed_connector_set`) and updated after
  an install. Installing never invalidates the index cache.
- **Connector enrichment** (`start_catalogue_enrichment` /
  `poll_connector_enrichment`, FR-044) is cached in the same way as plugin
  enrichment: the connector slot holds enriched descriptors. *(Designer's call.)*
- **Marketplace add/remove at runtime** (`/plugins marketplace add|remove` and
  the `/connectors` equivalent, in `app/plugin.rs` / `app/connector.rs`)
  invalidates the affected slot and re-starts its background fetch, so a new
  marketplace appears on the next open, as it does today. *(Designer's call.)*
- **Partial failure** (some marketplaces answered, some failed): the slot is
  `Ready` with the entries it has, plus a record of the failed marketplaces.
  Opening the browser shows the cached entries at once and re-fetches in the
  background; the new result replaces the slot when it lands. A full success
  clears the record. This keeps today's retry-on-open behaviour without a
  spinner. *(Designer's call.)*
- **Prefetch warnings** (a skipped or refused marketplace, malformed-entry
  counts) are stored in the slot, not posted at startup. They are shown, in the
  same place as today, the first time that browser opens. *(Designer's call.)*
- **Run modes:** prefetch only in the interactive TUI and GUI, not in headless,
  one-shot or ACP modes. No config switch. *(Designer's call.)*
- `--refresh` is stored today but never read; this design is the first to give
  it an effect.
- The existing result slots (`plugin_store_result`,
  `connector_catalogue_result`) are per panel. The prefetch needs per-kind
  delivery, so a Codex result does not get dropped as "stale" while Claude is
  open. The cache is the destination, and the poll step copies from the cache
  into the open browser.

### 5.5 Snapshot data

`StoreSnapshot` currently flattens everything into strings. Replace it with
structured row and detail data (name, version, marketplace, description, tags,
installed, plus a plugin- or connector-specific detail enum) so that each dialog
can draw its own rows and details. The filter-rail counts need either a
structured `(label, count)` accessor on the browsers or parsing of the existing
display string. The builder chooses, preferring an accessor if it is small.

## 6. Left to the builder

- The exact `material` components to add (for example a badge, a navigation rail
  item with a count, or a segmented button if `segmented.rs` does not fit), each
  with a test.
- Where in App startup the prefetch is triggered, and how GUI and TUI frames
  poll it.
- Exact colours for the sign-in badges, from `Palette::m3` roles, passing the
  existing contrast gates.
- Whether the search field becomes a real focusable text field or stays a
  read-only display of the query.

## 7. Verification

- app-core tests: prefetch fills the cache (fixture fetcher); open with `Ready`
  shows no Loading; open with `Pending` does not start a second fetch;
  `--refresh` replaces the slot; disabled connectors skip the prefetch; a failed
  slot retries on open.
- GUI tests: each dialog renders its own rows and details fields (sign-in badge,
  scopes, servers for connectors; version and homepage for plugins); the
  Codex/Claude switcher; the per-row Install; the existing mouse and keyboard
  tests still pass.
- Manual: run the GUI, open each of the three commands, and confirm the dialog
  is wide and populated without a spinner after startup.
