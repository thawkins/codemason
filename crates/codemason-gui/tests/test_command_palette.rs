//! Tests for the command palette dialog (`codemason_gui::dialogs::palette`):
//! Ctrl+P opens it with the search field focused, typing ranks the rows,
//! Enter runs the first row, Esc closes, and action rows show their key.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::dialogs::palette::{PALETTE_TITLE, SEARCH_LABEL, search_field_id};
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A window with the default Linux keymap and sources attached.
fn rig() -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.themes = ThemeCatalog::builtin();
        app.theme = ThemeSelection::System;
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

/// Press and release `key` with `modifiers`, then run.
fn key(harness: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    let input = harness.input_mut();
    for pressed in [true, false] {
        input.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        });
    }
    harness.run();
}

/// Open the palette with Ctrl+P.
fn open(harness: &mut Harness<'static, Shell>) {
    harness.run();
    key(
        harness,
        egui::Key::P,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    );
    harness.run();
}

fn shown(harness: &Harness<'static, Shell>) -> bool {
    harness
        .query_by_role_and_label(Role::Dialog, PALETTE_TITLE)
        .is_some()
}

#[test]
fn test_ctrl_p_opens_the_palette_with_the_search_focused() {
    let (_rt, mut harness) = rig();
    open(&mut harness);
    assert!(shown(&harness));
    harness.get_by_role_and_label(Role::SearchInput, SEARCH_LABEL);
    assert!(harness.ctx.memory(|m| m.has_focus(search_field_id())));
}

#[test]
fn test_typing_ranks_and_enter_runs_the_first_row() {
    let (_rt, mut harness) = rig();
    open(&mut harness);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("theme nord".to_string()));
    harness.run();
    let palette = harness.state().app().palette.clone().expect("open");
    assert_eq!(palette.query, "theme nord");
    assert_eq!(
        harness
            .state()
            .app()
            .palette_results()
            .first()
            .map(|e| e.id.clone()),
        Some("theme:nord".to_string())
    );
    key(&mut harness, egui::Key::Enter, egui::Modifiers::NONE);
    harness.run();
    assert!(!shown(&harness));
    assert_eq!(
        harness.state().app().theme,
        ThemeSelection::Named("nord".to_string())
    );
    // The query did not leak into the prompt.
    assert_eq!(harness.state().app().session.input, "");
}

#[test]
fn test_esc_closes_the_palette_without_change() {
    let (_rt, mut harness) = rig();
    open(&mut harness);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("theme nord".to_string()));
    harness.run();
    key(&mut harness, egui::Key::Escape, egui::Modifiers::NONE);
    harness.run();
    assert!(!shown(&harness));
    assert_eq!(harness.state().app().theme, ThemeSelection::System);
}

#[test]
fn test_action_rows_show_their_key() {
    let (_rt, mut harness) = rig();
    open(&mut harness);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("toggle log".to_string()));
    harness.run();
    harness.get_by_label("Toggle Log Panel");
    harness.get_by_label("Alt+L");
}

#[test]
fn test_which_key_popup_lists_the_leader_keys_after_a_moment() {
    use codemason_gui::dialogs::which_key::WHICH_KEY_TITLE;
    let (_rt, mut harness) = rig();
    harness.run();
    let ctrl = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
    for pressed in [true, false] {
        harness.input_mut().events.push(egui::Event::Key {
            key: egui::Key::G,
            physical_key: Some(egui::Key::G),
            pressed,
            repeat: false,
            modifiers: ctrl,
        });
    }
    // One frame: the leader waits, the popup is not up yet.
    harness.step();
    assert_eq!(harness.state().app().session.input, "");
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, WHICH_KEY_TITLE)
            .is_none()
    );
    // After 400 ms of waiting, and well before the 2 s timeout (egui shows
    // a new area from its second frame).
    // The harness advances its clock by a quarter second per step.
    for _ in 0..3 {
        harness.step();
    }
    harness.get_by_role_and_label(Role::Dialog, WHICH_KEY_TITLE);
    harness.get_by_label("Choose Model");
    harness.get_by_label("m");
}
