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

/// The GUI closes its context menu by sending Esc, and it lays out the
/// transcript itself, so `message_area` stays zero. That Esc must close the
/// menu and leave the running turn alone: it used to prune the menu as
/// stale and then fall through to halting the agent, so a right-click Copy
/// during a turn cancelled it.
#[tokio::test]
async fn test_esc_closing_gui_context_menu_keeps_turn_running() {
    let (mut app, flag) = processing_app("");
    app.open_transcript_context_menu(true, Some("notice".to_string()));

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(app.context_menu.is_none(), "Esc closes the menu");
    assert!(
        !flag.load(Ordering::Relaxed),
        "closing the menu must not cancel the turn"
    );
}

/// The GUI clears `esc_stops_turn` for an Esc that did not come from the
/// focused prompt; such an Esc leaves the running turn alone.
#[tokio::test]
async fn test_esc_from_outside_the_prompt_keeps_turn_running() {
    let (mut app, flag) = processing_app("");
    app.esc_stops_turn = false;

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;

    assert!(
        !flag.load(Ordering::Relaxed),
        "an Esc from outside the prompt must not cancel the turn"
    );
}
