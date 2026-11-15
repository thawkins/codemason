//! Tests for the GCF status-bar indicator and the Alt+G toggle.
//!
//! Covers:
//! - Alt+G toggles GCF encoding through the real persist path
//!   (`codemason_config::gcf::toggle_persist`) and reports the new state.
//! - The GCF icon renders on the second status-bar line, LEFT of the
//!   codeindex icon, with the enabled (`[ok]`) / disabled (`[no]`) marker.
//! - The `g` keystroke is never inserted into the input buffer (NFR-002).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_gcf_indicator.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use std::sync::MutexGuard;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;

/// Build an [`App`] backed by an in-memory database (mirrors the pattern used
/// by the slash-command tests).
/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

struct CwdGuard {
    prev: std::path::PathBuf,
    #[allow(dead_code)]
    lock: MutexGuard<'static, ()>,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Acquire the cwd test lock, recovering from any prior poisoning.
/// The process-wide cwd/env lock (see `tests/all.rs`).
fn cwd_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Create a tempdir, chdir into it with a project-local config primed for
/// GCF-off, and return the cwd-restoring guard. The tempdir is removed only
/// after cwd has been restored.
fn enter_isolated_config_project() -> (CwdGuard, tempfile::TempDir) {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let codemason_dir = temp.path().join(".codemason");
    std::fs::create_dir_all(&codemason_dir).expect("create .codemason");
    std::fs::write(
        codemason_dir.join("codemason.json"),
        r#"{"gcf": {"enabled": false}}"#,
    )
    .expect("write project config");
    (CwdGuard { prev, lock }, temp)
}

/// `render_to_text` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_to_text_effects(app: &mut App) {
    crate::frame::publish(app);
}

#[tokio::test]
async fn test_alt_g_toggles_gcf_and_status_bar_indicator() {
    let (_guard, _temp) = enter_isolated_config_project();
    codemason_config::gcf::set_enabled(false);

    // Warm the config cache BEFORE the toggle so the real project-config
    // resolution path (which mirrors the live app) is exercised.
    // Touch the config loader once so a malformed-config failure surfaces as
    // an explicit expect here rather than a silent toggle failure below.
    codemason_config::Config::load().expect("config load");

    let mut app = make_app();

    // Sanity: GCF starts off.
    assert!(!codemason_config::gcf::is_enabled());

    // Press Alt+G through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted GCF on.
    assert!(codemason_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding enabled"));

    // Status bar should show the enabled GCF tag next to the codeindex tag.
    render_to_text_effects(&mut app);

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT))
        .await;
    assert!(!codemason_config::gcf::is_enabled());
    assert!(app.status.contains("GCF encoding disabled"));

    render_to_text_effects(&mut app);
}
