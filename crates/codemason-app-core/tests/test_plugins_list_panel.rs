//! `/plugins list` panel tests (spec `plugins` FR-009, FR-030).
//!
//! `/plugins list [--verbose]` opens an interactive panel: the plugin table is
//! pinned at the top (a fixed column heading and separator) above a scrollable
//! list under a block cursor, and the capability blocks (contributions,
//! unsupported capabilities, errors, summary, and - verbose - telemetry) are
//! listed at the end of the same scrollable body. `Up`/`Down` move the cursor and
//! `Esc` closes the panel.
//!
//! The store is redirected into a tempdir so nothing touches the developer's real
//! project. Env/cwd mutation is `unsafe` in edition 2024; the workspace denies
//! `unsafe_code`, so this target opts back in explicitly (env mutation is
//! contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave (mirrors `test_mcp_discover_panel`).
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use crate::term_events::CrosstermAppExt;
use std::sync::MutexGuard;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;

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
    dir_name: &str,
    count: usize,
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

    for i in 0..count {
        write_codex_plugin(
            &temp
                .path()
                .join(".codemason")
                .join("plugins")
                .join(format!("{dir_name}-{i:02}")),
            &format!("{dir_name}-{i:02}"),
        );
    }
    (guard, env_guard, temp)
}

/// Write a minimal Codex-dialect plugin that registers one tool and one command.
fn write_codex_plugin(dir: &std::path::Path, id: &str) {
    std::fs::create_dir_all(dir).expect("plugin dir");
    let manifest = format!(
        r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js",
             "tools": [{{ "name": "ping", "description": "Ping", "parameters": {{ "type": "object" }} }}],
             "commands": [{{ "name": "{id}-go", "description": "Go" }}] }}"#
    );
    std::fs::write(dir.join("codex-plugin.json"), manifest).expect("manifest");
    // The entry registers a tool distinct from the manifest-declared `ping`, so a
    // real enable (which merges manifest and runtime tools) has no duplicate name.
    std::fs::write(
        dir.join("index.js"),
        "codemason.register_tool({ name: \"pong\", description: \"Pong\", parameters: { type: \"object\" }, handler: function(){ return \"pong\"; } });",
    )
    .expect("entry");
}

/// Press one key through the full input path.
async fn press(app: &mut App, code: KeyCode) {
    app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
        .await;
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_escape_closes_the_panel() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    assert!(app.plugins_list.is_some());

    press(&mut app, KeyCode::Esc).await;
    assert!(
        app.plugins_list.is_none(),
        "Esc must close the `/plugins list` panel"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_down_moves_cursor_and_clamps_at_ends() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 3);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    let last = app
        .plugins_list
        .as_ref()
        .map(|s| s.view.body.len().saturating_sub(1))
        .expect("panel open");

    press(&mut app, KeyCode::Down).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, 1);

    for _ in 0..(last + 5) {
        press(&mut app, KeyCode::Down).await;
    }
    assert_eq!(
        app.plugins_list.as_ref().unwrap().selected,
        last,
        "Down must clamp at the last body line"
    );

    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last - 1);

    press(&mut app, KeyCode::Home).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, 0);

    press(&mut app, KeyCode::End).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last);

    press(&mut app, KeyCode::Up).await;
    press(&mut app, KeyCode::Up).await;
    assert_eq!(app.plugins_list.as_ref().unwrap().selected, last - 2);
}
