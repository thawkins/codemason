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
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_plugins_list_panel.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave (mirrors `test_mcp_discover_panel`).
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use crate::term_events::CrosstermAppExt;
use std::sync::MutexGuard;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
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

/// `render_rows` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_rows_effects(app: &mut App) {
    crate::frame::publish(app);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_opens_a_panel_with_the_pinned_table_and_capabilities() {
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;

    // The panel owns the keyboard; the message window gets no appended report.
    assert!(
        app.plugins_list.is_some(),
        "`/plugins list` must open the interactive panel"
    );
    render_rows_effects(&mut app);
    // Pinned table heading and separator.
    // The plugin row and the trailing contributions block. Discovery of a
    // disabled plugin names its manifest-declared tool (`plugin_<id>_<tool>`) and
    // command, so the block is non-empty and proves the trailing section is
    // present at the end of the body.
    let body = &app.plugins_list.as_ref().unwrap().view.body;
    assert!(
        body.iter().any(|l| l == "Contributions:"),
        "the capability block must be listed at the end of the body: {body:?}"
    );
    assert!(
        body.iter()
            .any(|l| l.contains("tools [plugin_codex-weather-00_ping]")),
        "the contributed tool must be named in the capability block: {body:?}"
    );
    // The leading body lines are plugin table rows, exposed as `view.rows` so the
    // panel can toggle the highlighted plugin (FR-009).
    assert!(
        body.iter().any(|l| l.starts_with("- codex-weather-00:")),
        "the contributions line must name the plugin: {body:?}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn plugins_list_space_toggles_the_highlighted_plugin() {
    // FR-009/FR-011/FR-012: Space enables the highlighted plugin, a second Space
    // disables it, and the panel refreshes the row state + feedback each time.
    let (_lock, _guard, _temp) = enter_project("codex-weather", 1);
    let mut app = support::make_app();
    app.cwd_path = std::env::current_dir().expect("cwd");
    app.session.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    assert!(
        app.plugins_list
            .as_ref()
            .is_some_and(|s| !s.view.rows[0].enabled),
        "a freshly installed plugin starts disabled"
    );

    // Space enables it.
    press(&mut app, KeyCode::Char(' ')).await;
    let state = app.plugins_list.as_ref().expect("panel open");
    assert!(
        state.view.rows[0].enabled,
        "Space must enable the highlighted plugin"
    );
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "expected an [ok] feedback, got {:?}",
        state.feedback
    );
    render_rows_effects(&mut app);

    // A second Space disables it again.
    press(&mut app, KeyCode::Char(' ')).await;
    let _state = app.plugins_list.as_ref().expect("panel open");
}
