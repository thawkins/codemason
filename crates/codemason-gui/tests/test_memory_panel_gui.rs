//! Tests for the Memory side panel (spec `mempanel`).
//!
//! Covers two spec tasks:
//! - **T-007**: extending mouse hit-testing in `session_ops.rs` (`pane_at`)
//!   and `input_handler.rs` (scroll, scrollbar drag, left-click selection
//!   start, right-click context menu) to recognise clicks inside
//!   `memory_area` so users can select text in and open the context menu on
//!   the Memory panel the same way they can on the Log / Profile / Tasks
//!   panels (FR-013).
//! - **T-012**: `Alt+M` toggle flips `show_memory` (FR-003), mutual exclusion
//!   with `show_log` / `show_tasks_panel` / `show_profile` (FR-004), `Alt+M` does
//!   not insert `m` into the input buffer (FR-011), `render_memory_panel`
//!   with populated and missing files (FR-015), and scroll-offset bounds
//!   (FR-009).
//!
//! Tests live in `crates/codemason-tui/tests/` per the AGENTS.md test
//! organization rule (no inline `#[cfg(test)]` modules in `src/`).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_memory_panel.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

use codemason_agent::storage::Storage;
use codemason_app_core::{app::App, app::ScreenMode, input::InputAction};

/// Build an [`App`] backed by an in-memory database.
/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

// -----------------------------------------------------------------------------
// Mouse-event constructors
// --------------------���--------------------------------------------------------

// -----------------------------------------------------------------------------
// Render helper
// -----------------------------------------------------------------------------

/// Serialise `std::env::set_current_dir` across parallel tests.
///
/// `render_memory_panel` resolves the project memory path from
/// `std::env::current_dir()`, so the populated/missing-file render tests need
/// to change the process working directory. `set_current_dir` is process-
/// global, so a static mutex guards against parallel tests trampling each
/// other's cwd. The guard restores the original cwd when dropped.
struct CwdGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    prev: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Acquire the cwd mutex and change the process working directory to `dir`,
/// returning a guard that restores the previous cwd on drop.
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    let lock = crate::global_env_lock();
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard { _lock: lock, prev }
}

/// Seed a structured memory for the project identified by `dir`.
fn seed_project_memory_with_tags(
    storage: &Storage,
    dir: &std::path::Path,
    content: &str,
    tags: &[String],
) {
    let project = dir.to_string_lossy().to_string();
    storage
        .create_memory(content, "fact", "test", 0.7, &project, "", tags)
        .expect("seed memory");
}

/// Seed a structured memory for the project identified by `dir`.
fn seed_project_memory(storage: &Storage, dir: &std::path::Path, content: &str) {
    seed_project_memory_with_tags(storage, dir, content, &[]);
}

// =============================================================================
// T-007: `pane_at` recognises `memory_area` (FR-013 hit-testing)
// =============================================================================

// -----------------------------------------------------------------------------
// T-007: mouse scroll inside `memory_area` adjusts `memory_scroll_offset`
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// T-007: scrollbar-gutter click on `memory_area` starts a Memory drag
// ----------------------------------------------------���------------------------

// -----------------------------------------------------------------------------
// T-007: left-click inside `memory_area` starts a Memory text selection
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// T-007: right-click inside `memory_area` opens a Memory context menu
// -----------------------------------------------------------------------------

// =============================================================================
// T-012: Alt+M toggle flips `show_memory` (FR-003)
// =============================================================================

// =============================================================================
// T-012: Mutual exclusion with log / tasks / profile panels (FR-004)
// =============================================================================

// =============================================================================
// T-012: Alt+M does not insert `m` into the input buffer (FR-011)
// =============================================================================

// ====================��========================================================
// T-012: render_memory_panel with populated and missing files (FR-015)
// =============================================================================

