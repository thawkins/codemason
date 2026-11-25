//! The Model Router setup dialog (`ProviderSetupStep::SetupRouter`), drawn
//! with Material components.
//!
//! app-core runs the setup as a key-driven state machine
//! (`handle_router_setup_key` in `input.rs`): Tab switches between the
//! provider pane and the tier pane, Space toggles a provider in the
//! cluster, Enter opens the model picker for the highlighted provider,
//! Ctrl+Up/Down reorders a tier's models, Delete removes one, Ctrl+S asks to
//! save and Esc cancels. This dialog draws the step and drives the same
//! machine: a click first moves the step's highlight and pane focus (what
//! Tab, Up and Down move), then sends the key the action is bound to.
//!
//! - the cluster's providers are filter chips; a click toggles one (Space);
//! - the four tiers are segmented buttons, each labelled with its model
//!   count; a click makes it the active tier;
//! - the active tier's models are list items in fallback order, each with
//!   move up, move down and remove icon buttons;
//! - an assist chip per provider in the cluster adds one of its models to
//!   the active tier (Enter, which opens [`super::model_picker`]);
//! - Save (Ctrl+S) and Cancel (Esc) are the dialog's actions.
//!
//! Dependencies: `codemason-app-core`, `codemason-agent` (router tiers) and
//! `egui`.

use codemason_agent::provider::router_config::{RouterConfig, Tier, TierEntry};
use codemason_app_core::app::{App, ConfiguredProvider, ProviderSetupStep};
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use egui::{CornerRadius, Stroke, StrokeKind};

use super::{material_actions, material_hint, material_modal, plain, send_key};
use crate::icons::Icon;
use crate::material::chip::{ChipKind, chip};
use crate::material::icon_button::{IconButtonKind, icon_button_dense};
use crate::material::list::{TWO_LINE_HEIGHT, list_item};
use crate::material::segmented::segmented_buttons;
use crate::metrics::{space, text_size};
use crate::palette::{Palette, palette_for, rgb};

/// Title of the dialog, also its accessible label.
pub const ROUTER_TITLE: &str = "Model Router";

/// Accessible label of the active tier's model list.
pub const MODELS_LABEL: &str = "Tier models";

/// The button that asks to save the cluster (Ctrl+S).
pub const SAVE: &str = "Save";

/// The button that cancels the setup (Esc).
pub const CANCEL: &str = "Cancel";

/// Width of the dialog's content, in points.
const WIDTH: f32 = 560.0;

/// Tallest the model list grows before it scrolls, in points.
const LIST_HEIGHT: f32 = 240.0;

/// Width of the outline marking the keyboard-highlighted chip.
const HIGHLIGHT_WIDTH: f32 = 2.0;

/// Corner radius of a chip, which the highlight outline follows.
const CHIP_RADIUS: u8 = 8;

/// The keyboard hint under the content.
const HINT: &str = "Tab pane  Space toggle  Enter add  Ctrl+Up/Down reorder  Del remove  \
                    Ctrl+S save  Esc cancel";

/// The model list's highlight last frame, to scroll a newly highlighted row
/// into view.
fn last_highlight_id() -> egui::Id {
    egui::Id::unique("codemason_router_last_highlight")
}

/// Outline `rect` in `secondary`: the keyboard highlight on a chip, which
/// has no highlighted state of its own.
fn outline(ui: &egui::Ui, palette: &Palette, rect: egui::Rect, radius: u8) {
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(HIGHLIGHT_WIDTH, rgb(palette.m3.secondary)),
        StrokeKind::Inside,
    );
}

/// Accessible label of the assist chip adding a model from `provider`.
#[must_use]
pub fn add_label(provider: &str) -> String {
    format!("Add from {provider}")
}

/// Accessible label of the button moving `model` up the fallback order.
#[must_use]
pub fn move_up_label(model: &str) -> String {
    format!("Move {model} up")
}

