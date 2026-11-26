//! Tests for the wide three-pane Plugin Store dialog
//! (`codemason_gui::dialogs::stores`): its size rule, the result rows and
//! the details pane, the per-row Install, the Codex | Claude switcher, the
//! footer's Refresh (with its refreshing and failure notes) and the status
//! states.
//!
//! Each test draws `show_plugin_store` over an App with a filled
//! `PluginStoreBrowser` in an egui_kittest harness, and collects the key
//! presses the dialog queues for `App::handle_key` (`take_keys`), so an
//! install is observed as a routed Enter without installing anything.

use std::sync::Arc;

use codemason_app_core::app::{App, PluginStoreBrowser, StoreCacheKey, StorePane};
use codemason_app_core::input_event::{Key, KeyInput};
use codemason_gui::dialogs::stores::frame::{RAIL_WIDTH, dialog_size};
use codemason_gui::dialogs::stores::snapshot::StoreSnapshot;
use codemason_gui::dialogs::stores::{
    CATEGORIES_LABEL, DETAILS_LABEL, INSTALL, MARKETPLACES_LABEL, PLUGIN_STORE_TITLE, REFRESH,
    REFRESH_FAILED, REFRESHING, RESULTS_LABEL, RETRY, UNINSTALL, show_plugin_store,
};
use codemason_gui::dialogs::{take_keys, take_slash_commands};
use codemason_plugins::{
    FetchLimits, StoreEndpoint, StoreEntry, StoreError, StoreIndex, StoreIndexFetcher, StoreKind,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

#[path = "support_material.rs"]
#[allow(dead_code)] // Only its harness builder is used here.
mod support_material;

/// A fetcher that stays busy for longer than any assertion, so a started
/// fetch is still in flight when the test checks for it.
struct StalledFetcher;

impl StoreIndexFetcher for StalledFetcher {
    fn fetch_index(
        &self,
        _kind: StoreKind,
        _endpoint: &StoreEndpoint,
        _limits: &FetchLimits,
    ) -> Result<StoreIndex, StoreError> {
        std::thread::sleep(std::time::Duration::from_secs(2));
        Err(StoreError::Network {
            detail: "stalled".to_string(),
        })
    }
}

/// What the harness drives: the App and the keys the dialog queued.
struct Rig {
    app: App,
    keys: Vec<KeyInput>,
    slashes: Vec<String>,
}

/// A store entry `id` with version `version`, marketplace `acme` and `tags`.
fn entry(id: &str, version: &str, tags: &[&str]) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: version.to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: format!("{id} description"),
        dialect: None,
        tags: tags.iter().map(|t| (*t).to_string()).collect(),
        homepage: Some(format!("https://example.org/{id}/home")),
        marketplace: "acme".to_string(),
        category: None,
    }
}

/// An App with the Codex store open over `entries`; fetches stall.
fn app_with(runtime: &tokio::runtime::Runtime, entries: Vec<StoreEntry>) -> App {
    let _enter = runtime.enter();
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(Arc::new(StalledFetcher));
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(entries);
    app.plugin_store = Some(browser);
    app
}

/// Three plain entries.
fn three() -> Vec<StoreEntry> {
    vec![
        entry("alpha", "1.0.0", &[]),
        entry("bravo", "2.0.0", &[]),
        entry("charlie", "3.0.0", &[]),
    ]
}

/// A harness of `size` drawing the plugin store over `app`.
fn rig(size: egui::Vec2, app: App) -> Harness<'static, Rig> {
    let mut harness = support_material::builder().with_size(size).build_ui_state(
        |ui, rig: &mut Rig| {
            show_plugin_store(ui.ctx(), &mut rig.app);
            rig.keys.extend(take_keys(ui.ctx()));
            rig.slashes.extend(take_slash_commands(ui.ctx()));
        },
        Rig {
            app,
            keys: Vec::new(),
            slashes: Vec::new(),
        },
    );
    harness.run();
    harness
}

/// Drop `harness` and stop `runtime` without waiting for the stalled
/// fetch a test started.
fn finish(runtime: tokio::runtime::Runtime, harness: Harness<'static, Rig>) {
    drop(harness);
    runtime.shutdown_background();
}

