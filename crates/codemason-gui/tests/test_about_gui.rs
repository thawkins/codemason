//! Tests for the About dialog (`codemason_gui::dialogs::about`): the Help
//! tab's About opens it, it shows the version and the repository link, the
//! logo follows the theme, and Close or a click outside dismisses it.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::dialogs::about::{
    ABOUT_TITLE, CLOSE_ABOUT, REPOSITORY_URL, VERSION, is_open, logo_uri,
};
use codemason_gui::keys::default_keymap;
use codemason_gui::ribbon::TABS;
use craft_keymap::Platform;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window in theme `theme`, with the About dialog opened from the
/// Help tab. The runtime is returned so it outlives the harness.
fn opened(theme: &str) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::Named(theme.to_string());
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(64);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    harness.get_by_label("Help").click();
    harness.run();
    harness.get_by_label("About").click();
    harness.run();
    (runtime, harness)
}

#[test]
fn test_about_help_tab_is_last_and_holds_about() {
    let help = TABS.last().expect("tabs");
    assert_eq!(help.label, "Help");
    let labels: Vec<_> = help
        .groups
        .iter()
        .flat_map(|g| g.items.iter().map(|i| i.label))
        .collect();
    assert_eq!(labels, ["About"]);
}

#[test]
fn test_about_logo_follows_the_theme() {
    assert!(logo_uri(true).contains("dark"));
    assert!(logo_uri(false).contains("light"));
}

#[test]
fn test_about_shows_version_and_repository_link() {
    let (_runtime, harness) = opened("codemason-dark");
    assert!(is_open(&harness.ctx));
    // The dialog node and its headline both carry the title.
    assert!(harness.query_all_by_label(ABOUT_TITLE).next().is_some());
    // A label node and its text run both carry the text.
    let version = format!("Version {VERSION}");
    assert!(harness.query_all_by_label(&version).next().is_some());
    assert!(harness.query_all_by_label(REPOSITORY_URL).next().is_some());
    assert_eq!(REPOSITORY_URL, "https://github.com/thawkins/codemason");
}

#[test]
fn test_about_close_button_dismisses() {
    let (_runtime, mut harness) = opened("codemason-light");
    harness.get_by_label(CLOSE_ABOUT).click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    assert!(harness.query_all_by_label(ABOUT_TITLE).next().is_none());
}

#[test]
fn test_about_click_outside_dismisses() {
    let (_runtime, mut harness) = opened("codemason-dark");
    let outside = egui::pos2(10.0, 600.0);
    harness.event(egui::Event::PointerMoved(outside));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert!(!is_open(&harness.ctx));
}
