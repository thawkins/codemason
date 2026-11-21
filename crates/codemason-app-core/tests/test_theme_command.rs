//! `/theme` slash-command tests: registration, switching, the picker,
//! reporting, errors.

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};

mod support;

/// An App whose theme catalog is the built-in one and whose theme is
/// `system`, whatever theme files the developer's machine has.
fn make_app() -> codemason_app_core::app::App {
    let mut app = support::make_app();
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::System;
    app
}

fn last_text(app: &codemason_app_core::app::App) -> String {
    app.session
        .messages
        .last()
        .expect("command should create a message")
        .text_content()
}

async fn press(app: &mut codemason_app_core::app::App, key: Key) {
    let _ = app
        .handle_key(KeyInput {
            key,
            mods: Modifiers::default(),
        })
        .await;
}

fn named(name: &str) -> ThemeSelection {
    ThemeSelection::Named(name.to_string())
}

#[test]
fn test_theme_registered_in_slash_commands() {
    let _config = crate::scratch_config_target();
    let def = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "theme")
        .expect("theme must be registered in SLASH_COMMANDS");
    assert!(def.description.contains("picker"), "{}", def.description);
}

#[tokio::test]
async fn test_theme_switches_and_reports() {
    let _config = crate::scratch_config_target();
    let mut app = make_app();

    app.execute_slash_command("/theme high-contrast").await;
    assert_eq!(app.theme, named("high-contrast"));
    assert_eq!(app.status, "theme: high-contrast");

    app.execute_slash_command("/theme list").await;
    let listing = last_text(&app);
    assert!(listing.contains("Theme: **high-contrast**"), "{listing}");
    for name in [
        "`system`",
        "`codemason-dark`",
        "`codemason-light`",
        "`nord`",
    ] {
        assert!(listing.contains(name), "{name} missing from: {listing}");
    }

    app.execute_slash_command("/theme light").await;
    assert_eq!(app.theme, named("codemason-light"));

    app.execute_slash_command("/theme default").await;
    assert_eq!(app.theme, ThemeSelection::System);
    assert_eq!(app.status, "theme: system");
}

#[tokio::test]
async fn test_theme_help_and_unknown_name() {
    let _config = crate::scratch_config_target();
    let mut app = make_app();

    app.execute_slash_command("/theme help").await;
    assert!(last_text(&app).starts_with("From: /theme help"));
    assert_eq!(app.status, "theme: help");

    app.execute_slash_command("/theme solarized").await;
    assert!(last_text(&app).contains("Unknown theme `solarized`"));
    assert_eq!(app.status, "theme: usage");
    assert_eq!(
        app.theme,
        ThemeSelection::System,
        "an unknown name changes nothing"
    );
}

#[tokio::test]
async fn test_theme_picker_previews_applies_and_cancels() {
    let _config = crate::scratch_config_target();
    let mut app = make_app();
    app.execute_slash_command("/theme").await;
    let picker = app
        .theme_picker
        .clone()
        .expect("a bare /theme opens the picker");
    assert_eq!(picker.entries[0], "system");
    assert_eq!(
        picker.selected, 0,
        "the highlight starts on the current theme"
    );

    // Down previews the next theme without closing the picker.
    press(&mut app, Key::Down).await;
    assert_eq!(app.theme, named(&picker.entries[1]));
    assert!(app.theme_picker.is_some());

    // Esc restores the theme the picker opened on.
    press(&mut app, Key::Esc).await;
    assert_eq!(app.theme, ThemeSelection::System);
    assert!(app.theme_picker.is_none());

    // Enter keeps the previewed theme.
    app.execute_slash_command("/theme").await;
    press(&mut app, Key::Down).await;
    press(&mut app, Key::Down).await;
    press(&mut app, Key::Enter).await;
    assert_eq!(app.theme, named(&picker.entries[2]));
    assert!(app.theme_picker.is_none());

    // Down on the last entry stays there.
    app.execute_slash_command("/theme").await;
    let last = picker.entries.len() - 1;
    app.theme_picker_select(last);
    press(&mut app, Key::Down).await;
    assert_eq!(app.theme_picker.as_ref().map(|p| p.selected), Some(last));
}

