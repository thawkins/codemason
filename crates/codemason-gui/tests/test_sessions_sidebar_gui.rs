//! Session sidebar (spec MULTISESSION, milestone M5; spec `projects`).
//!
//! Renders the full layout into a `TestBackend` and drives the sidebar with
//! real key and mouse events. Sessions live in the in-memory storage
//! `make_app` builds, so nothing is written into the repository.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_sessions_sidebar.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_agent::event::Event;
use codemason_app_core::app::App;
use codemason_app_core::sessions_sidebar_state::SidebarState;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

/// An app with the sidebar on and two sessions: `(app, a, b)`, `b` focused
/// and newer than `a`.
async fn app_with_sidebar() -> (App, String, String) {
    let mut app = support::make_app();
    app.sidebar.visible = true;
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    // Distinct `updated_at` values keep the order deterministic.
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

/// `sidebar_rows` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn sidebar_rows_effects(app: &mut App) {
    crate::frame::publish(app);
    let _width = usize::from(app.sidebar.area.width);
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[tokio::test]
async fn test_sessions_sidebar_left_collapses_a_section() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(app.sidebar.focused, "Alt+S focuses the visible sidebar");
    // New session, Search, (blank), Pinned, (blank), then the project
    // heading: the cursor skips the blank rows, so three steps.
    for _ in 0..3 {
        app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
            .await;
    }
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;

    sidebar_rows_effects(&mut app);
}

#[tokio::test]
async fn test_sessions_sidebar_ctrl_down_and_alt_r_switch_sessions() {
    let (mut app, a, b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);

    app.handle_key_event(key(KeyCode::Down, KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));

    app.handle_key_event(key(KeyCode::Char('r'), KeyModifiers::ALT))
        .await;
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
}

#[tokio::test]
async fn test_sessions_sidebar_enter_on_row_switches_session() {
    let (mut app, a, _b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);
    app.sidebar.focused = true;
    // The terminal test placed the cursor on A's painted row; the row index
    // comes from the sidebar rows the panel draws.
    app.sidebar.cursor = codemason_app_core::sessions_sidebar_state::build_rows(&app)
        .iter()
        .position(|row| {
            matches!(
                row,
                codemason_app_core::sessions_sidebar_state::SidebarRow::Session { id, .. }
                    if *id == a
            )
        })
        .expect("A");

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
    assert!(!app.sidebar.focused, "focus returns to the input");
}

#[tokio::test]
async fn test_sessions_sidebar_collapsed_sections_persist() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);
    app.sidebar.focused = true;
    app.sidebar.cursor = project_heading(&app);
    app.handle_key_event(key(KeyCode::Left, KeyModifiers::NONE))
        .await;

    let loaded = SidebarState::load(&app.storage);
    assert_eq!(loaded.collapsed.len(), 1);
    assert!(loaded.visible);
}

// -- spec `projects`: project headings, branch markers, request counts ------

#[tokio::test]
async fn test_sessions_sidebar_search_row_starts_the_filter() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha parser")
        .expect("title A");
    app.storage
        .update_session(&b, "beta lexer")
        .expect("title B");
    app.sidebar.dirty = true;
    sidebar_rows_effects(&mut app);

    app.sidebar.focused = true;
    app.sidebar.cursor = 1;
    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(
        app.sidebar.filter_editing,
        "Enter on Search starts the filter"
    );
    for c in ['b', 'e', 't'] {
        app.handle_key_event(key(KeyCode::Char(c), KeyModifiers::NONE))
            .await;
    }

    sidebar_rows_effects(&mut app);
}

#[tokio::test]
async fn test_sessions_sidebar_project_update_reloads_the_rows() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    sidebar_rows_effects(&mut app);
    assert!(!app.sidebar.dirty);
    // What the request-count service does when a stored count changes.
    app.storage
        .rename_project(&project_id, "Fresh Name")
        .expect("rename");

    app.handle_event(Event::ProjectUpdated { project_id }).await;

    assert!(app.sidebar.dirty);
}

/// Index of the first project heading (the first heading after `Pinned`).
fn project_heading(app: &App) -> usize {
    codemason_app_core::sessions_sidebar_state::build_rows(app)
        .iter()
        .position(|row| {
            matches!(row, codemason_app_core::sessions_sidebar_state::SidebarRow::Header { label, .. } if label != "Pinned")
        })
        .expect("a project heading")
}

