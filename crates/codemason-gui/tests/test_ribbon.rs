//! Tests for the word-processor frame (`codemason_gui::ribbon`,
//! `codemason_gui::page`): every ribbon command is one the window already
//! has, the frame is drawn only for themes with their own Material roles,
//! tabs switch and the ribbon collapses, a ribbon click runs its command,
//! and the page column's width.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::SLASH_COMMANDS;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::keys::{command_key_input, default_keymap};
use codemason_gui::page::{DESK_MARGIN, PAGE_WIDTH, page_width};
use codemason_gui::platform::menu::MenuAction;
use codemason_gui::ribbon::{HOME_TAB, QUICK_ACCESS, RibbonAction, RibbonItem, TABS};
use craft_keymap::{CommandRef, Platform};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window in theme `theme`: its runtime and harness.
struct Rig {
    _runtime: tokio::runtime::Runtime,
    harness: Harness<'static, Shell>,
}

/// A shell with attached sources (so commands run) in theme `theme`.
fn rig(theme: &str) -> Rig {
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
    Rig {
        _runtime: runtime,
        harness,
    }
}

/// Every ribbon and quick access item.
fn items() -> impl Iterator<Item = &'static RibbonItem> {
    TABS.iter()
        .flat_map(|tab| tab.groups.iter())
        .flat_map(|group| group.items.iter())
        .chain(QUICK_ACCESS.iter())
}

#[test]
fn test_ribbon_commands_are_ones_the_window_has() {
    for item in items() {
        match item.action {
            RibbonAction::Menu(MenuAction::Slash(line)) => {
                let trigger = line
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or_default();
                assert!(
                    SLASH_COMMANDS.iter().any(|c| c.trigger == trigger),
                    "{}: /{trigger} is not a slash command",
                    item.label
                );
            }
            RibbonAction::Panel(command) => assert!(
                command_key_input(&CommandRef::new(command)).is_some(),
                "{}: {command} is not a keymap command",
                item.label
            ),
            RibbonAction::Menu(MenuAction::Key(_)) | RibbonAction::Files(_) => {}
        }
    }
    assert_eq!(TABS[HOME_TAB].label, "Home");
    for tab in TABS {
        let mut labels: Vec<_> = tab
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.label))
            .collect();
        let count = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), count, "{} repeats a label", tab.label);
    }
}

#[test]
fn test_ribbon_page_width_is_capped_and_never_negative() {
    assert!((page_width(2000.0) - PAGE_WIDTH).abs() < f32::EPSILON);
    assert!((page_width(500.0) - DESK_MARGIN.mul_add(-2.0, 500.0)).abs() < f32::EPSILON);
    assert!(page_width(10.0).abs() < f32::EPSILON);
}

#[test]
fn test_ribbon_drawn_only_for_document_themes() {
    let legacy = rig("codemason-light");
    assert!(legacy.harness.query_by_label("Home").is_none());
    assert!(legacy.harness.query_by_label("Compact").is_none());
    let document = rig("document-light");
    document.harness.get_by_label("Home");
    document.harness.get_by_label("Compact");
}

#[test]
fn test_ribbon_tabs_switch_and_the_ribbon_collapses() {
    let mut rig = rig("document-light");
    assert!(rig.harness.query_by_label("Theme").is_none());
    rig.harness.get_by_label("View").click();
    rig.harness.run();
    rig.harness.get_by_label("Theme");
    rig.harness.get_by_label("Collapse the ribbon").click();
    rig.harness.run();
    assert!(rig.harness.query_by_label("Theme").is_none());
    rig.harness.get_by_label("Expand the ribbon");
    // A tab click opens a collapsed ribbon on that tab.
    rig.harness.get_by_label("Review").click();
    rig.harness.run();
    rig.harness.get_by_label("Stats");
}

#[test]
fn test_ribbon_panel_button_toggles_the_panel() {
    let mut rig = rig("document-light");
    let before = rig.harness.state().app().show_log;
    rig.harness.get_by_label("View").click();
    rig.harness.run();
    rig.harness.get_by_label("Log").click();
    rig.harness.run();
    assert_ne!(rig.harness.state().app().show_log, before);
}