/// Accessible label of the button moving `model` down the fallback order.
#[must_use]
pub fn move_down_label(model: &str) -> String {
    format!("Move {model} down")
}

/// Accessible label of the button removing `model` from the tier.
#[must_use]
pub fn remove_label(model: &str) -> String {
    format!("Remove {model}")
}

/// A tier's name as the dialog shows it.
const fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Simple => "Simple",
        Tier::Medium => "Medium",
        Tier::Complex => "Complex",
        Tier::Reasoning => "Reasoning",
    }
}

/// What requests a tier receives, shown under the tier buttons.
const fn tier_description(tier: Tier) -> &'static str {
    match tier {
        Tier::Simple => "Short, low-complexity requests.",
        Tier::Medium => "Moderate complexity, typical coding questions.",
        Tier::Complex => "Advanced reasoning and multi-step tasks.",
        Tier::Reasoning => "Deep analysis, proofs and mathematical reasoning.",
    }
}

/// The models of `tier` in `config`, in fallback order.
fn tier_models(config: &RouterConfig, tier: Tier) -> &[TierEntry] {
    config
        .tiers
        .get(&tier.to_string())
        .map_or(&[], |t| t.models.as_slice())
}

/// Where a click moves the step's highlight before its key is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// Provider `index` in the provider pane.
    Provider(usize),
    /// The first model of `tier` in the tier pane.
    Tier(Tier),
    /// Model `index` of the active tier in the tier pane.
    Model(usize),
}

/// A click's effect: the highlight to move to, then the key to send.
#[derive(Debug, Clone, Copy)]
struct Action {
    focus: Option<Focus>,
    key: Option<KeyInput>,
}

impl Action {
    /// Move the highlight to `focus`, then send `key` if any.
    const fn at(focus: Focus, key: Option<KeyInput>) -> Self {
        Self {
            focus: Some(focus),
            key,
        }
    }
}

/// `key` with Ctrl held.
const fn ctrl(key: Key) -> KeyInput {
    KeyInput {
        key,
        mods: Modifiers::CTRL,
    }
}

/// Move the router step's highlight and pane focus to `focus`, as Tab, Up,
/// Down, Left and Right do.
fn apply_focus(app: &mut App, focus: Focus) {
    let Some(ProviderSetupStep::SetupRouter {
        selected_provider_index,
        active_bucket,
        active_bucket_index,
        left_pane_focused,
        ..
    }) = app.provider_setup.as_mut()
    else {
        return;
    };
    match focus {
        Focus::Provider(index) => {
            *selected_provider_index = index;
            *left_pane_focused = true;
        }
        Focus::Tier(tier) => {
            *active_bucket = tier;
            *active_bucket_index = 0;
            *left_pane_focused = false;
        }
        Focus::Model(index) => {
            *active_bucket_index = index;
            *left_pane_focused = false;
        }
    }
}

/// A section heading (M3 Title Small) in `on_surface`.
fn section_title(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(text_size::BODY)
            .strong()
            .color(rgb(palette.m3.on_surface)),
    );
    ui.add_space(space::XS);
}

/// Supporting text (M3 Body Small) in `on_surface_variant`.
fn supporting(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(text_size::SMALL)
            .color(rgb(palette.m3.on_surface_variant)),
    );
}

