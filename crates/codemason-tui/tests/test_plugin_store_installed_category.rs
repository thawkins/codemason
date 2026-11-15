//! Tests for the `Installed` pseudo-category of the plugin-store category
//! navigator (spec `pluginstores` FR-054).
//!
//! The pseudo-category is pinned directly after the `ALL` sentinel, so the
//! installed plugins are one keypress away in both `/plugins codex` and
//! `/plugins claude`. Unlike a store category it matches the plugin id against the
//! store scan ([`PluginStoreBrowser::installed`]) rather than an entry's declared
//! `category`, so an installed plugin with no category at all still appears under
//! it. These tests drive the browser and the real renderer only: no store endpoint
//! is contacted and nothing is installed.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_plugins::{StoreEntry, StoreKind};
use codemason_tui::App;
use codemason_tui::layout;

#[path = "support/mod.rs"]
mod support;

/// A store entry with `id` and an optional `category`.
fn entry(id: &str, category: Option<&str>) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: format!("{id} description"),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: category.map(str::to_string),
    }
}

/// A browser with `entries` loaded and `installed` recorded as the store scan.

/// The ids behind the current filtered set, in order.

/// The absolute navigator row carrying `label`.

/// Three entries: two installed (`alpha` in `Web`, `beta` uncategorised) and one
/// not installed (`gamma` in `Web`).
fn sample() -> Vec<StoreEntry> {
    vec![
        entry("alpha", Some("Web")),
        entry("beta", None),
        entry("gamma", Some("Web")),
    ]
}

// -- The row is pinned directly after ALL (FR-054) -----------------------------

// -- Selecting it narrows to the store scan (FR-054) ---------------------------

// -- The name is reserved: a declared `installed` category adds no second row ---

// -- A refreshed scan re-derives an already-selected pseudo-category -----------

// -- Render (FR-054) -----------------------------------------------------------

/// Render one frame so the navigator rows are painted.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

#[test]
fn the_panel_paints_the_installed_row_under_all() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_installed(std::iter::once("alpha".to_string()).collect());
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(sample());

    let terminal = render(&mut app, 120, 40);
    let nav = app.plugin_store_nav_area;
    let buffer = terminal.backend().buffer();
    let painted: Vec<String> = (nav.y..nav.y + nav.height)
        .map(|y| {
            (nav.x..nav.x + nav.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        })
        .collect();

    let all_row = painted
        .iter()
        .position(|row| row.contains("ALL"))
        .expect("ALL row painted");
    let installed_row = painted
        .iter()
        .position(|row| row.contains("Installed"))
        .expect("Installed row painted");
    assert_eq!(
        installed_row,
        all_row + 1,
        "Installed is painted directly beneath ALL: {painted:?}"
    );
    assert!(
        painted[installed_row].contains("Installed (1)"),
        "the row carries its installed count: {:?}",
        painted[installed_row]
    );
}
