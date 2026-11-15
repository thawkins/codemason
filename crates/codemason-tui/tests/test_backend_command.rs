//! `/backend` execution-backend TUI surface tests (spec `openhands` T-005;
//! FR-008).
//!
//! Covers the four FR-008 obligations:
//! - the active backend is surfaced in the status bar (default `local`);
//! - `/backend` opens a switcher panel listing every registered backend with a
//!   health state, including `docker`/`podman` kinds;
//! - `Enter` (or `/backend <kind>`) switches the active backend and persists it;
//! - the panel owns the keyboard while open, and an `unavailable` backend cannot
//!   be selected.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a
//! switch never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this test target opts
//! back in explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser (`test_lock`) is intentionally held across the async
// `#[tokio::test]` bodies so cwd/env mutations cannot interleave; this mirrors
// the `test_i18n_command` / `test_plugins_command` targets.
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::MutexGuard;

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_tui::App;
use codemason_tui::layout;

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
    (guard, env_guard, temp)
}

/// Render one frame and return the painted text, one line per screen row.
fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    (0..40)
        .map(|y| {
            (0..120)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every painted row of the `/backend` panel's inner area.
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> String {
    let area = app.backend_panel_area;
    (0..40)
        .map(|y| {
            (0..120)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .lines()
        .skip(area.y as usize + 1)
        .take(area.height.saturating_sub(2) as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Status bar surfaces the active backend (FR-008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn status_bar_surfaces_the_default_local_backend() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();

    assert_eq!(
        app.session.active_backend, "local",
        "FR-019 default backend"
    );
    let frame = render_app_to_string(&mut app);
    let line2 = frame.lines().nth(1).unwrap_or("");
    assert!(
        line2.contains("Backend:local"),
        "the status bar must surface the active backend; line was: {line2}"
    );
}

// ---------------------------------------------------------------------------
// `/backend` opens the switcher panel (FR-004, FR-008)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn panel_renders_rows_and_footer() {
    let (_guard, _env, _temp) = enter_project(r#"{"defaultAgent": "general"}"#);
    let mut app = support::make_app();
    app.session.session_id = Some("backend-test".to_string());
    app.execute_slash_command("/backend").await;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, &mut app))
        .expect("draw");
    let body = panel_rows(&terminal, &app);
    assert!(body.contains("local"), "the local row must render: {body}");
    assert!(
        body.contains("[active]"),
        "the active row must carry the marker: {body}"
    );
    assert!(
        body.contains("Enter switch"),
        "the footer hint must render: {body}"
    );
}

// ---------------------------------------------------------------------------
// Panel keyboard ownership (FR-008)
// ---------------------------------------------------------------------------
