//! Tests for the action catalog (`codemason_app_core::shortcuts`) and
//! `App::run_action`.

use codemason_app_core::shortcuts::{action, action_catalog};

mod support;

#[test]
fn test_action_catalog_lookup() {
    assert!(action_catalog().len() > 10);
    let new = action("session.new").expect("session.new");
    assert_eq!(new.default_key, Some("Ctrl+N"));
    assert!(new.key.is_some());
    assert!(action("no.such").is_none());
    // Every action without a terminal key press is one app-core or the
    // frontend runs; help.shortcuts is app-core's.
    let help = action("help.shortcuts").expect("help.shortcuts");
    assert_eq!(help.key, None);
}

#[test]
fn test_run_action_shows_the_shortcuts_panel() {
    let mut app = support::make_app();
    app.show_shortcuts = false;
    assert!(app.run_action("help.shortcuts"));
    assert!(app.show_shortcuts);
    assert!(app.run_action("palette.show"));
    assert!(app.palette.is_some());
    assert!(!app.run_action("no.such"));
}
