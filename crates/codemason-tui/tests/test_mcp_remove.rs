//! `/mcp remove <id>` tests.
//!
//! The command disconnects a server and deletes its definition from the file
//! that declares it: the `mcp` section of the `codemason.json` for a configured
//! server, or the `mcpServers` section of a plugin manifest / external
//! `mcp.json` for a plugin-contributed one. `/mcp` reports the defining file in
//! its `Defined in` column so the user can see what would be deleted.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a write
//! never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this target opts back in
//! explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::MutexGuard;

use codemason_tui::App;

#[path = "support/mod.rs"]
mod support;

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

/// Enter an isolated tempdir project, optionally seeding the user-global
/// `codemason.json` with `global_config`.
fn enter_project(
    global_config: Option<&str>,
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
    std::fs::write(codemason_dir.join("codemason.json"), "{}").expect("write project config");

    if let Some(config) = global_config {
        let global_dir = temp.path().join(".config").join("codemason");
        std::fs::create_dir_all(&global_dir).expect("create global config dir");
        std::fs::write(global_dir.join("codemason.json"), config).expect("write global config");
    }
    codemason_config::Config::invalidate_load_cache();
    (guard, env_guard, temp)
}

fn app() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("mcp-remove".to_string());
    app
}

/// The discover panel paints the defining file beside each row, and widens past
/// its former 90 columns so an absolute path is readable.
#[tokio::test]
async fn mcp_discover_panel_paints_the_definition_file() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let (_guard, _env, _temp) = enter_project(None);
    let mut app = app();
    app.mcp_discover = Some(codemason_tui::app::McpDiscoverState {
        servers: vec![codemason_agent::mcp::discovery::DiscoveredMcpServer {
            id: "registry-one".to_string(),
            name: "Registry One".to_string(),
            executable: std::path::PathBuf::from("/usr/bin/mcp-registry-one"),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            source: codemason_agent::mcp::discovery::McpDiscoverySource::McpRegistry {
                registry_dir: std::path::PathBuf::from("/opt/registry"),
            },
            definition_file: Some(std::path::PathBuf::from(
                "/home/user/.config/codemason/servers/registry-one/server.json",
            )),
        }],
        enabled: vec![true],
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    });
    // Keep the panel open (Esc would dismiss it) but leave the slash menu quiet.
    app.slash_menu = None;

    let backend = TestBackend::new(170, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render app");
    let buffer = terminal.backend().buffer();
    let painted: String = (0..40)
        .map(|y| {
            (0..170)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(painted.contains("Defined in"), "painted:\n{painted}");
    // The path is longer than the old 90-column panel could show, so seeing its
    // tail proves the dialog widened rather than clipping it away.
    assert!(
        painted.contains("servers/registry-one/server.json"),
        "the defining file must be painted:\n{painted}"
    );
}
