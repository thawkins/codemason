//! The Model Router setup dialog (`ProviderSetupStep::SetupRouter`) drawn
//! with Material components: provider filter chips, tier segmented
//! buttons, a model list with move/remove icon buttons, and Save/Cancel
//! actions. Each test drives the whole [`Shell`] frame, so a click goes
//! through app-core's router key handling as a key press would.

#[path = "support/mod.rs"]
mod support;

use std::collections::HashMap;

use codemason_app_core::app::{App, ConfiguredProvider, ProviderSetupStep, ProviderSource};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::dialogs::router::{
    MODELS_LABEL, ROUTER_TITLE, SAVE, add_label, move_down_label, move_up_label, remove_label,
};
use codemason_gui::keys::default_keymap;
use codemason_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The router step with Anthropic in the cluster and OpenAI available, and
/// `simple` as the SIMPLE tier's models.
fn router_step(simple: &[(&str, &str)]) -> ProviderSetupStep {
    let providers = vec![
        ConfiguredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            source: ProviderSource::Database,
        },
        ConfiguredProvider {
            id: "openai".to_string(),
            name: "OpenAI (GPT)".to_string(),
            source: ProviderSource::Database,
        },
    ];
    let mut tiers = HashMap::new();
    tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: simple
                .iter()
                .map(|(provider, model)| TierEntry {
                    provider_id: (*provider).to_string(),
                    model: (*model).to_string(),
                })
                .collect(),
            timeout_ms: None,
        },
    );
    ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: RouterConfig {
            tiers,
            ..RouterConfig::default()
        },
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    }
}

/// Store one discovered Anthropic model, so the add action has a model to
/// pick from.
fn seed_models(app: &App) {
    let model = codemason_agent::provider::ModelInfo {
        id: "claude-a".to_string(),
        provider_id: "anthropic".to_string(),
        name: "Claude A".to_string(),
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
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    };
    let json = serde_json::to_string(&vec![model]).expect("serialize models");
    app.storage
        .set_discovered_models("anthropic", &json)
        .expect("seed discovered models");
}

/// A shell showing `step`, and the runtime driving it.
fn rig(step: ProviderSetupStep) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        seed_models(&app);
        app.provider_setup = Some(step);
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    (runtime, harness)
}

/// The router step's fields the tests check.
fn router(app: &App) -> (&[String], Tier, usize, &RouterConfig, Option<&str>) {
    match app.provider_setup.as_ref() {
        Some(ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            active_bucket,
            active_bucket_index,
            draft_config,
            error,
            ..
        }) => (
            selected_provider_ids,
            *active_bucket,
            *active_bucket_index,
            draft_config,
            error.as_deref(),
        ),
        other => panic!("expected the router step, got {other:?}"),
    }
}

/// The SIMPLE tier's `provider/model` entries.
fn simple_models(app: &App) -> Vec<String> {
    router(app)
        .3
        .tiers
        .get(&Tier::Simple.to_string())
        .map(|t| {
            t.models
                .iter()
                .map(|e| format!("{}/{}", e.provider_id, e.model))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn test_router_dialog_shows_material_parts() {
    let (_rt, harness) = rig(router_step(&[("anthropic", "claude-a")]));
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, ROUTER_TITLE)
            .is_some()
    );
    // The providers are filter chips, not "[x]" text lines.
    assert!(harness.query_by_label("Anthropic (Claude)").is_some());
    assert!(harness.query_by_label("OpenAI (GPT)").is_some());
    assert!(harness.query_by_label_contains("[x]").is_none());
    // The tiers are segmented buttons, labelled with their model counts.
    for tier in ["Simple (1)", "Medium (0)", "Complex (0)", "Reasoning (0)"] {
        assert!(
            harness
                .query_by_role_and_label(Role::RadioButton, tier)
                .is_some(),
            "tier segment {tier}"
        );
    }
    // The tier's models are a labelled list.
    assert!(
        harness
            .query_by_role_and_label(Role::List, MODELS_LABEL)
            .is_some()
    );
    assert!(harness.query_by_label("claude-a").is_some());
}

#[test]
fn test_router_dialog_provider_chip_toggles_cluster() {
    let (_rt, mut harness) = rig(router_step(&[]));
    harness.get_by_label("OpenAI (GPT)").click();
    harness.run();
    let ids = router(harness.state().app()).0.to_vec();
    assert!(ids.contains(&"openai".to_string()), "ids: {ids:?}");
    // Off again, through the same chip.
    harness.get_by_label("OpenAI (GPT)").click();
    harness.run();
    let ids = router(harness.state().app()).0.to_vec();
    assert!(!ids.contains(&"openai".to_string()), "ids: {ids:?}");
}

#[test]
fn test_router_dialog_tier_segment_switches_bucket() {
    let (_rt, mut harness) = rig(router_step(&[]));
    harness
        .get_by_role_and_label(Role::RadioButton, "Complex (0)")
        .click();
    harness.run();
    assert_eq!(router(harness.state().app()).1, Tier::Complex);
}

#[test]
fn test_router_dialog_row_actions_reorder_and_remove() {
    let (_rt, mut harness) = rig(router_step(&[("anthropic", "a1"), ("anthropic", "a2")]));
    harness.get_by_label(&move_down_label("a1")).click();
    harness.run();
    assert_eq!(
        simple_models(harness.state().app()),
        vec!["anthropic/a2", "anthropic/a1"]
    );
    assert_eq!(router(harness.state().app()).2, 1);

    harness.get_by_label(&move_up_label("a1")).click();
    harness.run();
    assert_eq!(
        simple_models(harness.state().app()),
        vec!["anthropic/a1", "anthropic/a2"]
    );

    harness.get_by_label(&remove_label("a2")).click();
    harness.run();
    assert_eq!(simple_models(harness.state().app()), vec!["anthropic/a1"]);
}

#[test]
fn test_router_dialog_add_opens_model_picker_for_tier() {
    let (_rt, mut harness) = rig(router_step(&[]));
    // Only providers in the cluster offer an add action.
    assert!(harness.query_by_label(&add_label("OpenAI (GPT)")).is_none());
    harness
        .get_by_label(&add_label("Anthropic (Claude)"))
        .click();
    harness.run();
    match harness.state().app().provider_setup.as_ref() {
        Some(ProviderSetupStep::SelectRouterModel {
            provider_id,
            target_tier,
            ..
        }) => {
            assert_eq!(provider_id, "anthropic");
            assert_eq!(*target_tier, Tier::Simple);
        }
        other => panic!("expected the router model picker, got {other:?}"),
    }
}

#[test]
fn test_router_dialog_save_requires_a_model_then_confirms() {
    let (_rt, mut harness) = rig(router_step(&[]));
    harness.get_by_label(SAVE).click();
    harness.run();
    assert_eq!(
        router(harness.state().app()).4,
        Some("At least one tier must contain a model")
    );
    assert!(
        harness
            .query_by_label_contains("At least one tier")
            .is_some()
    );
    assert!(harness.state().app().pending_router_save.is_none());

    let (_rt, mut harness) = rig(router_step(&[("anthropic", "a1")]));
    harness.get_by_label(SAVE).click();
    harness.run();
    assert!(harness.state().app().pending_router_save.is_some());
}