/// The provider section: a filter chip per configured provider, on while
/// it is in the cluster; the keyboard-highlighted chip is outlined while
/// the provider pane has focus.
fn providers_section(
    ui: &mut egui::Ui,
    palette: &Palette,
    providers: &[ConfiguredProvider],
    selected_ids: &[String],
    highlighted: Option<usize>,
) -> Option<Action> {
    let mut action = None;
    section_title(ui, palette, "Providers");
    supporting(
        ui,
        palette,
        "Choose the providers whose models the router may use.",
    );
    ui.add_space(space::SM);
    if providers.is_empty() {
        supporting(
            ui,
            palette,
            "No providers are configured. Add one with /provider first.",
        );
        return None;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(space::SM, space::SM);
        for (i, provider) in providers.iter().enumerate() {
            let on = selected_ids.contains(&provider.id);
            let response = chip(
                ui,
                palette,
                ChipKind::Filter { selected: on },
                None,
                &provider.name,
            );
            if highlighted == Some(i) {
                let grow = HIGHLIGHT_WIDTH;
                outline(
                    ui,
                    palette,
                    response.rect.expand(grow),
                    CHIP_RADIUS + grow as u8,
                );
            }
            if response.clicked() {
                action = Some(Action::at(Focus::Provider(i), Some(plain(Key::Char(' ')))));
            }
        }
    });
    action
}

/// One model row: the list item (model, with its provider as supporting
/// text) and its move up, move down and remove buttons. The selected row
/// is scrolled into view when `moved`.
#[allow(clippy::too_many_arguments)] // one row's data, each used once
fn model_row(
    ui: &mut egui::Ui,
    palette: &Palette,
    entry: &TierEntry,
    provider_name: &str,
    index: usize,
    count: usize,
    selected: bool,
    moved: bool,
) -> Option<Action> {
    let mut action = None;
    let size = egui::vec2(ui.available_width(), TWO_LINE_HEIGHT);
    let layout = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        let buttons = [
            (Icon::Cross, remove_label(&entry.model), Key::Delete, true),
            (
                Icon::ArrowDownward,
                move_down_label(&entry.model),
                Key::Down,
                index + 1 < count,
            ),
            (
                Icon::ArrowUpward,
                move_up_label(&entry.model),
                Key::Up,
                index > 0,
            ),
        ];
        for (icon, label, key, enabled) in buttons {
            let clicked = ui
                .add_enabled_ui(enabled, |ui| {
                    icon_button_dense(ui, palette, IconButtonKind::Standard, icon, &label, None)
                })
                .inner
                .clicked();
            if clicked {
                // Delete takes no modifier; the moves are Ctrl+Up/Down.
                let key = if key == Key::Delete {
                    plain(key)
                } else {
                    ctrl(key)
                };
                action = Some(Action::at(Focus::Model(index), Some(key)));
            }
        }
        let item = list_item(
            ui,
            palette,
            &entry.model,
            Some(provider_name),
            None,
            selected,
        );
        if selected && moved {
            item.scroll_to_me(None);
        }
        if item.clicked() && action.is_none() {
            action = Some(Action::at(Focus::Model(index), None));
        }
    });
    action
}

/// The router step's fields the dialog draws.
struct View<'a> {
    providers: &'a [ConfiguredProvider],
    selected_ids: &'a [String],
    provider_index: usize,
    config: &'a RouterConfig,
    tier: Tier,
    model_index: usize,
    left_focused: bool,
    error: Option<&'a str>,
}

