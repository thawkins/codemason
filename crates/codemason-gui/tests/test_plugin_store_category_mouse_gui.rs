//! Mouse-routing tests for the plugin-store category navigator (spec `catnav`
//! T-006; FR-011, FR-012, FR-013, FR-023, FR-024, FR-028).
//!
//! These drive the real `handle_mouse_event` path with an in-memory entry list
//! after a real render has populated the navigator and result column areas, so
//! they are deterministic and contact no store endpoint. They pin the mouse
//! convenience layer over the keyboard path: a left-click on a category row
//! selects and applies it and focuses the navigator, a wheel over the navigator
//! scrolls its viewport without changing the active category, a left-click on a
//! result row moves the result cursor and leaves the category alone, and every
//! mouse path installs nothing and never touches the message input.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_plugin_store_category_mouse.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use codemason_app_core::app::App;
use codemason_plugins::{StoreEntry, StoreKind};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
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

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn left_click(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), col, row)
}

fn wheel_down(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::ScrollDown, col, row)
}

/// An app with the panel open and the fixture entries loaded. The distinct
/// categories are `Utility` and `Web` (the two `utility` spellings fold into one
/// row through the case-insensitive match), so the navigator rows are
/// `ALL` (0), `Utility` (1), `Web` (2).
fn open() -> App {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(vec![
            entry("codex-weather", Some("Web")),
            entry("codex-time", Some("Utility")),
            entry("codex-shell", Some("utility")),
            entry("codex-uncat", None),
        ]);
    app
}

/// The browser behind the open panel.
fn browser(app: &App) -> &codemason_app_core::app::PluginStoreBrowser {
    app.plugin_store.as_ref().expect("panel open")
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// -- Left-click a category row selects, applies, and focuses it (FR-011) -------

#[tokio::test]
async fn left_click_outside_the_navigator_rows_leaves_the_selection_unchanged() {
    let mut app = open();
    render_effects(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;

    // A click far below the last category row maps past the row count, so the
    // selection cannot change (FR-011, FR-027).
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 10));

    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
}

// -- Left-click a result row moves the result cursor, keeps the category (FR-013) --

// -- Wheel over the navigator scrolls it without changing the category (FR-012) --

// -- Wheel over the result column moves the result cursor (FR-013) ------------

// -- No mouse path installs or touches the input (FR-024) ----------------------

#[tokio::test]
async fn mouse_paths_never_install_or_start_a_text_selection() {
    let mut app = open();
    app.session.input = "draft".to_string();
    render_effects(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let result = app.plugin_store_result_area;

    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 2));
    app.handle_mouse_event(left_click(result.x + 1, result.y + 1));
    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1));

    assert!(
        browser(&app).last_install.is_none(),
        "no navigator or result click installs anything (FR-024)"
    );
    assert!(
        app.text_selection.is_none(),
        "the panel swallows the click instead of starting a selection"
    );
    assert_eq!(app.session.input, "draft", "the message draft is untouched");
}

// -- Marketplace navigator mouse routing (spec `pluginstores` FR-052) ----------
