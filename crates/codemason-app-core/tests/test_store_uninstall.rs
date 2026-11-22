//! Tests for uninstalling the highlighted entry from the store browsers
//! (`App::plugin_store_uninstall_selected`,
//! `App::connector_catalogue_uninstall_selected`).
//!
//! The plugin store is pointed at a scratch directory under `target/temp`
//! through `CODEMASON_CONFIG_CONTENT`, so the removal never touches a real
//! store.

// Rust 2024 makes `std::env::set_var`/`remove_var` unsafe; this is the one
// sanctioned test use (AGENTS-LANGUAGE.md, Core Code Rules).
#![allow(unsafe_code)]

use codemason_app_core::app::{App, PluginStoreBrowser};
use codemason_connectors::{
    CatalogueBrowser, ConnectorAuthShape, ConnectorDescriptor, ConnectorId,
};
use codemason_plugins::{StoreEntry, StoreKind};

#[path = "support/mod.rs"]
mod support;

/// Sets `CODEMASON_CONFIG_CONTENT` until dropped, restoring the old value.
struct ConfigContentGuard {
    previous: Option<std::ffi::OsString>,
}

impl ConfigContentGuard {
    fn set(json: &str) -> Self {
        let previous = std::env::var_os("CODEMASON_CONFIG_CONTENT");
        // env-isolation: the caller holds the binary-wide env lock for the guard's lifetime.
        // SAFETY: the binary-wide env lock is held.
        unsafe { std::env::set_var("CODEMASON_CONFIG_CONTENT", json) };
        Self { previous }
    }
}

impl Drop for ConfigContentGuard {
    fn drop(&mut self) {
        // env-isolation: restores the value under the same lock the caller holds.
        // SAFETY: the binary-wide env lock is held.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("CODEMASON_CONFIG_CONTENT", value),
                None => std::env::remove_var("CODEMASON_CONFIG_CONTENT"),
            }
        }
    }
}

/// A store entry `id`.
fn entry(id: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: None,
    }
}

/// The plugin browser's footer notice.
fn plugin_notice(app: &App) -> String {
    app.plugin_store
        .as_ref()
        .and_then(|b| b.last_install.clone())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the config env for the whole test
async fn test_store_uninstall_plugin_removes_it_and_shows_install_again() {
    let _lock = crate::global_env_lock();
    let scratch = crate::scratch_tempdir("store-uninstall-");
    let store = scratch.path().join("store");
    let plugin = store.join("codex-weather");
    std::fs::create_dir_all(&plugin).expect("plugin dir");
    std::fs::write(
        plugin.join("codex-plugin.json"),
        r#"{ "id": "codex-weather", "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#,
    )
    .expect("manifest");
    std::fs::write(plugin.join("index.js"), "// entry").expect("entry");
    let config = serde_json::json!({ "plugins": { "store_dir": store } }).to_string();
    let _config = ConfigContentGuard::set(&config);

    let mut app = support::make_app();
    app.cwd_path = scratch.path().to_path_buf();
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(vec![entry("codex-weather")]);
    app.plugin_store = Some(browser);
    app.refresh_plugin_store_installed();
    assert!(
        app.plugin_store
            .as_ref()
            .expect("open")
            .is_installed("codex-weather"),
        "the staged plugin is found by the store scan"
    );

    let lines = app.plugin_store_uninstall_commands();
    assert_eq!(
        lines,
        [
            "/plugins disable codex-weather",
            "/plugins remove codex-weather"
        ]
    );
    assert_eq!(plugin_notice(&app), "uninstalling codex-weather...");
    for line in &lines {
        app.execute_slash_command(line).await;
    }

    assert!(!plugin.exists(), "the plugin directory is removed");
    let browser = app.plugin_store.as_ref().expect("open");
    assert!(!browser.is_installed("codex-weather"));
    assert_eq!(plugin_notice(&app), "uninstalled plugin codex-weather");
}

#[test]
fn test_store_uninstall_plugin_not_installed_yields_no_commands() {
    let mut app = support::make_app();
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(vec![entry("alpha")]);
    app.plugin_store = Some(browser);

    assert_eq!(app.plugin_store_uninstall_commands(), Vec::<String>::new());
    assert_eq!(plugin_notice(&app), "plugin alpha is not installed");
}

/// A connector `github` with no servers.
fn github() -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new("github").expect("valid id"),
        name: "github".to_string(),
        description: String::new(),
        category: "developer".to_string(),
        tags: Vec::new(),
        source: String::new(),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: Vec::new(),
        unsupported: Vec::new(),
    }
}

/// The connector browser's footer notice.
fn connector_notice(app: &App) -> String {
    app.connector_store
        .as_ref()
        .and_then(|b| b.last_install.clone())
        .unwrap_or_default()
}

#[test]
fn test_store_uninstall_connector_commands_disable_then_remove() {
    let mut app = support::make_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(vec![github()], 0);
    browser.set_installed(std::iter::once("github".to_string()).collect());
    app.connector_store = Some(browser);

    assert_eq!(
        app.connector_catalogue_uninstall_commands(),
        ["/connectors disable github", "/connectors remove github"]
    );
    assert_eq!(connector_notice(&app), "uninstalling github...");
}

#[test]
fn test_store_uninstall_connector_not_installed_yields_no_commands() {
    let mut app = support::make_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(vec![github()], 0);
    app.connector_store = Some(browser);

    assert_eq!(
        app.connector_catalogue_uninstall_commands(),
        Vec::<String>::new()
    );
    assert_eq!(connector_notice(&app), "connector github is not installed");
}