/// The window the tests draw in unless they say otherwise.
const WIDE: egui::Vec2 = egui::vec2(1400.0, 900.0);

/// The dialog's rect.
fn dialog_rect(harness: &Harness<'static, Rig>) -> egui::Rect {
    harness
        .get_by_role_and_label(Role::Dialog, PLUGIN_STORE_TITLE)
        .rect()
}

/// The result list's rect.
fn list_rect(harness: &Harness<'static, Rig>) -> egui::Rect {
    harness
        .get_by_role_and_label(Role::List, RESULTS_LABEL)
        .rect()
}

/// The in-flight generation of the Codex plugin slot.
fn codex_in_flight(app: &App) -> Option<u64> {
    app.store_cache
        .lock()
        .expect("store cache")
        .plugin(StoreKind::Codex)
        .in_flight
}

#[test]
fn test_store_dialog_plugin_dialog_is_wide() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let harness = rig(WIDE, app_with(&rt, three()));
    let dialog = dialog_rect(&harness);
    assert!(dialog.width() >= 960.0, "dialog {dialog:?}");
    let expected = WIDE.x * 0.85;
    assert!(
        (dialog.width() - expected).abs() < 2.0,
        "dialog width {} is about 85% of {}",
        dialog.width(),
        WIDE.x
    );
    // The rail, the list and the details sit side by side inside it.
    let rail = harness
        .get_by_role_and_label(Role::List, CATEGORIES_LABEL)
        .rect();
    let list = list_rect(&harness);
    let details = harness
        .get_by_role_and_label(Role::Group, DETAILS_LABEL)
        .rect();
    for pane in [rail, list, details] {
        assert!(
            dialog.expand(0.5).contains_rect(pane),
            "{pane:?} in {dialog:?}"
        );
    }
    assert!(rail.right() <= list.left() && list.right() <= details.left());
}

#[test]
fn test_store_dialog_small_window_clamps() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let small = egui::vec2(800.0, 500.0);
    let harness = rig(small, app_with(&rt, three()));
    let dialog = dialog_rect(&harness);
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, small).expand(0.5);
    assert!(
        window.contains_rect(dialog),
        "dialog {dialog:?} in {small:?}"
    );
    // The size rule asks for the whole window; the dialog keeps the usual
    // margin inside it (80 pt: its padding and a gap).
    assert!((dialog.width() - 720.0).abs() < 1.0, "dialog {dialog:?}");
    assert!((dialog.height() - 420.0).abs() < 1.0, "dialog {dialog:?}");
}

#[test]
fn test_store_dialog_keeps_a_usable_list_at_the_minimum_window() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    // The smallest window the GUI opens (window::MIN_SIZE).
    let small = egui::vec2(640.0, 420.0);
    let harness = rig(small, app_with(&rt, three()));
    let dialog = dialog_rect(&harness);
    let rail = harness
        .get_by_role_and_label(Role::List, CATEGORIES_LABEL)
        .rect();
    let list = list_rect(&harness);
    // The rail and the details pane are capped to a share of a narrow body,
    // so the result list keeps width instead of collapsing between them.
    assert!(
        list.width() >= 120.0,
        "at {small:?} the result list collapses to {} points: {list:?}",
        list.width()
    );
    assert!(
        rail.right() <= list.left(),
        "the rail {rail:?} overlaps the list {list:?}"
    );
    assert!(
        list.right() <= dialog.max.x + 1.0,
        "the list {list:?} runs past the dialog {dialog:?}"
    );
    finish(rt, harness);
}

#[test]
fn test_store_dialog_size_rule() {
    assert_eq!(
        dialog_size(egui::vec2(800.0, 500.0)),
        egui::vec2(800.0, 500.0)
    );
    assert_eq!(
        dialog_size(egui::vec2(1000.0, 700.0)),
        egui::vec2(960.0, 600.0)
    );
    assert_eq!(
        dialog_size(egui::vec2(2000.0, 1000.0)),
        egui::vec2(1700.0, 850.0)
    );
}

