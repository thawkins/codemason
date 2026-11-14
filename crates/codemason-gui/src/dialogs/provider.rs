//! The provider setup dialog (`/provider`, `/model`, `/agent`, `/gitlab`,
//! `/telemetry` and the router setup), over `App::provider_setup`.
//!
//! app-core runs the whole flow as a key-driven state machine
//! (`handle_provider_setup_key` in `input.rs`): Up and Down move a list's
//! highlight, Enter confirms a step (saving keys, settings and the chosen
//! provider), Esc cancels and typed characters fill the step's text fields.
//! This dialog draws the current step and drives the same machine:
//!
//! - a step that picks from a list draws a `craft_ui::ComboBox`; choosing an
//!   entry moves the step's highlight (what Up and Down move), and Continue
//!   sends Enter;
//! - a step with text fields shows them as the App holds them (secrets
//!   masked as the TUI masks them), with the active field marked; typing
//!   reaches them as key presses through the router, as in the TUI;
//! - Cancel sends Esc, and every other button sends the key the TUI names
//!   for it.
//!
//! The model lists of the flow are drawn by [`super::model_picker`].
//!
//! Dependencies: `codemason-app-core`, `codemason-config` (telemetry
//! protocol), `codemason-types` (thinking levels, secret masking), `craft-ui`
//! (`ComboBox`) and `egui`.

use codemason_app_core::app::{App, DeviceFlowKind, PROVIDER_LIST, ProviderSetupStep};
use codemason_app_core::input_event::{Key, KeyInput};
use codemason_config::OtelProtocol;
use codemason_types::ThinkingLevel;

use super::{key_buttons, modal, plain};

/// The button that confirms the current step (Enter).
pub const CONTINUE: &str = "Continue";

/// The button that cancels the flow (Esc).
pub const CANCEL: &str = "Cancel";

/// Label of the provider list's combo box.
pub const PROVIDER_CHOICE_LABEL: &str = "Provider";

/// Title of the provider list step (`/provider`, first-time setup).
pub const SELECT_PROVIDER_TITLE: &str = "Select provider";

/// A step's list: the entries and the highlighted one.
struct Choice {
    label: &'static str,
    options: Vec<String>,
    selected: usize,
    empty: &'static str,
}

/// One read-only text field of a step.
struct Field {
    label: &'static str,
    value: String,
    active: bool,
}

/// What the dialog draws for one step.
struct StepView {
    title: String,
    lines: Vec<String>,
    choice: Option<Choice>,
    fields: Vec<Field>,
    error: Option<String>,
    buttons: Vec<(&'static str, KeyInput)>,
    hint: &'static str,
}

impl StepView {
    /// A view titled `title` with Continue and Cancel buttons.
    fn new(title: impl Into<String>, hint: &'static str) -> Self {
        Self {
            title: title.into(),
            lines: Vec::new(),
            choice: None,
            fields: Vec::new(),
            error: None,
            buttons: vec![(CONTINUE, plain(Key::Enter)), (CANCEL, plain(Key::Esc))],
            hint,
        }
    }

    /// Set the step's list.
    fn choice(
        mut self,
        label: &'static str,
        options: Vec<String>,
        selected: usize,
        empty: &'static str,
    ) -> Self {
        self.choice = Some(Choice {
            label,
            options,
            selected,
            empty,
        });
        self
    }
}

/// Mask a secret as the TUI does (`mask_secret`: the first and last four
/// characters of a long key, the rest hidden).
fn mask(text: &str) -> String {
    codemason_types::sanitize::mask_edges(text, 4, 12)
}

/// The TUI's name for a thinking level.
fn thinking_label(level: ThinkingLevel) -> &'static str {
    match level {
        ThinkingLevel::Auto => "Auto",
        ThinkingLevel::Off => "Off",
        ThinkingLevel::Low => "Low",
        ThinkingLevel::Medium => "Medium",
        ThinkingLevel::High => "High",
    }
}

/// The view of the provider list steps (`SelectProvider`, `ResetProvider`).
fn provider_names() -> Vec<String> {
    PROVIDER_LIST
        .iter()
        .map(|(_, name)| (*name).to_string())
        .collect()
}

