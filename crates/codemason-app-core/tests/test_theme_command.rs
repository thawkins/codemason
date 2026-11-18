//! `/theme` slash-command tests: registration, switching, reporting, errors.

use codemason_app_core::theme_mode::ThemeMode;
use support::make_app;

mod support;

fn last_text(app: &codemason_app_core::app::App) -> String {
    app.session
        .messages
        .last()
        .expect("command should create a message")
        .text_content()
}

#[test]
fn test_theme_registered_in_slash_commands() {
    let def = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "theme")
        .expect("theme must be registered in SLASH_COMMANDS");
    assert!(
        def.description.contains("high-contrast"),
        "{}",
        def.description
    );
}

#[tokio::test]
async fn test_theme_switches_and_reports() {
    let mut app = make_app();
    app.theme_mode = ThemeMode::Default;

    app.execute_slash_command("/theme high-contrast").await;
    assert_eq!(app.theme_mode, ThemeMode::HighContrast);
    assert_eq!(app.status, "theme: high-contrast");

    app.execute_slash_command("/theme").await;
    assert!(last_text(&app).contains("Theme: **high-contrast**"));

    app.execute_slash_command("/theme default").await;
    assert_eq!(app.theme_mode, ThemeMode::Default);
    assert_eq!(app.status, "theme: default");
}

#[tokio::test]
async fn test_theme_help_and_unknown_name() {
    let mut app = make_app();
    app.theme_mode = ThemeMode::Default;

    app.execute_slash_command("/theme help").await;
    assert!(last_text(&app).starts_with("From: /theme help"));
    assert_eq!(app.status, "theme: help");

    app.execute_slash_command("/theme solarized").await;
    assert!(last_text(&app).contains("Unknown theme `solarized`"));
    assert_eq!(app.status, "theme: usage");
    assert_eq!(
        app.theme_mode,
        ThemeMode::Default,
        "an unknown name changes nothing"
    );
}

#[test]
fn test_theme_config_key_parses_and_merges() {
    let base: codemason_config::Config =
        serde_json::from_str(r#"{ "theme": "high-contrast" }"#).expect("parse");
    assert_eq!(base.theme.as_deref(), Some("high-contrast"));

    let unset: codemason_config::Config = serde_json::from_str("{}").expect("parse");
    assert_eq!(unset.theme, None);
    assert!(
        !serde_json::to_string(&unset)
            .expect("serialise")
            .contains("\"theme\""),
        "an unset theme is not written"
    );

    // An overlay without the key keeps the base value; one with it wins.
    let kept = codemason_config::Config::merge(base.clone(), unset);
    assert_eq!(kept.theme.as_deref(), Some("high-contrast"));
    let overlay: codemason_config::Config =
        serde_json::from_str(r#"{ "theme": "default" }"#).expect("parse");
    let replaced = codemason_config::Config::merge(base, overlay);
    assert_eq!(replaced.theme.as_deref(), Some("default"));
}