/// The tier section: the tier buttons, the active tier's models and the
/// chips adding a model from a provider in the cluster.
fn tiers_section(ui: &mut egui::Ui, palette: &Palette, view: &View<'_>) -> Option<Action> {
    let mut action = None;
    section_title(ui, palette, "Tiers");
    supporting(
        ui,
        palette,
        "Each request goes to a tier by its complexity; the tier tries its models in order.",
    );
    ui.add_space(space::SM);
    let tiers = Tier::all();
    let labels: Vec<String> = tiers
        .iter()
        .map(|t| format!("{} ({})", tier_name(*t), tier_models(view.config, *t).len()))
        .collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut active = tiers.iter().position(|t| *t == view.tier).unwrap_or(0);
    if segmented_buttons(ui, palette, &label_refs, &mut active).changed()
        && let Some(tier) = tiers.get(active)
    {
        action = Some(Action::at(Focus::Tier(*tier), None));
    }
    ui.add_space(space::XS);
    supporting(ui, palette, tier_description(view.tier));
    ui.add_space(space::SM);

    let models = tier_models(view.config, view.tier);
    let highlight = (!view.left_focused).then_some((view.tier, view.model_index));
    let moved = ui.ctx().data_mut(|d| {
        let last = d
            .get_temp::<Option<(Tier, usize)>>(last_highlight_id())
            .flatten();
        d.insert_temp(last_highlight_id(), highlight);
        last != highlight
    });
    if models.is_empty() {
        ui.label(
            egui::RichText::new(format!("No models in {}", tier_name(view.tier)))
                .size(text_size::BODY)
                .color(rgb(palette.m3.on_surface)),
        );
        supporting(ui, palette, "Add one from a provider below.");
    } else {
        let list = egui::ScrollArea::vertical()
            .id_salt("codemason_router_models")
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, entry) in models.iter().enumerate() {
                        let provider = view
                            .providers
                            .iter()
                            .find(|p| p.id == entry.provider_id)
                            .map_or(entry.provider_id.as_str(), |p| p.name.as_str());
                        let selected = !view.left_focused && i == view.model_index;
                        if let Some(a) = model_row(
                            ui,
                            palette,
                            entry,
                            provider,
                            i,
                            models.len(),
                            selected,
                            moved,
                        ) {
                            action = Some(a);
                        }
                    }
                })
                .response
            });
        list.inner
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, MODELS_LABEL));
    }
    ui.add_space(space::MD);

    let in_cluster: Vec<(usize, &ConfiguredProvider)> = view
        .providers
        .iter()
        .enumerate()
        .filter(|(_, p)| view.selected_ids.contains(&p.id))
        .collect();
    if in_cluster.is_empty() {
        supporting(ui, palette, "Turn on a provider above to add its models.");
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(space::SM, space::SM);
            for (i, provider) in in_cluster {
                let label = add_label(&provider.name);
                if chip(ui, palette, ChipKind::Assist, Some(Icon::Add), &label).clicked() {
                    action = Some(Action::at(Focus::Provider(i), Some(plain(Key::Enter))));
                }
            }
        });
    }
    action
}

/// Draw the Model Router setup dialog while the App is on the router step.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `ProviderSetupStep::SetupRouter` step is drawn;
///   a click moves its highlight and queues the action's key press.
///
/// # Returns
///
/// `true` while the router step is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids,
        selected_provider_index,
        draft_config,
        active_bucket,
        active_bucket_index,
        left_pane_focused,
        error,
    }) = app.provider_setup.as_ref()
    else {
        return false;
    };
    let view = View {
        providers,
        selected_ids: selected_provider_ids,
        provider_index: *selected_provider_index,
        config: draft_config,
        tier: *active_bucket,
        model_index: *active_bucket_index,
        left_focused: *left_pane_focused,
        error: error.as_deref(),
    };
    let palette = palette_for(app, ctx);
    let action = material_modal(
        ctx,
        &palette,
        "codemason_router_setup",
        ROUTER_TITLE,
        WIDTH,
        |ui| {
            let highlighted = view.left_focused.then_some(view.provider_index);
            let mut action =
                providers_section(ui, &palette, view.providers, view.selected_ids, highlighted);
            ui.add_space(space::LG);
            if let Some(a) = tiers_section(ui, &palette, &view) {
                action = Some(a);
            }
            ui.add_space(space::LG);
            if let Some(error) = view.error {
                ui.colored_label(rgb(palette.m3.error), error);
                ui.add_space(space::SM);
            }
            material_hint(ui, &palette, HINT);
            ui.add_space(space::LG);
            material_actions(
                ui,
                &palette,
                &[(SAVE, ctrl(Key::Char('s'))), (CANCEL, plain(Key::Esc))],
                true,
            );
            action
        },
    );
    if let Some(action) = action {
        if let Some(focus) = action.focus {
            apply_focus(app, focus);
        }
        if let Some(key) = action.key {
            send_key(ctx, key);
        }
    }
    true
}
