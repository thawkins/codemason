//! `/mcp discover` panel tests.
//!
//! The dialog lists discovered MCP servers under a block cursor. `Up`/`Down`
//! move the cursor (the list scrolls to keep it visible), `Space` toggles the
//! highlighted server's enabled state in `codemason.json` + the global ledger, and
//! `Esc` closes the dialog.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a toggle
//! never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this target opts back in
//! explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave (mirrors `test_backend_command`).
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use crate::term_events::CrosstermAppExt;
use std::sync::MutexGuard;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::mcp::discovery::{DiscoveredMcpServer, McpDiscoverySource};
use codemason_app_core::app::App;
use codemason_app_core::app::McpDiscoverState;

#[path = "support/mod.rs"]
mod support;

/// The cwd/env mutations below are process-global; one mutex serialises every
/// test in this binary against them.
/// The process-wide cwd/env lock (see `tests/all.rs`).
fn test_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Restores cwd and config env vars on drop.
struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            keys: vec![
                ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
                ("CODEMASON_CONFIG", std::env::var("CODEMASON_CONFIG").ok()),
                (
                    "CODEMASON_CONFIG_CONTENT",
                    std::env::var("CODEMASON_CONFIG_CONTENT").ok(),
                ),
            ],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.keys {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter an isolated tempdir project with the given project config contents.
fn enter_project(
    config: &str,
) -> (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
) {
    let guard = test_lock();
    let env_guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("CODEMASON_CONFIG") };
    unsafe { std::env::remove_var("CODEMASON_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");

    let codemason_dir = temp.path().join(".codemason");
    std::fs::create_dir_all(&codemason_dir).expect("create .codemason");
    std::fs::write(codemason_dir.join("codemason.json"), config).expect("write project config");
    // SEC-001: `mcp` entries are read from the user-global config, so the same
    // config is mirrored there for the tests that exercise the MCP toggle.
    let global_dir = temp.path().join(".config").join("codemason");
    std::fs::create_dir_all(&global_dir).expect("create global config dir");
    std::fs::write(global_dir.join("codemason.json"), config).expect("write global config");
    (guard, env_guard, temp)
}

/// A discovered server with the given id, on `PATH`.
fn discovered(id: &str) -> DiscoveredMcpServer {
    DiscoveredMcpServer {
        id: id.to_string(),
        name: format!("{id} MCP Server"),
        // SEC-082: the toggle now validates the executable, so use a real,
        // executable file rather than a bare name.
        executable: std::env::current_exe().expect("current exe"),
        args: Vec::new(),
        env: std::collections::HashMap::new(),
        source: McpDiscoverySource::SystemPath,
        definition_file: None,
    }
}

/// The dialog state for a set of servers, none enabled, cursor on the first row.
fn dialog(servers: Vec<DiscoveredMcpServer>) -> McpDiscoverState {
    let enabled = vec![false; servers.len()];
    McpDiscoverState {
        servers,
        enabled,
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    }
}

/// Press a key through the full input path.
async fn press(app: &mut App, code: KeyCode) {
    app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
        .await;
}

#[tokio::test]
async fn mcp_discover_down_moves_cursor_and_clamps_at_ends() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![
        discovered("a"),
        discovered("b"),
        discovered("c"),
    ]));

    press(&mut app, KeyCode::Down).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 1);

    press(&mut app, KeyCode::Down).await;
    press(&mut app, KeyCode::Down).await;
    // Clamped at the last row (index 2).
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 2);

    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 1);

    press(&mut app, KeyCode::Home).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 0);

    press(&mut app, KeyCode::End).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 2);

    // Up at the top stays put.
    press(&mut app, KeyCode::Home).await;
    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.mcp_discover.as_ref().unwrap().selected, 0);
}

#[tokio::test]
async fn mcp_discover_escape_closes_the_dialog() {
    let _guard = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("a")]));

    press(&mut app, KeyCode::Esc).await;
    assert!(app.mcp_discover.is_none());
}

#[tokio::test]
async fn mcp_discover_space_enables_then_disables_the_highlighted_server() {
    let (_guard, _env, temp) = enter_project("{}");
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("filesystem")]));

    // First Space adds the server to codemason.json and enables it.
    press(&mut app, KeyCode::Char(' ')).await;

    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(
        state.enabled[0],
        "the toggled row must read enabled (feedback: {:?})",
        state.feedback
    );
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );

    // The MCP client reads the user-global config, so the toggled entry lands
    // there (XDG_CONFIG_HOME is `<temp>/.config`).
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".config/codemason/codemason.json"))
            .expect("read global config"),
    )
    .expect("parse config");
    assert_eq!(
        config["mcp"]["filesystem"]["disabled"], false,
        "the entry must be written enabled"
    );

    // Second Space disables it (persisted in the global ledger).
    press(&mut app, KeyCode::Char(' ')).await;
    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(!state.enabled[0], "the toggled row must read disabled");
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );

    let ledger = std::fs::read_to_string(temp.path().join(".config/codemason/mcp_state.json"))
        .expect("read ledger");
    let ledger: serde_json::Value = serde_json::from_str(&ledger).expect("parse ledger");
    assert_eq!(ledger["servers"]["filesystem"], false);
}

#[tokio::test]
async fn mcp_discover_space_lifts_a_hard_disabled_override() {
    let (_guard, _env, temp) = enter_project(
        r#"{ "mcp": { "filesystem": { "type": "stdio", "command": "custom", "disabled": true } } }"#,
    );
    let mut app = support::make_app();
    app.mcp_discover = Some(dialog(vec![discovered("filesystem")]));

    press(&mut app, KeyCode::Char(' ')).await;

    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(state.enabled[0]);

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".config/codemason/codemason.json"))
            .expect("read global config"),
    )
    .expect("parse config");
    assert_eq!(config["mcp"]["filesystem"]["disabled"], false);
    // The user's own command is preserved, not overwritten by the discovery.
    assert_eq!(config["mcp"]["filesystem"]["command"], "custom");
}
