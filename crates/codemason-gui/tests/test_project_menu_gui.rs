//! Project heading context menu and the sidebar multi-selection (spec
//! SESSIONMENU).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a project
//! heading with keys or the mouse, and checks the items and what they do.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_project_menu.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;
use codemason_app_core::session_menu::{MenuAction, MenuTarget};
use codemason_app_core::sessions_sidebar_state::{SidebarRow, SidebarState};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
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

/// `draw` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn draw_effects(app: &mut App) {
    crate::frame::publish(app);
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

/// Index of project `project_id`'s heading row.
fn heading(app: &mut App, project_id: &str) -> usize {
    row_index(
        app,
        |row| matches!(row, SidebarRow::Header { key, .. } if key == project_id),
    )
}

/// Index of session `id`'s row.
fn session_row(app: &mut App, id: &str) -> usize {
    row_index(
        app,
        |row| matches!(row, SidebarRow::Session { id: sid, .. } if sid == id),
    )
}

/// Opens the project menu on `project_id`'s heading with `m`.
async fn open_project_menu(app: &mut App, project_id: &str) {
    let index = heading(app, project_id);
    crate::frame::publish(app);
    app.sidebar.focused = true;
    app.sidebar.cursor = index;
    app.handle_key_event(key(KeyCode::Char('m'))).await;
}

/// Registers a second project in an existing directory under `target/temp`.
fn second_project(app: &mut App, name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/project_menu")
        .join(name);
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let dir = dir.canonicalize().expect("canonical");
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.sidebar.dirty = true;
    project.id
}

#[tokio::test]
async fn test_project_menu_disabled_item_does_nothing() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;

    app.handle_key_event(key(KeyCode::Char('e'))).await;

    assert!(
        app.sidebar.menu.is_some(),
        "a greyed item keeps the menu open"
    );
    assert!(app.sidebar.collapsed.is_empty());
}

#[tokio::test]
async fn test_project_menu_collapse_all_then_expand_all() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;

    app.handle_key_event(key(KeyCode::Char('c'))).await;

    assert!(app.sidebar.menu.is_none());
    assert!(
        app.sidebar.collapsed.contains(&project),
        "projects collapse"
    );
    assert!(app.sidebar.collapsed.len() > 1, "Pinned collapses too");

    open_project_menu(&mut app, &project).await;
    app.handle_key_event(key(KeyCode::Char('e'))).await;

    assert!(app.sidebar.collapsed.is_empty(), "everything expands");
}

#[tokio::test]
async fn test_project_menu_select_all_marks_rows_and_esc_clears() {
    let (mut app, a, b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;

    app.handle_key_event(key(KeyCode::Char('s'))).await;

    assert_eq!(app.sidebar.selected.len(), 2);
    assert!(app.sidebar.selected.contains(&a) && app.sidebar.selected.contains(&b));
    draw_effects(&mut app);
    let _width = usize::from(app.sidebar.area.width);

    app.sidebar.focused = true;
    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.sidebar.selected.is_empty());
    assert!(app.sidebar.focused, "Esc cleared the selection only");
}

#[tokio::test]
async fn test_project_menu_selection_menu_on_unselected_row_is_the_session_menu() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.sidebar.selected = std::iter::once(a.clone()).collect();
    draw_effects(&mut app);
    app.sidebar.focused = true;
    app.sidebar.cursor = session_row(&mut app, &b);

    app.handle_key_event(key(KeyCode::Char('m'))).await;

    let menu = app.sidebar.menu.as_ref().expect("menu");
    assert_eq!(menu.target, MenuTarget::Session(b));
}

