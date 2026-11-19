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

#[tokio::test]
async fn test_run_action_shows_the_shortcuts_panel() {
    let mut app = support::make_app();
    app.show_shortcuts = false;
    assert!(app.run_action("help.shortcuts").await);
    assert!(app.show_shortcuts);
    assert!(app.run_action("palette.show").await);
    assert!(app.palette.is_some());
    assert!(!app.run_action("no.such").await);
}

#[tokio::test]
async fn test_run_action_opens_pickers() {
    let mut app = support::make_app();
    use codemason_app_core::palette::PaletteScope;
    assert!(app.run_action("picker.agent").await);
    assert_eq!(
        app.palette.as_ref().map(|p| p.scope),
        Some(PaletteScope::Agent)
    );
    assert!(app.run_action("picker.session").await);
    assert_eq!(
        app.palette.as_ref().map(|p| p.scope),
        Some(PaletteScope::Session)
    );
    app.palette = None;
    assert!(app.run_action("picker.model").await);
    assert!(app.provider_setup.is_some(), "the model or provider picker");
}

#[test]
fn test_leader_settings_from_config() {
    use codemason_app_core::shortcuts::{DEFAULT_LEADER, LeaderSettings};
    let mut config = codemason_config::Config::default();
    assert_eq!(
        LeaderSettings::from_config(&config).key.as_deref(),
        Some(DEFAULT_LEADER)
    );
    config.leader = Some("none".to_string());
    config.leader_timeout_ms = Some(500);
    let settings = LeaderSettings::from_config(&config);
    assert_eq!(settings.key, None);
    assert_eq!(settings.timeout_ms, 500);
    // Leader keys come from the catalog.
    assert_eq!(
        codemason_app_core::shortcuts::action("picker.model").and_then(|a| a.leader_key),
        Some('m')
    );
}
