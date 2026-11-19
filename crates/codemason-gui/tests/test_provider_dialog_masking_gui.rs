//! MS-03: the rendered provider-setup dialog masks credentials.
//!
//! SEC-codemason-tui-002 (SECTASKS T-045): the provider dialog once showed
//! the API key and the GitLab personal access token verbatim. These tests
//! draw the real dialog (`codemason_gui::dialogs::provider::show`) in an
//! egui_kittest harness and query its accessibility tree: the masked value
//! (`codemason_app_core::text_utils::mask_secret`) is shown and no node
//! carries the raw secret.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::{App, ProviderSetupStep};
use codemason_app_core::input_field::InputField;
use codemason_app_core::text_utils::mask_secret;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A long API key whose middle must never be drawn.
const API_KEY: &str = "sk-ant-api03-SECRETMIDDLEabcdefghijklmnop";

/// A GitLab PAT whose body must never be drawn.
const GITLAB_PAT: &str = "glpat-SECRETBODYabcdefghijklmnop";

/// Draw the provider dialog for `step` and run the harness to a still frame.
fn harness_for(step: ProviderSetupStep) -> Harness<'static, App> {
    let mut app = support::make_app();
    app.provider_setup = Some(step);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, app: &mut App| {
                codemason_gui::dialogs::provider::show(ui.ctx(), app);
            },
            app,
        );
    harness.run();
    harness
}

/// Assert the masked `secret` is drawn and no node holds `secret` itself or
/// its hidden middle `body`.
fn assert_masked(harness: &Harness<'static, App>, secret: &str, body: &str) {
    let masked = mask_secret(secret);
    assert_ne!(masked, secret, "the rule must mask the secret");
    // The field holds the masked text as its value; no field's value is
    // the secret.
    let values: Vec<String> = harness
        .query_all_by_role(Role::TextInput)
        .filter_map(|node| node.value())
        .collect();
    assert!(
        values.contains(&masked),
        "the dialog shows the masked value {masked}, fields: {values:?}"
    );
    assert!(
        values.iter().all(|v| !v.contains(body)),
        "the secret's middle must never be a field's value"
    );
    assert!(
        harness.query_by_label_contains(secret).is_none(),
        "the raw secret must never be drawn"
    );
    assert!(
        harness.query_by_label_contains(body).is_none(),
        "the secret's middle must never be drawn"
    );
}

#[test]
fn test_provider_dialog_masks_the_api_key() {
    let harness = harness_for(ProviderSetupStep::EnterKey {
        provider_id: "openai".to_string(),
        provider_name: "OpenAI (GPT)".to_string(),
        key_field: InputField::with_text(API_KEY),
        endpoint_field: InputField::new(),
        active_field: 0,
        error: None,
    });
    assert_masked(&harness, API_KEY, "SECRETMIDDLE");
}

#[test]
fn test_provider_dialog_masks_the_gitlab_pat() {
    let harness = harness_for(ProviderSetupStep::GitLabSetup {
        url_input: "https://gitlab.example.com".to_string(),
        url_cursor: 0,
        token_input: GITLAB_PAT.to_string(),
        token_cursor: GITLAB_PAT.len(),
        active_field: 1,
        error: None,
    });
    assert_masked(&harness, GITLAB_PAT, "SECRETBODY");
}
