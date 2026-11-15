//! Tests for the provider/model setup dialog chrome (`/provider`, `/model`).
//!
//! The setup dialogs share one composition: a slate accent frame, a heading strip,
//! a pinned reversed-video column band, a cursor-filled zebra-striped list, and a
//! key-hint footer. These tests pin the parts that make the dialogs easier to read
//! (column alignment, the configured-provider count, the feature legend, the
//! explicit scroll legend) without giving up any of the behaviour the pickers had.

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::{ModelPickerEntry, ProviderSetupStep};
use codemason_types::{ThinkingConfig, ThinkingLevel};

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, name: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: name.to_string(),
        context_window: 200_000,
        max_output: Some(64_000),
        cost_input: 3.0,
        cost_output: 15.0,
        reasoning: true,
        vision: true,
        tool_use: true,
        thinking_levels: vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ],
        thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        cost_tier: "Premium".to_string(),
        cost_multiplier: "1x".to_string(),
    }
}

#[tokio::test]
async fn test_thinking_selector_selection_still_persists_a_level() {
    // The restyle must not touch the state machine: Enter on the highlighted level
    // still records it and advances to the confirmation card.
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: entry("claude-sonnet-4", "Claude Sonnet 4"),
        selected: 2,
    });
    crate::term_events::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert_eq!(
        app.session.selected_thinking_level,
        Some(ThinkingLevel::Low)
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::Done { .. })
    ));
}
