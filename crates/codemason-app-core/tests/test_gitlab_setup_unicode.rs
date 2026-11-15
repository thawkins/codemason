//! Regression tests for non-ASCII editing in the GitLab setup dialog (AG-01).
//!
//! The dialog's `url_cursor` / `token_cursor` are char indices (the paste path
//! in `app/models.rs` treats them so), but the key handler used them as byte
//! indices: typing a two-byte char then `a` panicked in `String::insert`. These tests drive
//! the real `handle_key` dispatch.

mod support;

use crate::term_events::handle_key;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::{App, ProviderSetupStep};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn gitlab_setup(active: u8) -> ProviderSetupStep {
    ProviderSetupStep::GitLabSetup {
        url_input: String::new(),
        url_cursor: 0,
        token_input: String::new(),
        token_cursor: 0,
        active_field: active,
        error: None,
    }
}

/// Return `(input, cursor)` of the active field.
fn field(app: &App) -> (String, usize) {
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::GitLabSetup {
            url_input,
            url_cursor,
            token_input,
            token_cursor,
            active_field,
            ..
        } => {
            if *active_field == 0 {
                (url_input.clone(), *url_cursor)
            } else {
                (token_input.clone(), *token_cursor)
            }
        }
        other => panic!("expected GitLabSetup, got {other:?}"),
    }
}

async fn press(app: &mut App, code: KeyCode) {
    assert!(handle_key(app, key(code)).await.is_none());
}

async fn edit_non_ascii(active: u8) {
    let mut app = support::make_app();
    app.provider_setup = Some(gitlab_setup(active));
    press(&mut app, KeyCode::Char('\u{e9}')).await;
    press(&mut app, KeyCode::Char('a')).await;
    assert_eq!(field(&app), ("\u{e9}a".to_string(), 2));
    press(&mut app, KeyCode::Left).await;
    press(&mut app, KeyCode::Char('\u{fc}')).await;
    assert_eq!(field(&app), ("\u{e9}\u{fc}a".to_string(), 2));
    press(&mut app, KeyCode::Backspace).await;
    assert_eq!(field(&app), ("\u{e9}a".to_string(), 1));
    press(&mut app, KeyCode::End).await;
    assert_eq!(field(&app), ("\u{e9}a".to_string(), 2));
    press(&mut app, KeyCode::Right).await;
    assert_eq!(
        field(&app),
        ("\u{e9}a".to_string(), 2),
        "Right stops at end"
    );
    press(&mut app, KeyCode::Backspace).await;
    press(&mut app, KeyCode::Backspace).await;
    assert_eq!(field(&app), (String::new(), 0));
}

#[tokio::test]
async fn test_gitlab_setup_url_field_edits_non_ascii_by_char() {
    edit_non_ascii(0).await;
}

#[tokio::test]
async fn test_gitlab_setup_token_field_edits_non_ascii_by_char() {
    edit_non_ascii(1).await;
}
