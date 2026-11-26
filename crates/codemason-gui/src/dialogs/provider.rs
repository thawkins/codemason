//! The provider setup dialog (`/provider`, `/model`, `/agent`, `/gitlab`,
//! `/telemetry`), over `App::provider_setup`.
//!
//! app-core runs the whole flow as a key-driven state machine
//! (`handle_provider_setup_key` in `input.rs`): Up and Down move a list's
//! highlight, Enter confirms a step (saving keys, settings and the chosen
//! provider), Esc cancels and typed characters fill the step's text fields.
//! This dialog draws the current step and drives the same machine:
//!
//! - a step that picks from a list draws it as Material list items; clicking an
//!   entry moves the step's highlight (what Up and Down move), a double click
//!   or Continue sends Enter, and a newly highlighted entry scrolls into view;
//! - a step with text fields shows them as the App holds them (secrets
//!   masked by `codemason_app_core::text_utils::mask_secret`), with the
//!   active field marked; typing
//!   reaches them as key presses through the router, as in the TUI, and a
//!   paste (Ctrl+V, which egui reports as a paste event alone) goes to
//!   `App::paste_text_into_provider_setup` ([`prepare`]);
//! - Cancel sends Esc, and every other button sends the key the TUI names
//!   for it.
//!
//! The model lists of the flow are drawn by [`super::model_picker`] and the
//! Model Router setup by [`super::router`].
//!
//! Dependencies: `codemason-app-core`, `codemason-config` (telemetry
//! protocol), `codemason-types` (thinking levels) and `egui`.

use codemason_app_core::app::{App, DeviceFlowKind, PROVIDER_LIST, ProviderSetupStep};
use codemason_app_core::input_event::{Key, KeyInput};
use codemason_app_core::text_utils::mask_secret;
use codemason_config::OtelProtocol;
use codemason_types::ThinkingLevel;

use egui::Id;

use super::{material_actions, material_hint, material_modal, plain, send_key};
use crate::material::list::list_item;
use crate::material::text_field::outlined_value;
use crate::metrics::{space, text_size};
use crate::palette::{palette_for, rgb};

/// Width of the dialog's content, in points.
const WIDTH: f32 = 440.0;

/// Tallest a step's list grows before it scrolls, in points.
const LIST_HEIGHT: f32 = 320.0;

/// The list's highlight last frame, to scroll a newly highlighted row into
/// view.
fn last_highlight_id() -> Id {
    Id::unique("codemason_provider_last_highlight")
}