#[tokio::test]
async fn test_project_menu_archive_all_archives_listed_sessions() {
    let (mut app, a, b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;

    app.handle_key_event(key(KeyCode::Char('a'))).await;

    for id in [&a, &b] {
        let row = app.storage.get_session(id).expect("get").expect("row");
        assert!(row.archived_at.is_some(), "{id} archived: {}", app.status);
    }
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(&mut app);
    let listed = codemason_app_core::sessions_sidebar_state::build_rows(&app);
    assert!(
        !listed.iter().any(|r| matches!(
            r,
            SidebarRow::Session { id, .. } if *id == a || *id == b
        )),
        "archived sessions leave the sidebar"
    );
}

#[tokio::test]
async fn test_project_menu_delete_all_asks_once_and_yes_deletes() {
    let (mut app, a, b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;

    app.handle_key_event(key(KeyCode::Char('d'))).await;

    assert!(app.row_action_confirm_open(), "Delete all asks first");
    assert!(!app.row_action_confirm_is_yes(), "the default answer is No");
    draw_effects(&mut app);

    app.handle_key_event(key(KeyCode::Left)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    for id in [&a, &b] {
        assert!(app.storage.get_session(id).expect("get").is_none(), "{id}");
    }
}

#[tokio::test]
async fn test_project_menu_move_down_reorders_and_persists() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let first = project_of(&mut app, &a);
    let second = second_project(&mut app, "second");
    assert!(heading(&mut app, &first) < heading(&mut app, &second));
    open_project_menu(&mut app, &first).await;
    let menu = app.sidebar.menu.as_ref().expect("menu");
    let enabled = |action: MenuAction| {
        menu.items
            .iter()
            .find(|i| i.action == action)
            .expect("item")
            .enabled
    };
    assert!(!enabled(MenuAction::MoveUp) && enabled(MenuAction::MoveDown));

    app.handle_key_event(key(KeyCode::Char('j'))).await;

    assert!(heading(&mut app, &second) < heading(&mut app, &first));
    let first_heading = heading(&mut app, &first);
    assert_eq!(app.sidebar.cursor, first_heading, "cursor follows");
    let saved = SidebarState::load(&app.storage);
    assert_eq!(saved.project_order, [second.clone(), first.clone()]);

    open_project_menu(&mut app, &first).await;
    app.handle_key_event(key(KeyCode::Char('k'))).await;
    assert!(heading(&mut app, &first) < heading(&mut app, &second));
}

#[tokio::test]
async fn test_project_menu_rename_prefills_and_renames_that_project() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let _ = project_of(&mut app, &a);
    let other = second_project(&mut app, "renamed_fixture");
    open_project_menu(&mut app, &other).await;

    app.handle_key_event(key(KeyCode::Char('r'))).await;

    let want = format!("/project rename --project {other} renamed_fixture");
    assert_eq!(app.session.input, want);
    assert!(!app.sidebar.focused, "focus moves to the input");

    app.execute_slash_command(&format!("/project rename --project {other} Toolbox"))
        .await;

    let renamed = app
        .storage
        .list_projects()
        .expect("projects")
        .into_iter()
        .find(|p| p.id == other)
        .expect("project");
    assert_eq!(renamed.name, "Toolbox", "{}", app.status);
}

#[tokio::test]
async fn test_project_menu_open_in_uses_the_project_directory() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let _ = project_of(&mut app, &a);
    let other = second_project(&mut app, "open_in_fixture");
    app.sidebar.launch_log = Some(Vec::new());
    open_project_menu(&mut app, &other).await;
    let menu = app.sidebar.menu.as_mut().expect("menu");
    menu.cursor = menu
        .items
        .iter()
        .position(|i| i.action == MenuAction::OpenIn)
        .expect("Open in offered");
    app.handle_key_event(key(KeyCode::Right)).await;
    let sub = app
        .sidebar
        .menu
        .as_mut()
        .and_then(|m| m.submenu.as_mut())
        .expect("submenu");
    sub.cursor = sub
        .items
        .iter()
        .position(|i| i.label == "File manager")
        .expect("file manager offered");

    app.handle_key_event(key(KeyCode::Enter)).await;

    let log = app.sidebar.launch_log.as_ref().expect("log");
    assert_eq!(log.len(), 1, "{}", app.status);
    assert!(
        log[0].cwd.ends_with("project_menu/open_in_fixture"),
        "{:?}",
        log[0].cwd
    );
}

#[tokio::test]
async fn test_project_menu_select_all_follows_the_filter() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha notes")
        .expect("title");
    app.storage.update_session(&b, "beta draft").expect("title");
    app.sidebar.dirty = true;
    let project = project_of(&mut app, &a);
    app.sidebar.filter = "alpha".to_string();
    open_project_menu(&mut app, &project).await;

    let menu = app.sidebar.menu.clone().expect("menu open");
    assert!(
        menu.items.iter().any(|i| i.label == "Archive all (1)"),
        "counts follow the filter: {:?}",
        menu.items
    );
    app.handle_key_event(key(KeyCode::Char('s'))).await;

    assert_eq!(
        app.sidebar.selected.iter().collect::<Vec<_>>(),
        vec![&a],
        "the session hidden by the filter is not selected"
    );
}