/// Build the view of `step`.
#[allow(clippy::too_many_lines)] // one arm per step, each short
fn view(step: &ProviderSetupStep) -> StepView {
    const LIST_HINT: &str = "Up/Down move  Enter continue  Esc cancel";
    match step {
        ProviderSetupStep::SelectProvider { selected, .. } => {
            StepView::new(SELECT_PROVIDER_TITLE, LIST_HINT).choice(
                PROVIDER_CHOICE_LABEL,
                provider_names(),
                *selected,
                "No providers",
            )
        }
        ProviderSetupStep::ResetProvider { selected } => {
            let mut view = StepView::new("Reset provider", "Up/Down move  Enter reset  Esc cancel")
                .choice(
                    PROVIDER_CHOICE_LABEL,
                    provider_names(),
                    *selected,
                    "No providers",
                );
            view.lines
                .push("Removes the stored credentials of the chosen provider.".to_string());
            view
        }
        ProviderSetupStep::SelectConfiguredProvider {
            providers,
            selected,
        } => StepView::new("Switch provider", LIST_HINT).choice(
            PROVIDER_CHOICE_LABEL,
            providers.iter().map(|p| p.name.clone()).collect(),
            *selected,
            "No configured providers",
        ),
        ProviderSetupStep::ShowProviderConfig {
            providers,
            selected,
        } => StepView::new("Provider configuration", LIST_HINT).choice(
            PROVIDER_CHOICE_LABEL,
            providers.iter().map(|p| p.name.clone()).collect(),
            *selected,
            "No configured providers",
        ),
        ProviderSetupStep::SelectAgent { agents, selected } => {
            StepView::new("Select agent", LIST_HINT).choice(
                "Agent",
                agents
                    .iter()
                    .map(|(name, description, _)| format!("{name} - {description}"))
                    .collect(),
                *selected,
                "No agents",
            )
        }
        ProviderSetupStep::SelectThinkingLevel {
            provider_name,
            model,
            selected,
            ..
        } => {
            let mut view = StepView::new("Thinking level", LIST_HINT).choice(
                "Thinking level",
                model
                    .thinking_levels
                    .iter()
                    .map(|level| thinking_label(*level).to_string())
                    .collect(),
                *selected,
                "No thinking levels",
            );
            view.lines.push(format!("{provider_name} / {}", model.name));
            view
        }
        ProviderSetupStep::SelectAzureResource {
            entries,
            selected,
            error,
        } => {
            let mut view = StepView::new("Azure resource", LIST_HINT).choice(
                "Resource",
                entries
                    .iter()
                    .map(|e| format!("{} ({})", e.name, e.endpoint))
                    .collect(),
                *selected,
                "No Azure resources",
            );
            view.error = error.clone();
            view
        }
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            key_field,
            endpoint_field,
            active_field,
            error,
        } => {
            let mut view = StepView::new(
                format!("Enter API key - {provider_name}"),
                "Enter confirm  Esc cancel",
            );
            view.fields.push(Field {
                label: "API key",
                value: mask(key_field.text()),
                active: *active_field == 0,
            });
            if provider_id == "generic_openai" || provider_id == "azure_foundry" {
                view.fields.push(Field {
                    label: "Endpoint URL (optional)",
                    value: endpoint_field.text().to_string(),
                    active: *active_field == 1,
                });
                view.hint = "Tab switch fields  Enter confirm  Esc cancel";
            }
            view.error = error.clone();
            view
        }
        ProviderSetupStep::DeviceFlowPending {
            flow,
            user_code,
            verification_uri,
        } => {
            let title = match flow {
                DeviceFlowKind::Copilot => "Copilot sign in",
                DeviceFlowKind::GitHub => "GitHub sign in",
            };
            let mut view = StepView::new(title, "c copy code  Esc cancel");
            view.lines.push(format!("Open {verification_uri}"));
            view.lines.push(format!("and enter the code {user_code}"));
            view.buttons = vec![
                ("Copy code", plain(Key::Char('c'))),
                (CANCEL, plain(Key::Esc)),
            ];
            view
        }
        ProviderSetupStep::LoadingModels { provider_name, .. } => {
            let mut view = StepView::new("Loading models", "Esc cancel");
            view.lines
                .push(format!("Fetching the model list for {provider_name}..."));
            view.buttons = vec![(CANCEL, plain(Key::Esc))];
            view
        }
        ProviderSetupStep::Done {
            provider_name,
            model_name,
        } => {
            let mut view = StepView::new("Provider configured", "Press any key to close");
            view.lines.push(match model_name {
                Some(model) => format!("{provider_name} / {model}"),
                None => provider_name.clone(),
            });
            view.buttons = vec![("OK", plain(Key::Enter))];
            view
        }
        ProviderSetupStep::GitLabSetup {
            url_input,
            token_input,
            active_field,
            error,
            ..
        } => {
            let mut view = StepView::new(
                "GitLab setup",
                "Tab switch fields  Enter validate & save  Esc cancel",
            );
            view.fields.push(Field {
                label: "Instance URL",
                value: url_input.clone(),
                active: *active_field == 0,
            });
            view.fields.push(Field {
                label: "Personal access token",
                value: mask(token_input),
                active: *active_field == 1,
            });
            view.error = error.clone();
            view
        }
        ProviderSetupStep::GitLabValidating { instance_url, .. } => {
            let mut view = StepView::new("GitLab setup", "Esc cancel");
            view.lines
                .push(format!("Validating the token for {instance_url}..."));
            view.buttons = vec![(CANCEL, plain(Key::Esc))];
            view
        }
        ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            protocol,
            interval_field,
            timeout_field,
            port_field,
            logs_field,
            active_field,
            error,
        } => {
            let mut view = StepView::new(
                "Telemetry setup",
                "Tab switch fields  Up/Down toggle protocol  Enter save  Esc cancel",
            );
            let protocol = match protocol {
                OtelProtocol::Http => "HTTP",
                OtelProtocol::Grpc => "gRPC",
            };
            let rows = [
                ("OTLP endpoint", endpoint_field.text().to_string()),
                ("Transport protocol", protocol.to_string()),
                ("Export interval (s)", interval_field.text().to_string()),
                ("Export timeout (s)", timeout_field.text().to_string()),
                ("Prometheus port", port_field.text().to_string()),
                ("Logs level", logs_field.text().to_string()),
            ];
            view.fields = rows
                .into_iter()
                .zip(0u8..)
                .map(|((label, value), i)| Field {
                    label,
                    value,
                    active: *active_field == i,
                })
                .collect();
            view.error = error.clone();
            view
        }
        ProviderSetupStep::SetupRouter {
            providers,
            selected_provider_ids,
            selected_provider_index,
            draft_config,
            active_bucket,
            active_bucket_index,
            left_pane_focused,
            error,
        } => {
            let mut view = StepView::new(
                "Model router setup",
                "Esc cancel | Tab switch pane | Up/Down move | Space toggle provider | \
                 Enter assign | Ctrl+S save | Ctrl+Up/Down reorder | Del remove",
            );
            view.lines.push(if *left_pane_focused {
                "Providers (focused):".to_string()
            } else {
                "Providers:".to_string()
            });
            view.lines
                .extend(providers.iter().enumerate().map(|(i, provider)| {
                    let mark = if selected_provider_ids.contains(&provider.id) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    let cursor = if i == *selected_provider_index {
                        ">"
                    } else {
                        " "
                    };
                    format!("{cursor} {mark} {}", provider.name)
                }));
            let bucket = active_bucket.to_string();
            view.lines.push(if *left_pane_focused {
                format!("Bucket {bucket}:")
            } else {
                format!("Bucket {bucket} (focused):")
            });
            if let Some(tier) = draft_config.tiers.get(&bucket) {
                view.lines
                    .extend(tier.models.iter().enumerate().map(|(i, entry)| {
                        let cursor = if i == *active_bucket_index { ">" } else { " " };
                        format!("{cursor} {}/{}", entry.provider_id, entry.model)
                    }));
            }
            view.buttons = vec![(CANCEL, plain(Key::Esc))];
            view.error = error.clone();
            view
        }
        // Drawn by the model picker; listed so a new step is not missed.
        ProviderSetupStep::SelectModel { .. } | ProviderSetupStep::SelectRouterModel { .. } => {
            StepView::new(super::model_picker::MODEL_PICKER_TITLE, LIST_HINT)
        }
    }
}