#[tokio::test]
async fn test_sessions_sidebar_blank_line_above_pinned_and_projects() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);
    let rows = codemason_app_core::sessions_sidebar_state::build_rows(&app);
    use codemason_app_core::sessions_sidebar_state::SidebarRow;
    assert!(matches!(rows[0], SidebarRow::NewSession));
    assert!(matches!(rows[1], SidebarRow::Search));
    assert!(matches!(rows[2], SidebarRow::Spacer));
    assert!(
        matches!(&rows[3], SidebarRow::Header { label, count: 0, .. } if label == "Pinned"),
        "Pinned is listed with nothing pinned: {rows:#?}"
    );
    assert!(matches!(rows[4], SidebarRow::Spacer));
    let project = project_heading(&app);
    assert!(matches!(rows[project - 1], SidebarRow::Spacer));

    // The cursor never rests on a blank row, moving either way.
    app.sidebar.focused = true;
    app.sidebar.cursor = 1;
    app.handle_key_event(key(KeyCode::Down, KeyModifiers::NONE))
        .await;
    assert_eq!(app.sidebar.cursor, 3, "Down skips the blank row");
    app.handle_key_event(key(KeyCode::Up, KeyModifiers::NONE))
        .await;
    assert_eq!(app.sidebar.cursor, 1, "Up skips it too");
}

#[tokio::test]
async fn test_sessions_sidebar_cursor_leaves_a_spacer_after_rows_change() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage.set_session_pinned(&a, true).expect("pin");
    app.sidebar.dirty = true;
    sidebar_rows_effects(&mut app);
    // Rows: New, Search, (blank), Pinned, A, (blank), project ...
    app.sidebar.cursor = 4;
    app.storage.set_session_pinned(&a, false).expect("unpin");
    app.sidebar.dirty = true;
    sidebar_rows_effects(&mut app);
    let rows = codemason_app_core::sessions_sidebar_state::build_rows(&app);
    assert!(
        !matches!(
            rows.get(app.sidebar.cursor),
            Some(codemason_app_core::sessions_sidebar_state::SidebarRow::Spacer)
        ),
        "cursor at {} rests on a selectable row: {rows:#?}",
        app.sidebar.cursor
    );
}

#[tokio::test]
async fn test_sessions_sidebar_focused_passes_alt_panel_chords_through() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    sidebar_rows_effects(&mut app);
    app.sidebar.focused = true;
    // The cursor sits on a session row, where plain `p` pins and `m` opens
    // the session menu.

    app.handle_key_event(key(KeyCode::Char('p'), KeyModifiers::ALT))
        .await;
    assert!(app.show_profile, "Alt+P toggles the profile panel");
    assert!(
        app.storage.pinned_session_ids().expect("pins").is_empty(),
        "Alt+P does not pin the cursor session"
    );

    app.handle_key_event(key(KeyCode::Char('m'), KeyModifiers::ALT))
        .await;
    assert!(app.show_memory, "Alt+M toggles the memory panel");
    assert!(app.sidebar.menu.is_none(), "Alt+M opens no session menu");
    assert!(app.sidebar.focused, "the sidebar keeps the keyboard");
}

#[tokio::test]
async fn test_sessions_sidebar_pr_badge_shows_zero_without_requests() {
    use codemason_gui::panels::sessions::{PrBadge, PrState, pr_badge};

    let (mut app, a, b) = app_with_sidebar().await;
    assert_eq!(
        pr_badge(&app, &a),
        PrBadge {
            count: 0,
            state: PrState::None
        },
        "a session with no request still has a badge, counting 0"
    );
    app.sidebar.request_totals.insert(a.clone(), 3);
    assert_eq!(pr_badge(&app, &a).count, 3);
    assert_eq!(pr_badge(&app, &a).state, PrState::Past);
    app.sidebar.bound.insert(b.clone(), false);
    assert_eq!(
        pr_badge(&app, &b),
        PrBadge {
            count: 1,
            state: PrState::Active
        }
    );
    app.sidebar.bound.insert(b.clone(), true);
    assert_eq!(pr_badge(&app, &b).state, PrState::Paused);
}

#[test]
fn test_sessions_sidebar_backend_names() {
    use codemason_config::ExecutionBackendKind;
    use codemason_gui::panels::sessions::backend_name;

    assert_eq!(backend_name(ExecutionBackendKind::Local), "Local computer");
    assert_eq!(
        backend_name(ExecutionBackendKind::Docker),
        "Docker container"
    );
    assert_eq!(
        backend_name(ExecutionBackendKind::Podman),
        "Podman container"
    );
    assert_eq!(backend_name(ExecutionBackendKind::Remote), "Remote system");
}

#[tokio::test]
async fn test_sessions_sidebar_session_details_name_backend_and_requests() {
    use codemason_config::ExecutionBackendKind;
    use codemason_gui::panels::sessions::session_details;

    let (mut app, a, _b) = app_with_sidebar().await;
    app.sidebar
        .backends
        .insert(a.clone(), ExecutionBackendKind::Docker);
    let now = chrono::Utc::now().to_rfc3339();
    let details = session_details(&app, &a, &now, Some("feature/x"), Some(2));
    assert_eq!(
        details.first().map(String::as_str),
        Some("Updated just now")
    );
    assert!(
        details.contains(&"Backend: Docker container".to_string()),
        "{details:?}"
    );
    assert!(
        details.contains(&"Branch: feature/x (2 open from it)".to_string()),
        "{details:?}"
    );
    assert_eq!(details.last().map(String::as_str), Some("Pull requests: 0"));
}