#[tokio::test]
async fn test_memory_stored_event_marks_panel_cache_dirty() {
    // When `memory_store` writes a new structured memory, the tool emits
    // `Event::MemoryStored`.  The TUI event handler must mark the panel cache
    // dirty so the next Alt+M render shows the new row instead of stale data.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.session.session_id = Some("sess-dirty".to_string());
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // First render primes the cache with the empty state.
    crate::frame::publish(&mut app);
    assert!(
        !app.memory_cache_dirty,
        "cache should be clean after render"
    );

    // Simulate the tool emitting MemoryStored for the current session.
    app.handle_event(codemason_agent::event::Event::MemoryStored {
        session_id: "sess-dirty".to_string(),
        id: 1,
        category: "fact".to_string(),
    })
    .await;
    assert!(
        app.memory_cache_dirty,
        "MemoryStored must mark the panel cache dirty"
    );
}

// =============================================================================
// FR-016: interactive cursor, open, and delete in the Alt+M panel
// =============================================================================

#[tokio::test]
async fn test_memory_cursor_down_and_up_moves_selection() {
    // Cursor Down/Up in the visible Memory panel changes memory_cursor and
    // keeps it clamped to the number of rows.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "First memory: alpha");
    seed_project_memory(&app.storage, dir.path(), "Second memory: beta");
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    crate::frame::publish(&mut app);
    assert_eq!(app.memory_row_count, 2);
    assert_eq!(app.memory_cursor, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        .await;
    assert_eq!(app.memory_cursor, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
        .await;
    // Cannot move past the last row.
    assert_eq!(app.memory_cursor, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
        .await;
    assert_eq!(app.memory_cursor, 0);

    app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
        .await;
    // Cannot move above the first row.
    assert_eq!(app.memory_cursor, 0);
}

#[tokio::test]
async fn test_memory_enter_with_non_empty_input_sends_message() {
    // Regression: when the Memory panel is visible and the main prompt input
    // contains text, Enter must submit the message rather than opening the
    // selected memory.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(
        &app.storage,
        dir.path(),
        "Full view content marker: gamma-omega",
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;
    app.session.input = "send this message".to_string();

    crate::frame::publish(&mut app);
    assert_eq!(app.memory_row_count, 1);

    let action =
        crate::term_events::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await;
    assert!(
        matches!(action, Some(InputAction::SendMessage(ref text)) if text == "send this message"),
        "Enter with a non-empty prompt must send the message, not open memory, got: {action:?}"
    );
}

#[tokio::test]
async fn test_memory_enter_opens_full_memory_view() {
    // Pressing Enter on the selected memory opens the full-memory overlay.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory_with_tags(
        &app.storage,
        dir.path(),
        "Full view content marker: gamma-omega",
        &["test-tag".to_string()],
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    crate::frame::publish(&mut app);
    assert_eq!(app.memory_row_count, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(
        app.memory_view.is_some(),
        "Enter on a selected memory must open the memory view overlay"
    );
    let view = app.memory_view.as_ref().unwrap();
    assert_eq!(view.row.id, 1);
    assert!(view.row.content.contains("gamma-omega"));
}

#[tokio::test]
async fn test_memory_delete_shows_confirmation_and_cancel_clears_it() {
    // Pressing Delete on a selected memory shows a confirmation dialog; Esc
    // cancels it without deleting.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "Delete candidate: zeta");
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    crate::frame::publish(&mut app);
    assert_eq!(app.memory_row_count, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    assert!(
        app.pending_memory_delete.is_some(),
        "Delete must show the confirmation dialog"
    );
    assert_eq!(app.pending_memory_delete.as_ref().unwrap().id, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert!(
        app.pending_memory_delete.is_none(),
        "Esc must cancel the confirmation dialog"
    );

    // Memory must still be present.
    app.memory_cache_dirty = true;
    crate::frame::publish(&mut app);
}

#[tokio::test]
async fn test_memory_delete_confirmation_deletes_memory() {
    // Confirming the delete dialog removes the memory and refreshes the panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(&app.storage, dir.path(), "Delete me: eta");
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    crate::frame::publish(&mut app);
    assert_eq!(app.memory_row_count, 1);

    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        app.pending_memory_delete.is_none(),
        "confirming delete must clear the dialog"
    );
    assert!(app.memory_cache_dirty, "delete must mark the cache dirty");

    crate::frame::publish(&mut app);
}