/// Move the highlight of a list step to `index`, as its Up and Down keys do.
fn set_selected(app: &mut App, index: usize) {
    match app.provider_setup.as_mut() {
        Some(
            ProviderSetupStep::SelectProvider { selected, .. }
            | ProviderSetupStep::ResetProvider { selected }
            | ProviderSetupStep::SelectConfiguredProvider { selected, .. }
            | ProviderSetupStep::ShowProviderConfig { selected, .. }
            | ProviderSetupStep::SelectAgent { selected, .. }
            | ProviderSetupStep::SelectThinkingLevel { selected, .. }
            | ProviderSetupStep::SelectAzureResource { selected, .. },
        ) => *selected = index,
        Some(
            ProviderSetupStep::EnterKey { .. }
            | ProviderSetupStep::DeviceFlowPending { .. }
            | ProviderSetupStep::LoadingModels { .. }
            | ProviderSetupStep::SelectModel { .. }
            | ProviderSetupStep::Done { .. }
            | ProviderSetupStep::GitLabSetup { .. }
            | ProviderSetupStep::GitLabValidating { .. }
            | ProviderSetupStep::SetupRouter { .. }
            | ProviderSetupStep::SelectRouterModel { .. }
            | ProviderSetupStep::TelemetrySetup { .. },
        )
        | None => {}
    }
}

/// Draw the provider setup dialog while the App runs the flow.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `provider_setup` step is drawn.
///
/// # Returns
///
/// `true` while the flow is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(step) = app.provider_setup.as_ref() else {
        return false;
    };
    let view = view(step);
    let mut picked = None;
    modal(ctx, "codemason_provider_setup", &view.title, |ui| {
        for line in &view.lines {
            ui.label(line);
        }
        if let Some(choice) = view.choice.as_ref() {
            if choice.options.is_empty() {
                ui.weak(choice.empty);
            } else {
                let mut selected = choice.selected.min(choice.options.len() - 1);
                let combo = craft_ui::ComboBox::new("codemason_provider_choice", choice.label)
                    .width(320.0)
                    .show_index(ui, &mut selected, choice.options.len(), |i| {
                        choice.options.get(i).cloned().unwrap_or_default()
                    });
                if combo.changed() {
                    picked = Some(selected);
                }
            }
        }
        for field in &view.fields {
            ui.horizontal(|ui| {
                ui.label(if field.active { ">" } else { " " });
                let label = ui.label(format!("{}:", field.label));
                ui.monospace(&field.value).labelled_by(label.id);
            });
        }
        if let Some(error) = view.error.as_ref() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.add_space(6.0);
        key_buttons(ui, &view.buttons);
        ui.weak(view.hint);
    });
    if let Some(index) = picked {
        set_selected(app, index);
    }
    true
}