/// Draw a step's list as Material list items, labelled with the list's
/// label (`Role::List`), with the App's highlight selected.
///
/// # Arguments
///
/// * `ui` - the dialog's ui.
/// * `palette` - the colours.
/// * `choice` - the step's list.
/// * `selected` - the highlighted entry.
/// * `moved` - whether the highlight moved since last frame; the row is then
///   scrolled into view.
///
/// # Returns
///
/// The clicked entry, if any, and whether it was double-clicked (chosen).
fn choice_list(
    ui: &mut egui::Ui,
    palette: &crate::palette::Palette,
    choice: &Choice,
    selected: usize,
    moved: bool,
) -> (Option<usize>, bool) {
    let mut clicked = (None, false);
    let list = egui::ScrollArea::vertical()
        .id_salt("codemason_provider_choice")
        .max_height(LIST_HEIGHT.min(super::body_height(ui.ctx(), 1.0, super::DIALOG_CHROME)))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, option) in choice.options.iter().enumerate() {
                    let item = list_item(ui, palette, option, None, None, i == selected);
                    if i == selected && moved {
                        item.scroll_to_me(None);
                    }
                    if item.double_clicked() {
                        clicked = (Some(i), true);
                    } else if item.clicked() {
                        clicked = (Some(i), false);
                    }
                }
            })
            .response
        });
    list.inner
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, choice.label));
    clicked
}

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
                format!("{provider_name} API key"),
                "Enter confirm  Esc cancel",
            );
            view.fields.push(Field {
                label: "API key",
                value: mask_secret(key_field.text()),
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
                value: mask_secret(token_input),
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
        // Drawn by the model picker; listed so a new step is not missed.
        ProviderSetupStep::SelectModel { .. } | ProviderSetupStep::SelectRouterModel { .. } => {
            StepView::new(super::model_picker::MODEL_PICKER_TITLE, LIST_HINT)
        }
        // Drawn by the router dialog; listed so a new step is not missed.
        ProviderSetupStep::SetupRouter { .. } => {
            StepView::new(super::router::ROUTER_TITLE, LIST_HINT)
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

/// Paste into the step's active text field before the key router.
///
/// egui turns Ctrl+V into an `Event::Paste` alone, so app-core never sees
/// the key the TUI pastes with. While a step with text fields is open
/// (EnterKey, GitLabSetup, TelemetrySetup), each paste is taken out of the
/// input and handed to `App::paste_text_into_provider_setup`, the app-core
/// action that strips carriage returns and fills the active field.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose provider step receives the paste.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
    let takes_text = matches!(
        app.provider_setup,
        Some(
            ProviderSetupStep::EnterKey { .. }
                | ProviderSetupStep::GitLabSetup { .. }
                | ProviderSetupStep::TelemetrySetup { .. }
        )
    );
    if !takes_text {
        return;
    }
    let mut pasted = Vec::new();
    ctx.input_mut(|input| {
        input.events.retain(|event| match event {
            egui::Event::Paste(text) => {
                pasted.push(text.clone());
                false
            }
            _ => true,
        });
    });
    for text in pasted {
        app.paste_text_into_provider_setup(&text);
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
    let palette = palette_for(app, ctx);
    let mut picked = None;
    let highlight = view.choice.as_ref().map(|c| c.selected);
    let moved = ctx.data_mut(|d| {
        let last = d.get_temp::<Option<usize>>(last_highlight_id()).flatten();
        d.insert_temp(last_highlight_id(), highlight);
        last != highlight
    });
    material_modal(
        ctx,
        &palette,
        "codemason_provider_setup",
        &view.title,
        WIDTH,
        |ui| {
            for line in &view.lines {
                ui.label(
                    egui::RichText::new(line)
                        .size(text_size::BODY)
                        .color(rgb(palette.m3.on_surface_variant)),
                );
            }
            if !view.lines.is_empty() {
                ui.add_space(space::LG);
            }
            if let Some(choice) = view.choice.as_ref() {
                if choice.options.is_empty() {
                    ui.label(
                        egui::RichText::new(choice.empty).color(rgb(palette.m3.on_surface_variant)),
                    );
                } else {
                    let selected = choice.selected.min(choice.options.len() - 1);
                    let (clicked, choose) = choice_list(ui, &palette, choice, selected, moved);
                    if let Some(i) = clicked {
                        picked = Some(i);
                        if choose {
                            send_key(ui.ctx(), plain(Key::Enter));
                        }
                    }
                }
                ui.add_space(space::LG);
            }
            // The step's error is the active field's supporting text, as a
            // Material field shows it; without fields it stands alone.
            let error_field = view.fields.iter().position(|f| f.active).unwrap_or(0);
            for (i, field) in view.fields.iter().enumerate() {
                let error = view.error.as_deref().filter(|_| i == error_field);
                outlined_value(
                    ui,
                    &palette,
                    field.label,
                    &field.value,
                    field.active,
                    error,
                    error.is_some(),
                );
                ui.add_space(space::LG);
            }
            if view.fields.is_empty()
                && let Some(error) = view.error.as_ref()
            {
                ui.colored_label(rgb(palette.m3.error), error);
                ui.add_space(space::LG);
            }
            material_hint(ui, &palette, view.hint);
            ui.add_space(space::LG);
            material_actions(ui, &palette, &view.buttons, true);
        },
    );
    if let Some(index) = picked {
        set_selected(app, index);
    }
    true
}
