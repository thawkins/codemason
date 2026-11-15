//! Tests that the input history picker and the `/config list` save picker
//! never index past their entry list: Enter (and the history picker's `c`)
//! on an empty list closes the picker or does nothing, instead of panicking.
//!
//! Both pickers refuse to open with no entries today, so these pin the
//! hardening of a user key path against a list that could become empty.

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::{ConfigSavePickerState, HistoryPickerState};

/// Press `code` with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[tokio::test]
async fn test_history_picker_empty_enter_closes_without_choosing() {
    let mut app = support::make_app();
    app.session.input = "draft".to_string();
    app.history_picker = Some(HistoryPickerState {
        entries: Vec::new(),
        selected: 0,
        scroll_offset: 0,
    });

    app.handle_key_event(key(KeyCode::Char('c'))).await;
    assert!(app.history_picker.is_some(), "nothing to copy; still open");

    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(app.history_picker.is_none(), "Enter closes the picker");
    assert_eq!(app.session.input, "draft", "the draft is kept");
}

#[tokio::test]
async fn test_config_save_picker_empty_enter_closes_without_restoring() {
    let mut app = support::make_app();
    app.config_save_picker = Some(ConfigSavePickerState {
        entries: Vec::new(),
        selected: 0,
        scroll_offset: 0,
        config_dir: std::path::PathBuf::from("target/temp/picker-empty-guard"),
    });

    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(app.config_save_picker.is_none(), "Enter closes the picker");
    assert_eq!(app.status, "config list: cancelled");
}