#[test]
fn test_store_dialog_row_shows_version_and_marketplace() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut entries = three();
    entries.push(entry("delta", "", &[]));
    let app = app_with(&rt, entries);
    let snap = StoreSnapshot::plugin(app.plugin_store.as_ref().expect("open"));
    assert_eq!(snap.rows[1].subtitle, "2.0.0 - acme");
    assert_eq!(snap.rows[3].subtitle, "acme", "a blank version is left out");
    let harness = rig(WIDE, app);
    let list = list_rect(&harness);
    assert!(
        harness
            .get_all_by_label("2.0.0 - acme")
            .any(|node| list.contains_rect(node.rect())),
        "row two shows its version and marketplace"
    );
}

#[test]
fn test_store_dialog_details_show_homepage_and_source() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let harness = rig(WIDE, app_with(&rt, three()));
    harness.get_by_role_and_label(Role::Button, "https://example.org/alpha/home");
    harness.get_by_label("https://example.org/alpha.zip");
    // The highlighted row is the selected list item for screen readers.
    let selected = |name: &str| {
        harness
            .get_by_role_and_label(Role::ListItem, name)
            .accesskit_node()
            .is_selected()
    };
    assert_eq!(selected("alpha"), Some(true));
    assert_ne!(selected("bravo"), Some(true));
}

#[test]
fn test_store_dialog_row_install_installs_that_row() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut harness = rig(WIDE, app_with(&rt, three()));
    let list = list_rect(&harness);
    let mut installs: Vec<egui::Rect> = harness
        .get_all_by_role_and_label(Role::Button, INSTALL)
        .map(|node| node.rect())
        .filter(|rect| list.contains(rect.center()))
        .collect();
    installs.sort_by(|a, b| a.top().total_cmp(&b.top()));
    assert_eq!(installs.len(), 3, "one Install per row");
    let target = installs[2].center();
    harness
        .get_all_by_role_and_label(Role::Button, INSTALL)
        .find(|node| node.rect().center() == target)
        .expect("row two's Install")
        .click();
    harness.run();
    let browser = harness.state().app.plugin_store.as_ref().expect("open");
    assert_eq!(browser.cursor, 2);
    assert_eq!(browser.focus, StorePane::Results);
    assert!(
        harness.state().keys.iter().any(|k| k.key == Key::Enter),
        "an Enter was routed: {:?}",
        harness.state().keys
    );
}

#[test]
fn test_store_dialog_installed_row_uninstall_queues_disable_then_remove() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut app = app_with(&rt, three());
    if let Some(browser) = app.plugin_store.as_mut() {
        browser.set_installed(std::iter::once("bravo".to_string()).collect());
    }
    let mut harness = rig(WIDE, app);
    let list = list_rect(&harness);
    let in_list = |label: &str, harness: &Harness<'static, Rig>| -> usize {
        harness
            .get_all_by_role_and_label(Role::Button, label)
            .filter(|node| list.contains(node.rect().center()))
            .count()
    };
    assert_eq!(in_list(INSTALL, &harness), 2, "Install on the two others");
    assert_eq!(in_list(UNINSTALL, &harness), 1, "Uninstall on bravo");
    harness
        .get_all_by_role_and_label(Role::Button, UNINSTALL)
        .find(|node| list.contains(node.rect().center()))
        .expect("bravo's Uninstall")
        .click();
    harness.run();
    let browser = harness.state().app.plugin_store.as_ref().expect("open");
    assert_eq!(browser.cursor, 1);
    assert_eq!(
        harness.state().slashes,
        ["/plugins disable bravo", "/plugins remove bravo"]
    );
    assert!(harness.state().keys.is_empty(), "no Enter was routed");
    // The details pane follows the highlight onto the installed row.
    assert_eq!(
        harness
            .get_all_by_role_and_label(Role::Button, UNINSTALL)
            .count(),
        2,
        "the row's and the details pane's Uninstall"
    );
    finish(rt, harness);
}

