//! Esc stops a running agent turn, and Alt+X no longer does anything.
//!
//! Alt+X used to open a stop-agent confirmation dialog; it was removed
//! because Esc already halts the turn (sets the turn cancel flag) with no
//! dialog in between.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

#[path = "support/mod.rs"]
mod support;

/// An app with a running turn whose cancel flag is returned.
fn processing_app(input: &str) -> (codemason_app_core::app::App, Arc<AtomicBool>) {
    let mut app = support::make_app();
    app.session.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.session.cancel_flag = Some(Arc::clone(&flag));
    app.session.input = input.to_string();
    app.session.input_cursor = input.chars().count();
    (app, flag)
}

#[tokio::test]
async fn test_esc_stops_running_agent_with_empty_prompt() {
    let (mut app, flag) = processing_app("");

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(
        flag.load(Ordering::Relaxed),
        "Esc sets the turn cancel flag"
    );
    assert!(app.status.contains("halting agent"), "{}", app.status);
}

#[tokio::test]
async fn test_esc_stops_running_agent_with_typed_prompt() {
    let (mut app, flag) = processing_app("draft");

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(
        flag.load(Ordering::Relaxed),
        "Esc sets the turn cancel flag"
    );
    assert_eq!(app.session.input, "draft", "Esc keeps the typed draft");
}
