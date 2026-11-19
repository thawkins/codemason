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

/// Choose each of `models` in the picker, as Enter does.
async fn choose(app: &mut App, models: &[ModelPickerEntry]) {
    for model in models {
        app.provider_setup = Some(ProviderSetupStep::SelectModel {
            provider_id: model.provider_id.clone(),
            provider_name: "Anthropic".to_string(),
            models: vec![model.clone()],
            selected: 0,
        });
        // Enter picks the model; a model with thinking levels asks for one
        // first, and a second Enter takes the default.
        for _ in 0..2 {
            if !matches!(
                app.provider_setup,
                Some(
                    ProviderSetupStep::SelectModel { .. }
                        | ProviderSetupStep::SelectThinkingLevel { .. }
                )
            ) {
                break;
            }
            let _ = app
                .handle_key(KeyInput {
                    key: Key::Enter,
                    mods: Modifiers::default(),
                })
                .await;
        }
    }
}

/// Three models anthropic lists (seeded as discovered models), with
/// anthropic configured.
fn anthropic_models(app: &App) -> Vec<ModelPickerEntry> {
    app.storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store key");
    let models: Vec<codemason_agent::provider::ModelInfo> = ["m-one", "m-two", "m-three"]
        .iter()
        .map(|id| codemason_agent::provider::ModelInfo {
            id: (*id).to_string(),
            provider_id: "anthropic".to_string(),
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
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        })
        .collect();
    app.storage
        .set_discovered_models("anthropic", &serde_json::to_string(&models).expect("json"))
        .expect("seed models");
    let entries: Vec<ModelPickerEntry> = ["m-one", "m-two", "m-three"]
        .iter()
        .map(|id| {
            app.models_for_provider("anthropic")
                .into_iter()
                .find(|m| m.id == *id)
                .expect("listed")
        })
        .collect();
    entries
}

#[tokio::test]
async fn test_model_favorites_cycle_recent_walks_the_list() {
    let mut app = support::make_app();
    let models = anthropic_models(&app);
    let key = |m: &ModelPickerEntry| Some(format!("anthropic/{}", m.id));
    choose(&mut app, &models).await;
    assert_eq!(app.session.selected_model, key(&models[2]));
    // Recent order: 2, 1, 0. F2 walks it without reordering.
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model, key(&models[1]));
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model, key(&models[0]));
    assert!(app.run_action("model.cycle_recent").await);
    assert_eq!(app.session.selected_model, key(&models[2]));
}

#[tokio::test]
async fn test_model_favorites_cycle_recent_skips_unusable_models() {
    let mut app = support::make_app();
    let models = anthropic_models(&app);
    choose(&mut app, &models[..1]).await;
    // Newest first: a model anthropic no longer lists, then one of a
    // provider with no credentials, then the usable first model.
    let mut recent = app.recent_models();
    let gone = RecentModel {
        id: "retired-model".to_string(),
        name: "Retired".to_string(),
        ..recent[0].clone()
    };
    let unconfigured = RecentModel {
        provider_id: "no-such-provider".to_string(),
        ..recent[0].clone()
    };
    recent.splice(0..0, [gone, unconfigured]);
    app.storage
        .set_setting(
            codemason_app_core::app::RECENT_MODELS_KEY,
            &serde_json::to_string(&recent).expect("json"),
        )
        .expect("save");
    app.session.selected_model = None;
    assert!(app.cycle_recent_model());
    assert_eq!(
        app.session.selected_model,
        Some(format!("anthropic/{}", models[0].id)),
        "both unusable entries are skipped"
    );
}

#[test]
fn test_model_favorites_cycle_recent_with_nothing_to_switch_to() {
    let mut app = support::make_app();
    assert!(!app.cycle_recent_model());
    assert_eq!(app.status, "model: no other recent model");
}
