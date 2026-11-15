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

#[path = "support/mod.rs"]
mod support;

use codemason_tui::crossterm_input::CrosstermAppExt;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::TempDir;

use codemason_agent::{
    event::EventBus,
    storage::Storage,
    tool::{Tool, ToolContext, structured_memory::MemoryStoreTool},
};
use codemason_tui::{App, app::ScreenMode, input::InputAction, layout};
use serde_json::json;

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

/// Render the app into a string buffer of the given terminal size.
///
/// Mirrors the helper in `test_tasks_panel.rs`: draws `layout::render` into a
/// `TestBackend` and flattens the cell buffer into a single string so tests
/// can assert on visible text.
fn render_app_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render memory panel");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

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

#[test]
fn test_render_memory_panel_sets_memory_area_rect() {
    // FR-002 / FR-013: while the Memory panel is visible, `app.memory_area`
    // must be populated with the panel's rect so mouse hit-testing works.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert!(
        app.memory_area.area() > 0,
        "memory_area should be set to a non-empty rect when panel is visible"
    );
}

#[test]
fn test_render_memory_panel_hidden_clears_memory_area() {
    // FR-002: when the Memory panel is not visible, `memory_area` should be
    // reset to an empty rect by the layout split so hit-testing never
    // targets a hidden panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 40);
    assert_eq!(
        app.memory_area.area(),
        0,
        "memory_area should be empty when panel is hidden"
    );
}

#[test]
fn test_render_memory_panel_with_populated_project_memory() {
    // FR-015 (positive case): when the project has structured memories, the
    // panel renders their content.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    let body = "Unique marker line: zebra-tango-mango";
    seed_project_memory(&app.storage, dir.path(), body);
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered; got:
{text}"
    );
    assert!(
        text.contains("zebra-tango-mango"),
        "panel should render the project memory content; got:\n{text}"
    );
    assert!(
        text.contains("Structured memories: 1 (37 bytes)"),
        "panel should report the memory count and total bytes; got:\n{text}"
    );
    assert!(
        text.contains("37b Unique"),
        "panel should render the per-memory byte size next to the preview; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_with_missing_files_shows_placeholder() {
    // FR-015 (missing case): when a project has no structured memories, the
    // panel renders a placeholder and does NOT abort rendering.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered even with no memories"
    );
    assert!(
        text.contains("(no memories for this project)"),
        "missing memories should render the placeholder; got:
{text}"
    );
}

#[test]
fn test_render_memory_panel_does_not_panic_with_no_home_dir() {
    // FR-015 robustness: the panel must not panic when the user memory path
    // cannot be resolved. `dirs::home_dir()` returning None is handled by a
    // dedicated "(no user memory)" branch in `render_memory_panel`.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // Rendering should complete without panicking regardless of home dir.
    let _ = render_app_to_string(&mut app, 120, 40);
}

#[test]
fn test_render_memory_panel_re_reads_files_on_every_render() {
    // FR-010: the panel re-reads the SQLite store when the cache is dirty
    // so external changes are reflected without restarting the TUI.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    let first = render_app_to_string(&mut app, 140, 40);
    assert!(first.contains("(no memories for this project)"));

    seed_project_memory(
        &app.storage,
        dir.path(),
        "Fresh content marker: kiwi-rewrite",
    );
    // Mark the cache dirty so the next render refreshes from SQLite.
    app.memory_cache_dirty = true;
    let second = render_app_to_string(&mut app, 140, 40);
    assert!(
        second.contains("kiwi-rewrite"),
        "second render should pick up the newly stored memory; got:
{second}"
    );
}

#[test]
fn test_render_memory_panel_sets_max_scroll_when_content_overflows() {
    // FR-009: when the rendered content exceeds the visible height,
    // `memory_max_scroll` is set to a positive value.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // Seed many memories to force content overflow.
    for i in 0..60u32 {
        seed_project_memory(
            &app.storage,
            dir.path(),
            &format!("overflow line number {i}"),
        );
    }

    let _ = render_app_to_string(&mut app, 120, 20);
    assert!(
        app.memory_max_scroll > 0,
        "memory_max_scroll should be > 0 when content overflows the panel, got {}",
        app.memory_max_scroll
    );
}

