//! Session sidebar (spec MULTISESSION, milestone M5; spec `projects`).
//!
//! Renders the full layout into a `TestBackend` and drives the sidebar with
//! real key and mouse events. Sessions live in the in-memory storage
//! `make_app` builds, so nothing is written into the repository.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;

#[path = "support/mod.rs"]
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

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[tokio::test]
async fn test_sessions_sidebar_ctrl_n_opens_a_session() {
    let (mut app, _a, _b) = app_with_sidebar().await;
    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::CONTROL))
        .await;
    assert_eq!(app.background.len(), 2);
}

#[tokio::test]
async fn test_sessions_sidebar_alt_s_cycles_show_focus_hide() {
    let mut app = support::make_app();
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(app.sidebar.visible && app.sidebar.focused);
    app.handle_key_event(key(KeyCode::Char('s'), KeyModifiers::ALT))
        .await;
    assert!(!app.sidebar.visible && !app.sidebar.focused);
}

#[test]
fn test_sessions_sidebar_relative_age() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-12T12:00:00Z")
        .expect("now")
        .with_timezone(&chrono::Utc);
    let age = |t: &str| codemason_app_core::sessions_sidebar_state::relative_age(t, now);
    assert_eq!(age("2026-11-12T11:59:30Z"), "now");
    assert_eq!(age("2026-11-12T11:55:00Z"), "5m");
    assert_eq!(age("2026-11-12T09:00:00Z"), "3h");
    assert_eq!(age("2026-11-08T12:00:00Z"), "4d");
    assert_eq!(age("2026-10-29T12:00:00Z"), "2w");
    assert_eq!(age("not a time"), "");
}

// -- spec `projects`: project headings, branch markers, request counts ------

/// An existing directory under the git-ignored `target/temp/` tree, unique to
/// `name`, so a fixture project is not painted as `(missing)`.
fn fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/sessions_sidebar")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("fixture dir");
    dir.canonicalize().expect("canonical")
}

/// Index of the heading row for project `id` in the sidebar's row list.
fn heading_index(app: &mut App, id: &str) -> usize {
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(app);
    codemason_app_core::sessions_sidebar_state::build_rows(app)
        .iter()
        .position(
            |row| matches!(row, codemason_app_core::sessions_sidebar_state::SidebarRow::Header { key, .. } if key == id),
        )
        .expect("project heading")
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the roots
async fn test_sessions_sidebar_n_on_a_heading_opens_a_session_in_that_project() {
    // The permitted roots depend on process-global state (see
    // `support::allow_session_root`).
    let _env = crate::global_env_lock();
    let (mut app, _a, b) = app_with_sidebar().await;
    let dir = fixture_dir("n_on_heading").join("other");
    std::fs::create_dir_all(&dir).expect("dir");
    support::allow_session_root(&app, &dir);
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project.id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::NONE))
        .await;

    let id = app.session.session_id.clone().expect("focused session");
    assert_ne!(id, b, "a new session is focused");
    let row = app.storage.get_session(&id).expect("get").expect("row");
    assert_eq!(row.project_id, project.id);
    assert_eq!(std::path::PathBuf::from(row.directory), dir);
    assert_eq!(app.cwd_path, dir, "the TUI works in the project");
    assert!(!app.sidebar.focused, "focus returns to the input");
}

#[tokio::test]
async fn test_sessions_sidebar_b_on_a_heading_prefills_the_branch_command() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    app.sidebar.cursor = heading_index(&mut app, &project_id);
    app.sidebar.focused = true;

    app.storage
        .rename_project(&project_id, "My Shop")
        .expect("rename");
    app.sidebar.dirty = true;

    app.handle_key_event(key(KeyCode::Char('b'), KeyModifiers::NONE))
        .await;

    let prefix = "/session new --project \"My Shop\" --branch ";
    let input = app.session.input.clone();
    let branch = input
        .strip_prefix(prefix)
        .unwrap_or_else(|| panic!("names the project by name: {input}"));
    let (left, right) = branch.split_once('_').expect("adjective_surname");
    assert!(
        !left.is_empty()
            && !right.is_empty()
            && branch.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
        "{branch}"
    );
    assert!(!app.sidebar.focused);
}