#[test]
fn test_store_dialog_draws_only_rows_in_view_and_follows_the_highlight() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let entries: Vec<StoreEntry> = (0..200)
        .map(|i| entry(&format!("plugin-{i:03}"), "1.0.0", &[]))
        .collect();
    let mut harness = rig(WIDE, app_with(&rt, entries));
    let list = list_rect(&harness);
    let drawn = harness
        .get_all_by_role_and_label(Role::Button, INSTALL)
        .filter(|node| list.intersects(node.rect()))
        .count();
    assert!(drawn < 30, "only the rows in view are drawn, got {drawn}");
    if let Some(browser) = harness.state_mut().app.plugin_store.as_mut() {
        browser.cursor = 150;
    }
    harness.run();
    let rows: Vec<egui::Rect> = harness
        .query_all_by_label("plugin-150")
        .map(|node| node.rect())
        .collect();
    assert!(
        rows.iter().any(|row| list.contains(row.center())),
        "the highlighted row is scrolled into view: {rows:?} in {list:?}"
    );
    finish(rt, harness);
}

#[test]
fn test_store_dialog_rail_splits_in_half_and_scrolls_each_section() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let entries: Vec<StoreEntry> = (0..60)
        .map(|i| StoreEntry {
            category: Some(format!("cat-{i:02}")),
            ..entry(&format!("plugin-{i:02}"), "1.0.0", &[])
        })
        .collect();
    let harness = rig(WIDE, app_with(&rt, entries));
    let categories = harness
        .get_by_role_and_label(Role::List, CATEGORIES_LABEL)
        .rect();
    let marketplaces = harness
        .get_by_role_and_label(Role::List, MARKETPLACES_LABEL)
        .rect();
    assert!(
        (categories.width() - RAIL_WIDTH).abs() < 1.0,
        "rail width {} is {RAIL_WIDTH}",
        categories.width()
    );
    assert!(
        (marketplaces.min.y - categories.max.y).abs() < 1.0,
        "marketplaces {marketplaces:?} start where categories {categories:?} end"
    );
    assert!(
        (marketplaces.height() - categories.height()).abs() < 1.0,
        "marketplaces take half the rail: {marketplaces:?} vs {categories:?}"
    );
    // Sixty categories overflow their half; the last is scrolled out of
    // view rather than pushing the marketplaces down.
    let first = harness.get_by_label("cat-00 (1)").rect();
    assert!(
        categories.contains(first.center()),
        "{first:?} in {categories:?}"
    );
    let last = harness.query_by_label("cat-59 (1)").map(|node| node.rect());
    assert!(
        last.is_none_or(|rect| !categories.contains(rect.center())),
        "the last category {last:?} scrolls inside {categories:?}"
    );
    finish(rt, harness);
}

#[test]
fn test_store_dialog_switcher_changes_kind() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut harness = rig(WIDE, app_with(&rt, three()));
    {
        let _enter = rt.enter();
        harness
            .get_by_role_and_label(Role::RadioButton, "Claude")
            .click();
        // The Claude store loads, and its progress bar keeps repainting.
        harness.run_steps(2);
    }
    let kind = harness.state().app.plugin_store.as_ref().map(|b| b.kind);
    assert_eq!(kind, Some(StoreKind::Claude));
    finish(rt, harness);
}

#[test]
fn test_store_dialog_refresh_button_starts_fetch() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut harness = rig(WIDE, app_with(&rt, three()));
    assert_eq!(codex_in_flight(&harness.state().app), None);
    {
        let _enter = rt.enter();
        harness.get_by_role_and_label(Role::Button, REFRESH).click();
        harness.run();
    }
    assert!(codex_in_flight(&harness.state().app).is_some());
    finish(rt, harness);
}

#[test]
fn test_store_dialog_long_row_does_not_overflow() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut long = entry("longest", "1.0.0", &["t1", "t2", "t3", "t4", "t5", "t6"]);
    long.description = "word ".repeat(60);
    let harness = rig(WIDE, app_with(&rt, vec![long]));
    let list = list_rect(&harness);
    let row = harness
        .get_by_role_and_label(Role::ListItem, "longest")
        .rect();
    assert!(
        row.width() <= list.width() + 0.5,
        "row {row:?} in list {list:?}"
    );
    let in_row = |label: &str| {
        harness
            .query_all_by_label(label)
            .any(|node| row.intersects(node.rect()))
    };
    for tag in ["t1", "t2", "t3"] {
        assert!(in_row(tag), "{tag} is drawn in the row");
    }
    for tag in ["t4", "t5", "t6"] {
        assert!(!in_row(tag), "{tag} is not drawn in the row");
    }
}