#[tokio::test]
async fn test_memory_store_tool_content_appears_in_memory_panel() {
    // Regression for the bug where `memory_store` wrote memories under the
    // directory basename ("myproject") while the TUI memory panel and the
    // `/memory show` slash command queried by the full working directory path
    // ("/home/user/myproject"). The mismatch made freshly stored memories
    // invisible in the panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let tool = MemoryStoreTool;
    let ctx = ToolContext {
        session_id: "memory-panel-regression-sess".to_string(),
        working_dir: dir.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(app.storage.clone()),
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.path().to_path_buf()],
        tool_registry: ToolContext::default_tool_registry(),
    };
    tool.execute(
        json!({
            "content": "memory_store panel visibility marker: banana-kangaroo-sapphire",
            "category": "fact",
            "confidence": 0.85,
            "tags": ["regression", "memory-panel"],
            "source": "memory_store"
        }),
        &ctx,
    )
    .await
    .expect("memory_store tool should execute");

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Memory"),
        "panel border/title should be rendered; got:\n{text}"
    );
    assert!(
        text.contains("banana-kangaroo-sapphire"),
        "panel must render a memory written by memory_store using the same project key as the panel; got:\n{text}"
    );
    assert!(
        text.contains("Structured memories: 1 (62 bytes)"),
        "panel should report the single stored memory and its byte size; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_shows_tags_for_structured_memories() {
    // The Memory panel should render the comma-separated tags stored for each
    // structured memory alongside the content preview.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory_with_tags(
        &app.storage,
        dir.path(),
        "Memory with tags marker: alpha-bravo",
        &["rust".to_string(), "tui".to_string()],
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("alpha-bravo"),
        "panel should render the memory content; got:\n{text}"
    );
    assert!(
        text.contains("tags: rust, tui"),
        "panel should render the associated tags; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_omits_tags_line_when_memory_has_no_tags() {
    // Memories without tags should not produce a 'tags:' line in the panel.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    seed_project_memory(
        &app.storage,
        dir.path(),
        "No-tags memory marker: charlie-delta",
    );
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("charlie-delta"),
        "panel should render the memory content; got:\n{text}"
    );
    assert!(
        !text.contains("tags:"),
        "panel should not render a tags line for tag-less memory; got:\n{text}"
    );
}

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
    let first = render_app_to_string(&mut app, 140, 40);
    assert!(first.contains("(no memories for this project)"));
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

#[tokio::test]
async fn test_memory_panel_refreshes_after_stored_event() {
    // End-to-end: the MemoryStored event path causes the panel to refresh
    // from SQLite even though the cache was previously clean.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    app.session.session_id = Some("sess-refresh".to_string());
    app.show_memory = true;
    app.current_screen = ScreenMode::Chat;

    // Prime empty cache.
    let _ = render_app_to_string(&mut app, 140, 40);

    // Store a memory directly in SQLite (the tool path would also emit an event).
    seed_project_memory(
        &app.storage,
        dir.path(),
        "New memory appears after event: event-freshen",
    );

    // Simulate the event that the tool would emit.
    app.handle_event(codemason_agent::event::Event::MemoryStored {
        session_id: "sess-refresh".to_string(),
        id: 1,
        category: "fact".to_string(),
    })
    .await;

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("event-freshen"),
        "panel should render the newly stored memory after the event; got:\n{text}"
    );
}

