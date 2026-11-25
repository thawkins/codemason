//! Tests for the Swarm dialog (`codemason_gui::dialogs::swarm`): the Agent
//! tab's Swarm sits after Loop and opens it, Ok runs `/swarm <prompt>`, a
//! blank prompt cannot be sent, and Cancel or Esc closes it running nothing.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::dialogs::swarm::{
    CANCEL_SWARM, OK_SWARM, PROMPT_LABEL, SWARM_TITLE, is_open, swarm_command,
};
use codemason_gui::keys::default_keymap;
use codemason_gui::platform::menu::MenuAction;
use codemason_gui::ribbon::{RibbonAction, TABS};
use craft_keymap::Platform;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window with the Swarm dialog opened from the Agent tab. The
/// runtime is returned so it outlives the harness.
fn opened() -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        support::make_app()
    };
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
    harness.get_by_label("Agent").click();
    harness.run();
    harness.get_by_label("Swarm").click();
    harness.run();
    (runtime, harness)
}

#[test]
fn test_swarm_follows_loop_in_the_agent_tab() {
    let agent = TABS.iter().find(|t| t.label == "Agent").expect("Agent tab");
    let items: Vec<_> = agent.groups.iter().flat_map(|g| g.items.iter()).collect();
    let labels: Vec<_> = items.iter().map(|i| i.label).collect();
    let at = labels.iter().position(|l| *l == "Loop").expect("Loop");
    assert_eq!(labels[at + 1], "Swarm");
    assert_eq!(items[at + 1].action, RibbonAction::Menu(MenuAction::Swarm));
}

#[test]
fn test_swarm_command_trims_and_rejects_blank() {
    assert_eq!(
        swarm_command("  add tests  ").as_deref(),
        Some("/swarm add tests")
    );
    assert_eq!(swarm_command(""), None);
    assert_eq!(swarm_command(" \t "), None);
}

#[test]
fn test_swarm_ribbon_opens_dialog_with_focused_field() {
    let (_runtime, harness) = opened();
    assert!(is_open(&harness.ctx));
    assert!(harness.query_all_by_label(SWARM_TITLE).next().is_some());
    assert!(harness.get_by_label(PROMPT_LABEL).is_focused());
}

#[test]
fn test_swarm_ok_runs_swarm_with_the_prompt() {
    let (_runtime, mut harness) = opened();
    harness
        .get_by_label(PROMPT_LABEL)
        .type_text("write the docs");
    harness.run();
    harness.get_by_label(OK_SWARM).click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    let swarm = harness.state().app().session.swarm_state.as_ref();
    assert!(swarm.is_some(), "/swarm did not start a swarm");
}

#[test]
fn test_swarm_ok_does_nothing_on_a_blank_prompt() {
    let (_runtime, mut harness) = opened();
    harness.get_by_label(OK_SWARM).click();
    harness.run();
    assert!(is_open(&harness.ctx));
    assert!(harness.state().app().session.swarm_state.is_none());
}

#[test]
fn test_swarm_cancel_closes_without_running() {
    let (_runtime, mut harness) = opened();
    harness
        .get_by_label(PROMPT_LABEL)
        .type_text("write the docs");
    harness.run();
    harness.get_by_label(CANCEL_SWARM).click();
    harness.run();
    assert!(!is_open(&harness.ctx));
    assert!(harness.state().app().session.swarm_state.is_none());
}

#[test]
fn test_swarm_esc_closes_without_running() {
    let (_runtime, mut harness) = opened();
    harness
        .get_by_label(PROMPT_LABEL)
        .type_text("write the docs");
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    assert!(!is_open(&harness.ctx));
    assert!(harness.state().app().session.swarm_state.is_none());
}
