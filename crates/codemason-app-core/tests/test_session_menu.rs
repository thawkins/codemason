//! Session context menu (spec SESSIONMENU, milestone M1).
//!
//! Renders the full layout into a `TestBackend`, opens the menu on a sidebar
//! row with keys or the mouse, and checks the painted menu and the actions it
//! dispatches.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;

#[path = "support/mod.rs"]
mod support;

/// An app with the sidebar on and two sessions: `(app, a, b)`, `b` focused.
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

#[tokio::test]
async fn test_session_menu_fork_refuses_a_running_session() {
    let (mut app, _a, b) = app_with_sidebar().await;
    app.session.is_processing = true;

    app.execute_slash_command("/session fork").await;

    assert_eq!(app.session.session_id.as_deref(), Some(b.as_str()));
    assert!(app.status.contains("running"), "{}", app.status);
    app.session.is_processing = false;
}

#[tokio::test]
async fn test_session_menu_delete_focused_session_focuses_another() {
    let (mut app, a, b) = app_with_sidebar().await;

    app.execute_slash_command(&format!("/session delete {b}"))
        .await;
    assert!(app.row_action_confirm_open());
    app.handle_key_event(key(KeyCode::Right)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(app.storage.get_session(&b).expect("get").is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(a.as_str()));
}

#[tokio::test]
async fn test_session_menu_delete_refuses_a_running_session() {
    let (mut app, _a, b) = app_with_sidebar().await;
    app.session.is_processing = true;

    app.execute_slash_command(&format!("/session delete {b}"))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "no dialog for a running session"
    );
    assert!(app.status.contains("running"), "{}", app.status);
    app.session.is_processing = false;
}

#[tokio::test]
async fn test_session_menu_section_slash_commands() {
    let (mut app, a, b) = app_with_sidebar().await;

    app.execute_slash_command("/session section new Alpha")
        .await;
    let placed = app.storage.session_sections().expect("placed");
    let alpha = placed[&b].clone();

    app.execute_slash_command("/session section rename Beta")
        .await;
    assert_eq!(app.storage.list_sections().expect("list")[0].name, "Beta");

    app.execute_slash_command("/session section --none").await;
    assert!(app.storage.session_sections().expect("placed").is_empty());

    app.execute_slash_command("/session section beta").await;
    assert_eq!(app.storage.session_sections().expect("placed")[&b], alpha);

    app.execute_slash_command("/session section nope").await;
    assert!(app.status.contains("no section named"), "{}", app.status);

    app.execute_slash_command("/session section delete Beta")
        .await;
    assert_eq!(app.storage.list_sections().expect("list"), Vec::new());
    assert!(app.storage.session_sections().expect("placed").is_empty());
    assert!(
        !app.storage
            .session_sections()
            .expect("placed")
            .contains_key(&a),
        "A was never moved"
    );
}

#[tokio::test]
async fn test_session_menu_delete_refuses_while_a_sub_agent_runs() {
    let (mut app, a, _b) = app_with_sidebar().await;
    let child = format!("{a}-child");
    app.storage
        .create_session_with(
            &child,
            "/nonexistent/menu",
            Some(&a),
            codemason_storage::SessionKind::Subagent,
        )
        .expect("child");
    app.active_tasks.push(codemason_agent::task::TaskEntry {
        id: "t1".to_string(),
        parent_session_id: a.clone(),
        child_session_id: child,
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: codemason_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: codemason_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    });

    app.execute_slash_command(&format!("/session delete {a}"))
        .await;

    assert!(!app.row_action_confirm_open(), "no dialog");
    assert!(app.status.contains("still running"), "{}", app.status);
    assert!(app.storage.get_session(&a).expect("get").is_some());
}