#[test]
fn test_render_memory_panel_uses_real_cwd_path_not_tilde_display() {
    // Regression: `App::new` collapses `$HOME` to `~` in `app.cwd` for the
    // status bar.  The Memory panel must use the real filesystem path (`cwd_path`)
    // as the project key, otherwise memories stored with the real full path
    // are invisible when launched from under `$HOME`.
    let dir = TempDir::new().expect("tempdir");
    let real_path = dir.path().to_path_buf();
    let _guard = with_cwd(&real_path);

    let mut app = make_app();
    // Simulate the real-world situation: cwd is `~`-collapsed for display,
    // but the storage layer records memories against the real full path.
    app.cwd = "~/Projects/codemason".to_string();
    app.cwd_path = real_path.clone();
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    // Force the cache to refresh from SQLite using the (possibly wrong) cwd.
    app.memory_cache_dirty = true;

    seed_project_memory_with_tags(
        &app.storage,
        &real_path,
        "Tilde-path regression marker: hotel-india-juliet",
        &["regression".to_string()],
    );

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("Structured memories: 1 (48 bytes)"),
        "panel count should reflect the real-path memory, got:\n{text}"
    );
    assert!(
        text.contains("hotel-india-juliet"),
        "panel should render memory stored under the real project path even when cwd is ~-collapsed; got:\n{text}"
    );
    assert!(
        text.contains("tags: regression"),
        "panel should render the tags for the real-path memory; got:\n{text}"
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

    let _ = render_app_to_string(&mut app, 140, 40);
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

    let _ = render_app_to_string(&mut app, 140, 40);
    assert_eq!(app.memory_row_count, 1);

    let action = codemason_tui::crossterm_input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
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

    let _ = render_app_to_string(&mut app, 140, 40);
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

    let _ = render_app_to_string(&mut app, 140, 40);
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
    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        text.contains("zeta"),
        "cancelling delete must keep the memory"
    );
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

    let _ = render_app_to_string(&mut app, 140, 40);
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

    let text = render_app_to_string(&mut app, 140, 40);
    assert!(
        !text.contains("Delete me: eta"),
        "memory content must disappear after confirmed delete"
    );
    assert!(
        text.contains("(no memories for this project)"),
        "panel should show the empty placeholder after deletion"
    );
}

#[tokio::test]
async fn test_memory_cursor_scrolls_selection_into_view_with_wrapped_lines() {
    // Regression for the block-cursor scrolling bug: the cursor is tracked in
    // wrapped-line coordinates, so as soon as the selected memory preview would
    // fall below the visible area the panel scrolls. We use long memory
    // content that wraps to multiple lines and a small panel so the mismatch
    // between wrapped and unwrapped indices is observable.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    // Create enough long memories to exceed the panel height.
    for i in 0..10usize {
        seed_project_memory(
            &app.storage,
            dir.path(),
            &format!("overflow cursor memory {i}: {} ", "x".repeat(120)),
        );
    }
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    // Render at a width that causes each preview to wrap to ~3 lines, with a
    // panel height that shows only a few rows.
    let _ = render_app_to_string(&mut app, 60, 12);
    let initial_scroll = app.memory_scroll_offset;

    // Move the cursor down several rows.
    for _ in 0..6 {
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }

    let cursor_line = app
        .memory_row_line_indices
        .get(app.memory_cursor)
        .copied()
        .unwrap_or(0);
    let visible = app.memory_area.height as usize;
    assert!(
        app.memory_scroll_offset > initial_scroll,
        "cursor navigation must scroll the panel down to keep the selection visible"
    );
    assert!(
        cursor_line < app.memory_scroll_offset as usize + visible,
        "selected memory cursor must remain inside the visible window"
    );
}

#[tokio::test]
async fn test_memory_cursor_scrolls_selection_into_view() {
    // Moving the cursor to a row below the visible area adjusts the scroll
    // offset so the selected row remains visible.
    let dir = TempDir::new().expect("tempdir");
    let _guard = with_cwd(dir.path());

    let mut app = make_app();
    for i in 0..30u32 {
        seed_project_memory(
            &app.storage,
            dir.path(),
            &format!("overflow cursor memory {i}"),
        );
    }
    app.show_memory = true;
    app.show_log = false;
    app.show_profile = false;
    app.show_tasks_panel = false;
    app.current_screen = ScreenMode::Chat;

    let _ = render_app_to_string(&mut app, 120, 12);
    let initial_scroll = app.memory_scroll_offset;

    // Move the cursor to the last row.
    for _ in 0..29 {
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }

    assert!(
        app.memory_scroll_offset > initial_scroll,
        "cursor navigation must scroll the panel down to keep the selection visible"
    );
}
