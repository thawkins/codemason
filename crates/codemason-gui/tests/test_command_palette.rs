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
    let _config = support::scratch_config_target();
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

#[test]
fn test_agent_picker_is_the_palette_scoped_to_agents() {
    use codemason_app_core::palette::PaletteScope;
    let (_rt, mut harness) = rig();
    harness
        .state_mut()
        .app_mut()
        .open_palette_scoped(PaletteScope::Agent);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, PaletteScope::Agent.title())
            .is_some(),
        "the agent picker is titled for its scope"
    );
    assert!(!shown(&harness), "not titled as the command palette");
    let rows = harness.state().app().palette_results();
    assert_ne!(rows.len(), 0, "agents are listed");
    assert!(rows.iter().all(|row| row.id.starts_with("agent:")));
}

#[test]
fn test_f2_cycles_the_recent_models() {
    let (_rt, mut harness) = rig();
    {
        let app = harness.state_mut().app_mut();
        app.storage
            .set_provider_auth("openai", "sk-test")
            .expect("store key");
        let models: Vec<codemason_agent::provider::ModelInfo> = ["gpt-4o", "o3"]
            .iter()
            .map(|id| codemason_agent::provider::ModelInfo {
                id: (*id).to_string(),
                provider_id: "openai".to_string(),
                name: (*id).to_string(),
                cost: codemason_config::Cost {
                    input: 0.0,
                    output: 0.0,
                },
                capabilities: codemason_config::Capabilities {
                    reasoning: false,
                    streaming: true,
                    vision: false,
                    tool_use: true,
                    thinking_levels: Vec::new(),
                },
                context_window: 128_000,
                max_output: None,
                request_multiplier: None,
                thinking_config: None,
            })
            .collect();
        app.storage
            .set_discovered_models("openai", &serde_json::to_string(&models).expect("json"))
            .expect("seed models");
        app.storage
            .set_setting(
                codemason_app_core::app::RECENT_MODELS_KEY,
                r#"[{"provider_id":"openai","provider_name":"OpenAI","id":"gpt-4o",
                    "name":"GPT-4o","context_window":128000},
                   {"provider_id":"openai","provider_name":"OpenAI","id":"o3",
                    "name":"o3","context_window":200000}]"#,
            )
            .expect("seed recent models");
        app.session.selected_model = Some("openai/gpt-4o".to_string());
    }
    harness.run();
    key(&mut harness, egui::Key::F2, egui::Modifiers::NONE);
    harness.run();
    assert_eq!(
        harness.state().app().session.selected_model.as_deref(),
        Some("openai/o3")
    );
}

#[test]
fn test_ctrl_p_does_not_open_the_palette_over_a_modal_dialog() {
    let (_rt, mut harness) = rig();
    harness.state_mut().app_mut().show_shortcuts = true;
    open(&mut harness);
    assert!(
        harness.state().app().palette.is_none(),
        "the open dialog keeps the keyboard"
    );
}

#[test]
fn test_enter_in_the_same_frame_as_typing_runs_the_new_query() {
    let (_rt, mut harness) = rig();
    open(&mut harness);
    let input = harness.input_mut();
    input
        .events
        .push(egui::Event::Text("theme nord".to_string()));
    for pressed in [true, false] {
        input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: Some(egui::Key::Enter),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    harness.run();
    assert!(!shown(&harness));
    assert_eq!(
        harness.state().app().theme,
        ThemeSelection::Named("nord".to_string()),
        "Enter ran the row of the query typed with it"
    );
}