#[tokio::test]
async fn test_sessions_sidebar_b_uses_the_id_when_the_name_is_another_projects_id() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    let other = fixture_dir("name_is_an_id").join("other");
    let other = app
        .storage
        .ensure_project(&other.display().to_string())
        .expect("project");
    // A lookup of this name finds the other project first, by its id.
    app.storage
        .rename_project(&project_id, &other.id)
        .expect("rename");
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project_id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('b'), KeyModifiers::NONE))
        .await;

    assert!(
        app.session
            .input
            .starts_with(&format!("/session new --project {project_id} --branch ")),
        "{}",
        app.session.input
    );
}

#[tokio::test]
async fn test_sessions_sidebar_b_uses_the_id_when_the_name_is_shared() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let project_id = app
        .storage
        .get_session(&a)
        .expect("get")
        .expect("row")
        .project_id;
    let other = fixture_dir("shared_name").join("twin");
    std::fs::create_dir_all(&other).expect("dir");
    let twin = app
        .storage
        .ensure_project(&other.display().to_string())
        .expect("project");
    for id in [&project_id, &twin.id] {
        app.storage.rename_project(id, "Twin").expect("rename");
    }
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project_id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('b'), KeyModifiers::NONE))
        .await;

    assert!(
        app.session
            .input
            .starts_with(&format!("/session new --project {project_id} --branch ")),
        "{}",
        app.session.input
    );
}

#[test]
fn test_sessions_sidebar_random_branch_names_are_adjective_surname() {
    let names: std::collections::HashSet<String> = (0..200)
        .map(|_| codemason_app_core::branch_names::random_branch_name())
        .collect();
    assert!(names.len() > 100, "names vary: {}", names.len());
    assert!(!names.contains("boring_wozniak"));
    for name in &names {
        assert_eq!(name.matches('_').count(), 1, "{name}");
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{name}"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // current-thread runtime; the lock pins the roots
async fn test_sessions_sidebar_n_on_a_heading_outside_permitted_roots_is_refused() {
    let _env = crate::global_env_lock();
    let (mut app, _a, b) = app_with_sidebar().await;
    let dir = fixture_dir("n_outside_roots").join("other");
    std::fs::create_dir_all(&dir).expect("dir");
    // Pin the permitted roots: the user-global config may allow this
    // checkout's own tree, which holds the fixture.
    let elsewhere = fixture_dir("n_outside_roots_allowed");
    support::allow_session_root(&app, &elsewhere);
    let project = app
        .storage
        .ensure_project(&dir.display().to_string())
        .expect("project");
    app.sidebar.dirty = true;
    app.sidebar.cursor = heading_index(&mut app, &project.id);
    app.sidebar.focused = true;

    app.handle_key_event(key(KeyCode::Char('n'), KeyModifiers::NONE))
        .await;

    assert!(app.status.contains("outside"), "status: {}", app.status);
    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(
        app.storage
            .list_project_sessions(&project.id, false)
            .expect("list")
            .is_empty(),
        "no session was created in the project"
    );
}

#[tokio::test]
async fn test_sessions_sidebar_pinned_stays_while_filtering() {
    let (mut app, a, _b) = app_with_sidebar().await;
    app.storage
        .update_session(&a, "alpha parser")
        .expect("title");
    app.sidebar.dirty = true;
    app.sidebar.filter = "alpha".to_string();
    let rows = codemason_app_core::sessions_sidebar_state::build_rows(&app);
    assert!(
        rows.iter().any(|r| matches!(
            r,
            codemason_app_core::sessions_sidebar_state::SidebarRow::Header { label, count: 0, .. } if label == "Pinned"
        )),
        "the drop target stays while filtering: {rows:#?}"
    );
}

#[test]
fn test_sessions_sidebar_branch_name_skips_taken_names() {
    let taken: std::collections::HashSet<String> = (0..200)
        .map(|_| codemason_app_core::branch_names::random_branch_name())
        .collect();
    for _ in 0..50 {
        let name = codemason_app_core::branch_names::unused_branch_name(|n| taken.contains(n));
        assert!(!taken.contains(&name), "{name} is taken");
    }
}
