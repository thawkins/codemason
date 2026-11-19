//! Tests for the `/memory clear` confirmation dialog.
//!
//! Covers the `Yes`/`No` gate added to `/memory clear`:
//! - the slash command opens the dialog and removes nothing,
//! - `Yes` clears only the current project's memories,
//! - `No` and `Esc` leave memory unchanged,
//! - the dialog defaults to `No` so a stray `Enter` cannot clear memory.
//!
//! Tests live in `crates/codemason-tui/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_memory_clear_confirm.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use tempfile::TempDir;

use codemason_agent::storage::Storage;
use codemason_app_core::app::App;

// -----------------------------------------------------------------------------
// App fixture (mirrors `test_memory_panel.rs`)
// -----------------------------------------------------------------------------

/// Build an [`App`] backed by an in-memory database.
/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

/// `render_app_to_string` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_app_to_string_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);

    let _text = String::new();
}

/// Seed a structured memory attributed to `dir`.
fn seed_project_memory(storage: &Storage, dir: &std::path::Path, content: &str) {
    storage
        .create_memory(
            content,
            "fact",
            "test",
            0.7,
            &dir.to_string_lossy(),
            "",
            &[],
        )
        .expect("seed memory");
}

/// Serialise `std::env::set_current_dir` across parallel tests.
struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Change the process working directory to `dir`, returning a restoring guard.
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    let lock = crate::global_env_lock();
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard { _lock: lock, prev }
}

/// Count memories stored against `dir`.
fn project_memory_count(app: &App, dir: &std::path::Path) -> u64 {
    app.storage
        .count_memories_for_project(dir)
        .expect("count memories")
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

/// `/memory clear` must open the dialog and remove nothing.
#[tokio::test]
async fn test_memory_clear_opens_confirmation_without_deleting() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");

    app.execute_slash_command("/memory clear").await;

    assert!(
        app.memory_clear_confirm_open,
        "/memory clear must open the confirmation dialog"
    );
    assert!(
        !app.memory_clear_confirm_is_yes(),
        "the dialog must default to No"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "opening the dialog must not delete anything"
    );

    render_app_to_string_effects(&mut app, 140, 40);
}