#[tokio::test]
async fn test_project_menu_selection_skips_sessions_hidden_by_the_filter() {
    let (mut app, a, b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha notes")
        .expect("title");
    app.storage.update_session(&b, "beta draft").expect("title");
    app.sidebar.dirty = true;
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(&mut app);
    app.sidebar.selected = [a.clone(), b.clone()].into_iter().collect();
    app.sidebar.filter = "alpha".to_string();

    draw_effects(&mut app);
    app.sidebar.focused = true;
    codemason_app_core::session_menu::open_selection(&mut app, 1);
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert_eq!(menu.items[0].label, "Pin (1)", "{:?}", menu.items);

    app.handle_key_event(key(KeyCode::Char('p'))).await;

    let pinned = app.storage.pinned_session_ids().expect("pins");
    assert!(pinned.contains(&a), "{pinned:?}");
    assert!(!pinned.contains(&b), "hidden by the filter: {pinned:?}");
}

#[tokio::test]
async fn test_project_menu_properties_saves_description_and_backend() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;
    let menu = app.sidebar.menu.clone().expect("menu open");
    assert!(
        menu.items.iter().any(|i| i.label == "Properties..."),
        "{:?}",
        menu.items
    );

    app.handle_key_event(key(KeyCode::Char('p'))).await;
    let dialog = app.sidebar.properties.clone().expect("properties open");
    assert_eq!(dialog.project_id, project);
    assert!(dialog.backends[0].label.starts_with("Global ("));
    assert!(dialog.models[0].label.starts_with("Global ("));

    for c in "Agent home".chars() {
        app.handle_key_event(key(KeyCode::Char(c))).await;
    }
    // Backend: step from `Global` to `local`, always available.
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    let dialog = app.sidebar.properties.clone().expect("still open");
    assert_eq!(
        dialog.backends[dialog.backend].value.as_deref(),
        Some("local")
    );
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.sidebar.properties.is_none(), "saved and closed");
    let row = app
        .storage
        .get_project(&project)
        .expect("get")
        .expect("row");
    assert_eq!(row.description.as_deref(), Some("Agent home"));
    assert_eq!(row.execution_backend.as_deref(), Some("local"));
    draw_effects(&mut app);

    // A session created now copies the project's backend.
    app.execute_slash_command("/session new").await;
    let created = app.session.session_id.clone().expect("new session");
    assert_eq!(
        app.storage
            .session_backend(&created)
            .expect("backend")
            .as_deref(),
        Some("local")
    );
    assert_eq!(app.session.active_backend, "local");
}

#[tokio::test]
async fn test_project_menu_properties_esc_discards_and_disabled_backend_is_skipped() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    open_project_menu(&mut app, &project).await;
    app.handle_key_event(key(KeyCode::Char('p'))).await;
    app.handle_key_event(key(KeyCode::Char('x'))).await;

    // No remote backend is configured in tests: `remote` is greyed out, and
    // stepping left from `Global` skips it.
    app.handle_key_event(key(KeyCode::Tab)).await;
    let dialog = app.sidebar.properties.clone().expect("open");
    let remote = dialog
        .backends
        .iter()
        .position(|c| c.value.as_deref() == Some("remote"))
        .expect("remote listed");
    assert!(!dialog.backends[remote].enabled, "{:?}", dialog.backends);
    app.handle_key_event(key(KeyCode::Left)).await;
    let dialog = app.sidebar.properties.clone().expect("open");
    assert_ne!(dialog.backend, remote);

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(app.sidebar.properties.is_none());
    let row = app
        .storage
        .get_project(&project)
        .expect("get")
        .expect("row");
    assert_eq!((row.description, row.execution_backend), (None, None));
}

#[tokio::test]
async fn test_project_menu_properties_paste_and_long_description_stay_in_view() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project = project_of(&mut app, &a);
    app.session.input = "draft".to_string();
    open_project_menu(&mut app, &project).await;
    app.handle_key_event(key(KeyCode::Char('p'))).await;

    // A paste goes into the description, not the chat draft behind it.
    let long = format!("{}TAIL", "word ".repeat(14));
    app.handle_paste_text(&format!("{long}\nmore"));

    let dialog = app.sidebar.properties.clone().expect("open");
    assert_eq!(app.session.input, "draft", "the chat draft is untouched");
    assert!(
        dialog.description.starts_with(&long),
        "{}",
        dialog.description
    );
    assert!(!dialog.description.contains('\n'));
    assert!(dialog.description.chars().count() <= 80);

    // The caret is past the visible width; the field shows the text around it.
    draw_effects(&mut app);

    // On a choice field a paste changes nothing.
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_paste_text("ignored");
    let after = app.sidebar.properties.clone().expect("open");
    assert_eq!(after.description, dialog.description);
    assert_eq!(app.session.input, "draft");
}
