//! Tests for the permission answer guard in app-core's key handler
//! (`App::permission_answerable`, `PERMISSION_ANSWER_DELAY`), driven through
//! the terminal frontend's `handle_key`.
//!
//! A reply removes its request only when it comes back through the event
//! bus, so without the guard a fast double `y` answers the same request
//! twice or grants the next queued request before the user has seen it.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::event::Event;
use codemason_agent::permission::PermissionDecision;
use codemason_app_core::app::PERMISSION_ANSWER_DELAY;
use codemason_tui::app::App;
use codemason_tui::crossterm_input::handle_key;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The session the requests belong to.
const SESSION: &str = "session-guard";

/// The agent asks for permission `id`, as the event handler sees it.
async fn request(app: &mut App, id: &str) {
    app.handle_event(Event::PermissionRequested {
        session_id: SESSION.to_string(),
        request_id: id.to_string(),
        permission: "bash".to_string(),
        description: "ls".to_string(),
        options: Vec::new(),
    })
    .await;
}

/// Press `y`.
async fn press_y(app: &mut App) {
    handle_key(app, KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)).await;
}

/// The ids of the `PermissionReplied` events published so far.
fn replies(bus: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
    let mut ids = Vec::new();
    while let Ok(event) = bus.try_recv() {
        if let Event::PermissionReplied { request_id, .. } = event {
            ids.push(request_id);
        }
    }
    ids
}

/// Wait out the answer delay.
fn wait_shown() {
    std::thread::sleep(PERMISSION_ANSWER_DELAY + std::time::Duration::from_millis(50));
}

#[tokio::test]
async fn test_permission_double_y_answers_once_and_never_the_next_unread() {
    let mut app = support::make_app();
    app.session.session_id = Some(SESSION.to_string());
    let mut bus = app.event_bus.subscribe();
    request(&mut app, "req-1").await;
    request(&mut app, "req-2").await;

    press_y(&mut app).await;
    assert!(replies(&mut bus).is_empty(), "too soon after it was shown");

    wait_shown();
    press_y(&mut app).await;
    press_y(&mut app).await;
    assert_eq!(replies(&mut bus), vec!["req-1"], "answered exactly once");

    // The reply comes back and req-2 becomes the front: an immediate `y`
    // (the second half of a double press) does not grant it.
    app.handle_event(Event::PermissionReplied {
        session_id: SESSION.to_string(),
        request_id: "req-1".to_string(),
        allowed: true,
        decision: PermissionDecision::Once,
    })
    .await;
    press_y(&mut app).await;
    assert!(replies(&mut bus).is_empty());
    assert_eq!(app.session.permission_queue.len(), 1, "req-2 stays pending");

    wait_shown();
    press_y(&mut app).await;
    assert_eq!(replies(&mut bus), vec!["req-2"]);
}
