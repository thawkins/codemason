//! Project heading context menu and the sidebar multi-selection (spec
//! SESSIONMENU).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a project
//! heading with keys or the mouse, and checks the items and what they do.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;
use codemason_app_core::sessions_sidebar_state::SidebarRow;

#[path = "support/mod.rs"]
mod support;

/// An app with the sidebar on and two sessions in the launch project:
/// `(app, a, b)`, `b` focused.
async fn app_with_sidebar() -> (App, String, String) {
    let mut app = support::make_app();
    app.sidebar.visible = true;
    app.execute_slash_command("/session list").await;
    let a = app.session.session_id.clone().expect("session A");
    std::thread::sleep(std::time::Duration::from_millis(5));
    app.execute_slash_command("/session new").await;
    let b = app.session.session_id.clone().expect("session B");
    (app, a, b)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The project id of session `id`.
fn project_of(app: &mut App, id: &str) -> String {
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(app);
    app.sidebar
        .groups
        .iter()
        .find(|g| g.sessions.iter().any(|s| s.id == id))
        .map(|g| g.project_id.clone())
        .expect("project of session")
}

/// Index into the sidebar rows of the row matching `want`.
fn row_index(app: &mut App, want: impl Fn(&SidebarRow) -> bool) -> usize {
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(app);
    codemason_app_core::sessions_sidebar_state::build_rows(app)
        .iter()
        .position(want)
        .expect("row present")
}

/// Index of session `id`'s row.
fn session_row(app: &mut App, id: &str) -> usize {
    row_index(
        app,
        |row| matches!(row, SidebarRow::Session { id: sid, .. } if sid == id),
    )
}

#[tokio::test]
async fn test_project_menu_unread_mark_is_read_through_a_failed_state() {
    let (mut app, a, b) = app_with_sidebar().await;
    // B is unread and failed: the sidebar marker shows the failure, but the
    // unread mark is still there and the menu must offer to clear it.
    app.storage.set_session_unread(&b, true).expect("unread");
    app.session.unread = true;
    app.session.errored = true;
    app.sidebar.dirty = true;
    app.sidebar.cursor = session_row(&mut app, &b);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('m'))).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert!(
        menu.items.iter().any(|i| i.label == "Mark as read"),
        "{:?}",
        menu.items
    );
    app.handle_key_event(key(KeyCode::Char('u'))).await;

    let unread = app.storage.unread_session_ids().expect("unread ids");
    assert!(!unread.contains(&b), "mark cleared: {unread:?}");
    assert!(!unread.contains(&a));

    // A selection marked unread keeps the mark on an already-unread failed
    // session.
    app.storage.set_session_unread(&b, true).expect("unread");
    app.session.unread = true;
    app.sidebar.selected = [a.clone(), b.clone()].into_iter().collect();
    app.sidebar.cursor = session_row(&mut app, &b);
    app.sidebar.focused = true;
    app.handle_key_event(key(KeyCode::Char('m'))).await;
    app.handle_key_event(key(KeyCode::Char('u'))).await;
    let unread = app.storage.unread_session_ids().expect("unread ids");
    assert!(unread.contains(&a) && unread.contains(&b), "{unread:?}");
}

#[tokio::test]
async fn test_project_menu_backend_and_describe_commands() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);

    app.execute_slash_command(&format!("/project describe --project {project} Tools"))
        .await;
    app.execute_slash_command(&format!("/project backend --project {project} local"))
        .await;
    app.execute_slash_command(&format!("/project backend --project {project} nonsense"))
        .await;

    let row = app
        .storage
        .get_project(&project)
        .expect("get")
        .expect("row");
    assert_eq!(row.description.as_deref(), Some("Tools"));
    assert_eq!(row.execution_backend.as_deref(), Some("local"));

    app.execute_slash_command(&format!("/project backend --project {project} global"))
        .await;
    let row = app
        .storage
        .get_project(&project)
        .expect("get")
        .expect("row");
    assert_eq!(row.execution_backend, None);
}

#[tokio::test]
async fn test_project_menu_backend_switch_on_a_project_session_is_its_own() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    app.execute_slash_command(&format!("/project backend --project {project} local"))
        .await;
    app.execute_slash_command("/session new").await;
    let created = app.session.session_id.clone().expect("new session");

    app.execute_slash_command("/backend local").await;

    assert_eq!(
        app.storage
            .session_backend(&created)
            .expect("backend")
            .as_deref(),
        Some("local")
    );
    assert_eq!(app.session.active_backend, "local");
    let last = app
        .session
        .messages
        .iter()
        .rev()
        .map(|m| m.text_content())
        .find(|t| !t.is_empty())
        .unwrap_or_default();
    assert!(last.contains("This session now runs tools on"), "{last}");
}
