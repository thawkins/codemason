//! Tests for the wide three-pane Plugin Store dialog
//! (`codemason_gui::dialogs::stores`): its size rule, the result rows and
//! the details pane, the per-row Install, the Codex | Claude switcher, the
//! footer's Refresh and the status states.
//!
//! Each test draws `show_plugin_store` over an App with a filled
//! `PluginStoreBrowser` in an egui_kittest harness, and collects the key
//! presses the dialog queues for `App::handle_key` (`take_keys`), so an
//! install is observed as a routed Enter without installing anything.

use std::sync::Arc;

use codemason_app_core::app::{App, PluginStoreBrowser, StorePane};
use codemason_app_core::input_event::{Key, KeyInput};
use codemason_gui::dialogs::stores::snapshot::StoreSnapshot;
use codemason_gui::dialogs::stores::{
    CATEGORIES_LABEL, DETAILS_LABEL, INSTALL, PLUGIN_STORE_TITLE, REFRESH, RESULTS_LABEL, RETRY,
    show_plugin_store,
};
use codemason_gui::dialogs::take_keys;
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
        },
        Rig {
            app,
            keys: Vec::new(),
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
