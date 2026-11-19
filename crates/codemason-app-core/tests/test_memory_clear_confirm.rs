//! Tests for the `/memory clear` confirmation dialog.
//!
//! Covers the `Yes`/`No` gate added to `/memory clear`:
//! - the slash command opens the dialog and removes nothing,
//! - `Yes` clears only the current project's memories,
//! - `No` and `Esc` leave memory unchanged,
//! - the dialog defaults to `No` so a stray `Enter` cannot clear memory.
//!
//! Tests live in `crates/codemason-app-core/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
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

/// `Esc` dismisses the dialog and leaves memory untouched.
#[tokio::test]
async fn test_memory_clear_esc_keeps_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");

    app.execute_slash_command("/memory clear").await;
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Esc must close the confirmation dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "Esc must leave memory unchanged"
    );
}

/// `No` (the default selection) dismisses without clearing.
#[tokio::test]
async fn test_memory_clear_no_keeps_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "keep me: alpha");

    app.execute_slash_command("/memory clear").await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Enter on the default No must close the dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        1,
        "the default No selection must leave memory unchanged"
    );
}

/// `Yes` clears the current project's memories.
#[tokio::test]
async fn test_memory_clear_yes_deletes_project_memories() {
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "delete me: alpha");
    seed_project_memory(&app.storage, dir.path(), "delete me: beta");

    app.execute_slash_command("/memory clear").await;
    // Right arrow moves the selection from No to Yes.
    app.handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.memory_clear_confirm_open,
        "Yes must close the confirmation dialog"
    );
    assert_eq!(
        project_memory_count(&app, dir.path()),
        0,
        "Yes must clear every memory for this project"
    );
}
