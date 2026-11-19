//! Tests for the word-processor frame (`codemason_gui::ribbon`,
//! `codemason_gui::page`): every ribbon command is one the window already
//! has, the frame is drawn on every theme, tabs switch and the ribbon
//! collapses, and a ribbon click runs its command.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::SLASH_COMMANDS;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::dock::panel_command;
use codemason_gui::keys::{command_key_input, default_keymap};
use codemason_gui::platform::menu::MenuAction;
use codemason_gui::ribbon::{HOME_TAB, QUICK_ACCESS, RibbonAction, RibbonItem, TABS};
use craft_keymap::{CommandRef, Platform};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// A running window in theme `theme`: its runtime and harness.
struct Rig {
    runtime: tokio::runtime::Runtime,
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
    Rig { runtime, harness }
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
            RibbonAction::Panel(pane) => {
                let command = panel_command(pane);
                assert!(
                    command.is_some_and(|c| command_key_input(&CommandRef::new(c)).is_some()),
                    "{}: {pane:?} has no keymap command",
                    item.label
                );
            }
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
fn test_ribbon_drawn_on_every_theme() {
    for theme in [
        "codemason-light",
        "codemason-dark",
        "document-light",
        "nord",
    ] {
        let rig = rig(theme);
        rig.harness.get_by_label("Home");
        rig.harness.get_by_label("Compact");
    }
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
    let after = rig.harness.state().app().show_log;
    assert_ne!(after, before);
    // The button shows the panel's state (the open panel's dock tab is
    // also labelled "Log", but is no toggle).
    let toggled: Vec<_> = rig
        .harness
        .query_all_by_label("Log")
        .filter_map(|node| node.accesskit_node().toggled())
        .collect();
    let want = if after {
        egui::accesskit::Toggled::True
    } else {
        egui::accesskit::Toggled::False
    };
    assert_eq!(toggled, vec![want]);
}

#[test]
fn test_ribbon_band_scrolls_when_the_window_is_too_narrow() {
    let mut rig = rig("document-light");
    let width = 520.0;
    rig.harness.set_size(egui::vec2(width, 900.0));
    rig.harness.run();
    let compact = rig.harness.get_by_label("Compact").rect();
    assert!(compact.right() > width, "Compact fits already: {compact:?}");
    // A plain mouse wheel over the band scrolls it sideways.
    rig.harness
        .hover_at(egui::pos2(width / 2.0, compact.center().y));
    rig.harness.run();
    for _ in 0..10 {
        rig.harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -100.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        });
        rig.harness.run();
    }
    let compact = rig.harness.get_by_label("Compact").rect();
    assert!(
        compact.right() <= width && compact.left() >= 0.0,
        "Compact is still out of view: {compact:?}"
    );
}

/// A fresh, empty scratch directory under the git-ignored `target/temp`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_ribbon")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn test_ribbon_state_file_round_trips_and_defaults() {
    use codemason_gui::ribbon::{RibbonState, load_ribbon, save_ribbon};

    let dir = scratch("round_trip");
    let path = dir.join("gui-ribbon.json");
    // Missing or invalid: expanded, on Home.
    assert_eq!(load_ribbon(&path), RibbonState::default());
    std::fs::write(&path, "not json").expect("write");
    assert_eq!(load_ribbon(&path), RibbonState::default());
    let collapsed = RibbonState {
        collapsed: true,
        ..RibbonState::default()
    };
    save_ribbon(&path, &collapsed).expect("save");
    assert_eq!(load_ribbon(&path), collapsed);
}

#[test]
fn test_ribbon_collapse_is_saved_and_restored() {
    let path = scratch("shell").join("gui-ribbon.json");
    let build = |path: &std::path::Path| {
        let shell = Shell::new(support::make_app()).with_ribbon_file(path.to_path_buf());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1400.0, 900.0))
            .allow_missing_glyphs()
            .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
        harness.run();
        harness
    };
    let mut first = build(&path);
    first.get_by_label("Collapse the ribbon").click();
    first.run();
    assert!(path.exists(), "collapsing saves the state");
    drop(first);
    // A new window starts collapsed.
    let second = build(&path);
    second.get_by_label("Expand the ribbon");
    assert!(second.query_by_label("Compact").is_none());
}

#[test]
fn test_ribbon_title_strip_shows_the_session_name() {
    let mut rig = rig("codemason-light");
    {
        let runtime = &rig.runtime;
        let app = rig.harness.state_mut().app_mut();
        runtime.block_on(app.execute_slash_command("/session new"));
        runtime.block_on(app.execute_slash_command("/name Release prep"));
    }
    rig.harness.run();
    rig.harness.get_by_label("codemason - Release prep");
}