#[tokio::test]
async fn test_theme_takes_the_whole_argument_as_the_name() {
    let _config = crate::scratch_config_target();
    let mut app = make_app();
    app.themes.insert(
        codemason_app_core::theme_tokens::ThemeTokens::from_json(
            &include_str!("../themes/codemason-dark.json")
                .replace("\"codemason-dark\"", "\"Ocean Mist\""),
        )
        .expect("parse"),
    );
    app.execute_slash_command("/theme Ocean Mist").await;
    assert_eq!(app.theme, named("ocean mist"));
    assert_eq!(app.status, "theme: ocean mist");
}

#[tokio::test]
async fn test_theme_picker_rows_select_catalog_names_not_aliases() {
    let _config = crate::scratch_config_target();
    let mut app = make_app();
    // A user theme whose name is also the `dark` alias.
    app.themes.insert(
        codemason_app_core::theme_tokens::ThemeTokens::from_json(
            &include_str!("../themes/codemason-dark.json")
                .replace("\"codemason-dark\"", "\"dark\""),
        )
        .expect("parse"),
    );
    app.open_theme_picker();
    let row = app
        .theme_picker
        .as_ref()
        .and_then(|p| p.entries.iter().position(|e| e == "dark"))
        .expect("the user theme is listed");
    app.theme_picker_select(row);
    assert_eq!(app.theme, named("dark"));
    app.theme_picker_select(0);
    assert_eq!(app.theme, ThemeSelection::System);
}

#[test]
fn test_theme_config_key_parses_and_merges() {
    let _config = crate::scratch_config_target();
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

#[test]
fn test_new_app_starts_in_the_startup_theme() {
    let _config = crate::scratch_config_target();
    // No launch configuration named a theme in this process, so a new App
    // follows the system, whatever the developer's own config file says.
    assert_eq!(codemason_app_core::theme_tokens::startup(), None);
    assert_eq!(support::make_app().theme, ThemeSelection::System);
}

/// The `theme` key of the config file at `path`, if the file sets one.
fn saved_theme(path: &std::path::Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).expect("config is JSON");
    json.get("theme")?.as_str().map(str::to_string)
}

/// `/theme <name>` saves the theme, so a restart opens in it; other keys in
/// the file are kept.
#[tokio::test]
async fn test_theme_command_persists_choice() {
    let config = crate::scratch_config_target();
    let target = config.path();
    let mut app = make_app();
    std::fs::create_dir_all(target.parent().expect("parent")).expect("dir");
    std::fs::write(target, r#"{"username":"kept"}"#).expect("seed config");

    app.execute_slash_command("/theme nord").await;

    assert_eq!(saved_theme(target).as_deref(), Some("nord"));
    let raw = std::fs::read_to_string(target).expect("config");
    assert!(raw.contains("\"username\": \"kept\""), "{raw}");
    assert_eq!(app.status, "theme: nord");
}

/// Enter in the picker saves the previewed theme; Esc saves nothing.
#[tokio::test]
async fn test_theme_picker_enter_persists_and_esc_does_not() {
    let config = crate::scratch_config_target();
    let target = config.path();
    let mut app = make_app();

    app.execute_slash_command("/theme").await;
    press(&mut app, Key::Down).await;
    press(&mut app, Key::Esc).await;
    assert_eq!(saved_theme(target), None, "Esc must not save");

    app.execute_slash_command("/theme").await;
    press(&mut app, Key::Down).await;
    let previewed = app.theme.display_name().to_string();
    press(&mut app, Key::Enter).await;
    assert_eq!(saved_theme(target), Some(previewed));
}

/// Choosing a theme from the command palette saves it too.
#[tokio::test]
async fn test_theme_palette_choice_persists() {
    let config = crate::scratch_config_target();
    let target = config.path();
    let mut app = make_app();

    app.run_palette_entry("theme:high-contrast").await;

    assert_eq!(saved_theme(target).as_deref(), Some("high-contrast"));
}