#[test]
fn test_store_dialog_no_matches_disables_install() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut app = app_with(&rt, three());
    if let Some(browser) = app.plugin_store.as_mut() {
        browser.set_query("zzz".to_string());
    }
    let harness = rig(WIDE, app);
    harness.get_by_label("No matches");
    assert!(
        harness
            .get_by_role_and_label(Role::Button, INSTALL)
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn test_store_dialog_failed_state_has_retry() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut app = app_with(&rt, Vec::new());
    if let Some(browser) = app.plugin_store.as_mut() {
        browser.set_failed("network down".to_string());
    }
    let mut harness = rig(WIDE, app);
    harness.get_by_label("network down");
    {
        let _enter = rt.enter();
        harness.get_by_role_and_label(Role::Button, RETRY).click();
        harness.run();
    }
    assert!(codex_in_flight(&harness.state().app).is_some());
    finish(rt, harness);
}

/// An App whose Codex cache slot holds `entries` and whose Codex store is
/// opened from that cache (no fetch starts: the slot is `Ready`).
fn cached_app(runtime: &tokio::runtime::Runtime, entries: Vec<StoreEntry>) -> App {
    let _enter = runtime.enter();
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(Arc::new(StalledFetcher));
    {
        let mut cache = app.store_cache.lock().expect("store cache");
        let generation = cache.begin(StoreCacheKey::Plugins(StoreKind::Codex));
        cache.fill_plugins(
            StoreKind::Codex,
            generation,
            entries,
            0,
            Vec::new(),
            Vec::new(),
        );
    }
    app.open_plugin_store(StoreKind::Codex, "", false);
    app
}

#[test]
fn test_store_dialog_retry_shows_loading_while_in_flight() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut app = {
        let _enter = rt.enter();
        support::make_app()
    };
    app.set_plugin_store_fetcher(Arc::new(StalledFetcher));
    {
        let key = StoreCacheKey::Plugins(StoreKind::Codex);
        let mut cache = app.store_cache.lock().expect("store cache");
        let generation = cache.begin(key);
        cache.fail(key, generation, "network down".to_string());
    }
    // Opened outside the runtime, the retry-on-open cannot start, so the
    // failure shows.
    app.open_plugin_store(StoreKind::Codex, "", false);
    let mut harness = rig(WIDE, app);
    harness.get_by_label("network down");
    {
        let _enter = rt.enter();
        harness.get_by_role_and_label(Role::Button, RETRY).click();
        // The loading state's progress bar keeps repainting, so step rather
        // than wait for the UI to settle.
        harness.run_steps(2);
    }
    assert!(codex_in_flight(&harness.state().app).is_some());
    harness.get_by_label("Loading the store index...");
    assert!(harness.query_by_label("network down").is_none());
    finish(rt, harness);
}

#[test]
fn test_store_dialog_refresh_shows_refreshing_note() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut harness = rig(WIDE, cached_app(&rt, three()));
    assert!(harness.query_by_label(REFRESHING).is_none());
    {
        let _enter = rt.enter();
        harness.get_by_role_and_label(Role::Button, REFRESH).click();
        harness.run();
    }
    harness.get_by_label(REFRESHING);
    harness.get_by_role_and_label(Role::ListItem, "alpha");
    finish(rt, harness);
}

#[test]
fn test_store_dialog_failed_refresh_keeps_entries_and_reports_once() {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let app = cached_app(&rt, three());
    {
        let key = StoreCacheKey::Plugins(StoreKind::Codex);
        let mut cache = app.store_cache.lock().expect("store cache");
        let generation = cache.begin(key);
        cache.fail(key, generation, "offline".to_string());
    }
    let mut harness = rig(WIDE, app);
    harness.state_mut().app.poll_store_cache();
    harness.run();
    harness.state_mut().app.poll_store_cache();
    harness.run();

    harness.get_by_label(&format!("{REFRESH_FAILED}: offline"));
    harness.get_by_role_and_label(Role::ListItem, "alpha");
    let posted = harness
        .state()
        .app
        .log_entries
        .iter()
        .filter(|e| {
            e.message
                .contains("refreshing the codex plugin store failed")
        })
        .count();
    assert_eq!(
        posted, 1,
        "the failure is posted to the message window once"
    );
    finish(rt, harness);
}
