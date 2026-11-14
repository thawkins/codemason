//! Tests for `App::housekeeping`, the one background-poll pass both
//! frontends run (the TUI from its event loop, the GUI every frame).
//!
//! The pass must run the gated polls only once `HOUSEKEEPING_INTERVAL` has
//! elapsed, and the ungated tail (memory-count copy, history flush) on every
//! call, exactly as the TUI loop did inline.

use std::sync::Arc;
use std::time::{Duration, Instant};

use codemason_tui::app::{HOUSEKEEPING_INTERVAL, QuestionRequest, SessionView};

#[path = "support/mod.rs"]
mod support;

/// Move the housekeeping gate back so the next pass is due.
fn make_housekeeping_due(app: &mut codemason_tui::App) {
    app.jobs_last_poll = Instant::now()
        .checked_sub(HOUSEKEEPING_INTERVAL + Duration::from_millis(1))
        .expect("clock");
}

/// Deposit results the gated polls pick up: a failed compaction, a
/// rollback outcome and a websearch engine-test report.
fn deposit_background_results(app: &mut codemason_tui::App) {
    app.session.compact_in_progress = true;
    *app.session.compact_result.lock().expect("compact slot") = Some(Err("boom".to_string()));
    *app.session.rollback_result.lock().expect("rollback slot") = Some(Ok(true));
    *app.websearch_test_result.lock().expect("websearch slot") = Some("engine ok".to_string());
}

#[tokio::test]
async fn test_housekeeping_runs_the_gated_polls_when_due() {
    let mut app = support::make_app();
    let processor = Arc::clone(&app.session_processor);
    deposit_background_results(&mut app);
    make_housekeeping_due(&mut app);
    let runs = app.housekeeping_runs;

    app.housekeeping(&processor).await;

    assert_eq!(app.housekeeping_runs, runs + 1);
    assert!(!app.session.compact_in_progress);
    assert!(app.session.compact_result.lock().expect("slot").is_none());
    assert!(app.session.rollback_result.lock().expect("slot").is_none());
    assert!(app.websearch_test_result.lock().expect("slot").is_none());
    // The websearch poll runs before the compaction poll, whose failure
    // status is the last one written.
    assert_eq!(app.status, "[warn] compact failed: boom");
}

#[tokio::test]
async fn test_housekeeping_skips_the_gated_polls_until_due() {
    let mut app = support::make_app();
    let processor = Arc::clone(&app.session_processor);
    app.jobs_last_poll = Instant::now();
    deposit_background_results(&mut app);
    let runs = app.housekeeping_runs;

    app.housekeeping(&processor).await;

    assert_eq!(app.housekeeping_runs, runs);
    assert!(app.session.compact_in_progress);
    assert!(app.session.compact_result.lock().expect("slot").is_some());
    assert!(app.session.rollback_result.lock().expect("slot").is_some());
    assert!(app.websearch_test_result.lock().expect("slot").is_some());
}

#[tokio::test]
async fn test_housekeeping_runs_the_ungated_tail_every_call() {
    let mut app = support::make_app();
    let processor = Arc::clone(&app.session_processor);
    app.jobs_last_poll = Instant::now();
    app.memory_entry_count = 0;
    app.memory_entry_count_pending
        .store(7, std::sync::atomic::Ordering::Relaxed);
    // A dirty history with no file configured is cleared by the flush.
    app.history_file_path = None;
    app.history_dirty = true;
    app.history_save_deadline = None;

    app.housekeeping(&processor).await;

    assert_eq!(app.memory_entry_count, 7);
    assert!(!app.history_dirty);
}

#[tokio::test]
async fn test_housekeeping_answers_an_expired_background_question() {
    let mut app = support::make_app();
    let processor = Arc::clone(&app.session_processor);
    let mut view = SessionView::new(
        app.session.agent_name.clone(),
        app.session.agent_info.clone(),
        app.session.current_agent_index,
    );
    view.session_id = Some("background-session".to_string());
    view.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "background-session".to_string(),
        question: "choose".to_string(),
        options: vec!["first".to_string(), "second".to_string()],
        default_index: Some(0),
        expire_at_ms: Some(1),
        secret: false,
    });
    app.background.insert_front(view);
    make_housekeeping_due(&mut app);

    app.housekeeping(&processor).await;

    let parked = app.background.iter().next().expect("background view");
    assert!(parked.question_queue.is_empty());
}

#[test]
fn test_next_timer_due_counts_background_question_deadlines() {
    let mut app = support::make_app();
    assert_eq!(app.next_timer_due(), None);
    let mut view = SessionView::new(
        app.session.agent_name.clone(),
        app.session.agent_info.clone(),
        app.session.current_agent_index,
    );
    view.session_id = Some("background-session".to_string());
    view.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "background-session".to_string(),
        question: "choose".to_string(),
        options: vec!["first".to_string()],
        default_index: Some(0),
        expire_at_ms: Some(1),
        secret: false,
    });
    app.background.insert_front(view);

    assert_eq!(app.next_timer_due(), Some(Duration::ZERO));
}
