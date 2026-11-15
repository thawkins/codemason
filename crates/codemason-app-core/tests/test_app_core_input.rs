//! Tests for the frontend-neutral key entry point `App::handle_key`.

use codemason_app_core::app::HandleOutcome;
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};

#[path = "support/mod.rs"]
mod support;

#[tokio::test]
async fn test_handle_key_alt_l_toggles_log_panel() {
    let mut app = support::make_app();
    let before = app.show_log;
    let alt_l = KeyInput {
        key: Key::Char('l'),
        mods: Modifiers {
            alt: true,
            ..Default::default()
        },
    };

    let outcome = app.handle_key(alt_l).await;
    assert_eq!(outcome, HandleOutcome::Consumed);
    assert_eq!(app.show_log, !before, "Alt+L must flip the log panel");

    app.handle_key(alt_l).await;
    assert_eq!(app.show_log, before, "a second Alt+L must flip it back");
}

#[tokio::test]
async fn test_handle_key_unbound_char_inserts_into_input() {
    let mut app = support::make_app();
    app.session.input.clear();
    app.session.input_cursor = 0;

    let outcome = app
        .handle_key(KeyInput {
            key: Key::Char('q'),
            mods: Modifiers::default(),
        })
        .await;

    assert_eq!(outcome, HandleOutcome::Consumed);
    assert_eq!(app.session.input, "q");
    assert_eq!(app.session.input_cursor, 1);
}
