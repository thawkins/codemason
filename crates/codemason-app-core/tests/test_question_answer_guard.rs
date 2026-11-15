//! Tests for the question answer guard in app-core's key handler
//! (`App::question_answerable`, `QUESTION_ANSWER_DELAY`), driven through the
//! terminal frontend's `handle_key`.
//!
//! An answer removes the front question at once, so without the guard a fast
//! double Enter (or Esc) answers the question that replaced it before the
//! user has seen it. The front question's typed answer and highlight are its
//! own: a question queued behind it, or the late echo of an answer, must not
//! reset them.

#[path = "support/mod.rs"]
mod support;

use crate::term_events::handle_key;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_agent::event::Event;
use codemason_app_core::app::App;
use codemason_app_core::app::QUESTION_ANSWER_DELAY;

/// The session the questions belong to.
const SESSION: &str = "session-question-guard";

/// The agent asks question `id` with `options` (free text when empty).
async fn ask(app: &mut App, id: &str, options: &[&str]) {
    app.handle_event(Event::QuestionRequested {
        session_id: SESSION.to_string(),
        request_id: id.to_string(),
        question: format!("question {id}?"),
        options: options.iter().map(ToString::to_string).collect(),
        default_index: if options.is_empty() { None } else { Some(0) },
        secret: false,
    })
    .await;
}

/// The App's answer to `id` comes back through the bus.
async fn echo(app: &mut App, id: &str) {
    app.handle_event(Event::QuestionAnswered {
        session_id: SESSION.to_string(),
        request_id: id.to_string(),
        response: "echo".to_string(),
        secret: false,
    })
    .await;
}

/// Press `code` with no modifiers.
async fn press(app: &mut App, code: KeyCode) {
    handle_key(app, KeyEvent::new(code, KeyModifiers::NONE)).await;
}

/// The `(request_id, response)` of the `QuestionAnswered` events published
/// so far.
fn answers(bus: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = bus.try_recv() {
        if let Event::QuestionAnswered {
            request_id,
            response,
            ..
        } = event
        {
            out.push((request_id, response));
        }
    }
    out
}

/// A test App for the guard session, its answer clock frozen.
fn guarded_app() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some(SESSION.to_string());
    app.advance_permission_clock_for_tests(std::time::Duration::ZERO);
    app
}

/// Move the frozen clock past the answer delay. Nothing sleeps.
fn wait_shown(app: &mut App) {
    app.advance_permission_clock_for_tests(QUESTION_ANSWER_DELAY);
}

#[tokio::test]
async fn test_question_double_enter_answers_once_and_never_the_next_unread() {
    let mut app = guarded_app();
    let mut bus = app.event_bus.subscribe();
    ask(&mut app, "q-1", &["yes", "no"]).await;
    ask(&mut app, "q-2", &["left", "right"]).await;

    press(&mut app, KeyCode::Enter).await;
    assert!(answers(&mut bus).is_empty(), "too soon after it was shown");

    wait_shown(&mut app);
    press(&mut app, KeyCode::Enter).await;
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(
        answers(&mut bus),
        vec![("q-1".to_string(), "yes".to_string())],
        "answered exactly once"
    );
    assert_eq!(
        app.session.question_queue.front().map(|r| r.id.as_str()),
        Some("q-2"),
        "q-1 leaves the queue at once; q-2 stays pending"
    );

    // q-1's answer comes back late: it removes nothing, and an immediate Esc
    // (a second press) still does not dismiss q-2.
    echo(&mut app, "q-1").await;
    press(&mut app, KeyCode::Esc).await;
    assert!(answers(&mut bus).is_empty());
    assert_eq!(app.session.question_queue.len(), 1, "q-2 stays pending");

    wait_shown(&mut app);
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(
        answers(&mut bus),
        vec![("q-2".to_string(), "left".to_string())]
    );
}

#[tokio::test]
async fn test_question_typed_answer_survives_a_queued_question_and_a_late_echo() {
    let mut app = guarded_app();
    let mut bus = app.event_bus.subscribe();
    ask(&mut app, "q-3", &[]).await;
    press(&mut app, KeyCode::Char('a')).await;
    press(&mut app, KeyCode::Char('b')).await;

    // Another question queues behind the one being answered.
    ask(&mut app, "q-4", &[]).await;
    assert_eq!(app.session.pending_question_input, "ab");

    wait_shown(&mut app);
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(
        answers(&mut bus),
        vec![("q-3".to_string(), "ab".to_string())]
    );

    // The user starts on q-4 before q-3's answer comes back.
    press(&mut app, KeyCode::Char('c')).await;
    echo(&mut app, "q-3").await;
    assert_eq!(app.session.pending_question_input, "c");
    assert_eq!(
        app.session.question_queue.front().map(|r| r.id.as_str()),
        Some("q-4")
    );
}

#[tokio::test]
async fn test_question_highlight_survives_a_queued_question() {
    let mut app = guarded_app();
    let mut bus = app.event_bus.subscribe();
    ask(&mut app, "q-5", &["x", "y", "z"]).await;
    press(&mut app, KeyCode::Down).await;
    assert_eq!(app.session.question_selected_index, 1);

    ask(&mut app, "q-6", &["only"]).await;
    assert_eq!(app.session.question_selected_index, 1);

    // A permission request arriving meanwhile leaves the highlight alone too.
    app.handle_event(Event::PermissionRequested {
        session_id: SESSION.to_string(),
        request_id: "perm-1".to_string(),
        permission: "bash".to_string(),
        description: "ls".to_string(),
        options: Vec::new(),
    })
    .await;
    assert_eq!(app.session.question_selected_index, 1);

    wait_shown(&mut app);
    press(&mut app, KeyCode::Enter).await;
    assert_eq!(
        answers(&mut bus),
        vec![("q-5".to_string(), "y".to_string())]
    );
}
