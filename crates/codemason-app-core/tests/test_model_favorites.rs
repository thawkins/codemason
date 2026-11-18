//! Tests for favourite and recent models (`app/model_favorites.rs`): Ctrl+F
//! in the model picker, favourites sorting first, the recent list and F2's
//! `model.cycle_recent` action.

use codemason_app_core::app::{
    App, ModelPickerEntry, ProviderSetupStep, RECENT_MODELS_MAX, RecentModel,
};
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};

mod support;

fn entry(id: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: format!("Model {id}"),
        context_window: 128_000,
        max_output: None,
        cost_input: 0.0,
        cost_output: 0.0,
        reasoning: false,
        vision: false,
        tool_use: true,
        thinking_levels: Vec::new(),
        thinking_config: None,
        cost_tier: "Free".to_string(),
        cost_multiplier: "0x".to_string(),
    }
}

fn open_picker(app: &mut App, ids: &[&str], selected: usize) {
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: ids.iter().map(|id| entry(id)).collect(),
        selected,
    });
}

fn picker(app: &App) -> (Vec<String>, usize) {
    match app.provider_setup.as_ref() {
        Some(ProviderSetupStep::SelectModel {
            models, selected, ..
        }) => (models.iter().map(|m| m.id.clone()).collect(), *selected),
        other => panic!("model picker closed: {other:?}"),
    }
}

async fn ctrl_f(app: &mut App) {
    let _ = app
        .handle_key(KeyInput {
            key: Key::Char('f'),
            mods: Modifiers::CTRL,
        })
        .await;
}

#[tokio::test]
async fn test_model_favorites_ctrl_f_toggles_and_sorts_first() {
    let mut app = support::make_app();
    open_picker(&mut app, &["a", "b", "c"], 2);
    ctrl_f(&mut app).await;
    assert_eq!(app.favorite_models(), vec!["anthropic/c".to_string()]);
    let (ids, selected) = picker(&app);
    assert_eq!(ids, ["c", "a", "b"], "the favourite moves first");
    assert_eq!(selected, 0, "the highlight follows the model");

    ctrl_f(&mut app).await;
    assert!(
        app.favorite_models().is_empty(),
        "a second Ctrl+F unmarks it"
    );
}

#[test]
fn test_model_favorites_toggle_persists_in_storage() {
    let mut app = support::make_app();
    assert!(app.toggle_favorite_model("openai", "gpt"));
    assert!(app.toggle_favorite_model("anthropic", "opus"));
    let saved = app
        .storage
        .get_setting(codemason_app_core::app::FAVORITE_MODELS_KEY)
        .expect("read")
        .expect("saved");
    assert_eq!(saved, r#"["openai/gpt","anthropic/opus"]"#);
    assert!(!app.toggle_favorite_model("openai", "gpt"));
    assert_eq!(app.favorite_models(), vec!["anthropic/opus".to_string()]);
}

#[tokio::test]
async fn test_model_favorites_enter_remembers_recent_models() {
    let mut app = support::make_app();
    for id in ["a", "b", "c", "d", "e", "f", "b"] {
        open_picker(&mut app, &[id], 0);
        let _ = app
            .handle_key(KeyInput {
                key: Key::Enter,
                mods: Modifiers::default(),
            })
            .await;
    }
    let recent: Vec<String> = app.recent_models().iter().map(RecentModel::key).collect();
    assert_eq!(recent.len(), RECENT_MODELS_MAX);
    assert_eq!(
        recent,
        [
            "anthropic/b",
            "anthropic/f",
            "anthropic/e",
            "anthropic/d",
            "anthropic/c"
        ],
        "newest first, distinct, capped"
    );
}

#[tokio::test]
async fn test_model_favorites_cycle_recent_walks_the_list() {
    let mut app = support::make_app();
    for id in ["a", "b", "c"] {
        open_picker(&mut app, &[id], 0);
        let _ = app
            .handle_key(KeyInput {
                key: Key::Enter,
                mods: Modifiers::default(),
            })
            .await;
    }
    assert_eq!(app.session.selected_model.as_deref(), Some("anthropic/c"));
    // Recent order: c, b, a. F2 walks it without reordering.
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model.as_deref(), Some("anthropic/b"));
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model.as_deref(), Some("anthropic/a"));
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model.as_deref(), Some("anthropic/c"));
}

#[test]
fn test_model_favorites_cycle_recent_with_nothing_to_switch_to() {
    let mut app = support::make_app();
    assert!(!app.cycle_recent_model());
    assert_eq!(app.status, "model: no other recent model");
}
