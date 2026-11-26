//! The model picker: the provider setup flow's model lists.
//!
//! Draws `ProviderSetupStep::SelectModel` (after `/model` or `/provider`)
//! and `ProviderSetupStep::SelectRouterModel` (a router bucket's model) as a
//! list of the step's models with the highlighted one (`selected`) marked,
//! as the TUI's model table does.
//!
//! A filter field above the list narrows it to the models whose name or id
//! fuzzy-matches the typed text (`codemason_app_core::fuzzy`), best match
//! first. Favourite models are marked with a star and listed first; Ctrl+F
//! (app-core's) toggles the highlighted model as a favourite. The filter is a view over the
//! App's list: it only chooses which rows are shown and keeps the App's
//! highlight on a shown row. The choice itself is app-core's: Enter (the key,
//! the Select button or a double click) runs the TUI's `Enter` arm, which
//! selects the highlighted model, and Esc (or Cancel) runs its `Esc`.
//!
//! While the filter hides rows, [`prepare`] makes Up and Down move between
//! the shown rows (the App's arrows would walk the hidden ones too) and
//! keeps Enter from choosing a hidden row when nothing matches.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::{App, ModelPickerEntry, ProviderSetupStep};
use codemason_app_core::fuzzy::fuzzy_score;
use codemason_app_core::input_event::Key;
use egui::Id;

use super::{material_actions, material_hint, material_modal, plain, send_key};
use crate::material::list::list_item;
use crate::material::text_field::{field_id, outlined_text_field};
use crate::metrics::{space, text_size};
use crate::palette::{Palette, palette_for, rgb};

/// Width of the dialog's content, in points.
const WIDTH: f32 = 520.0;

/// Tallest the model list grows before it scrolls, in points.
const LIST_HEIGHT: f32 = 360.0;

/// The keys the picker answers to.
const HINT: &str = "Up/Down move  Enter select  Ctrl+F favourite  Esc cancel";

/// Title and accessible label of the dialog.
pub const MODEL_PICKER_TITLE: &str = "Select model";

/// Accessible label of the filter field.
pub const FILTER_LABEL: &str = "Filter models";

/// Accessible label of the model list.
pub const MODELS_LABEL: &str = "Models";

/// The button that selects the highlighted model (Enter).
pub const SELECT: &str = "Select";

/// The button that closes the picker (Esc).
pub const CANCEL: &str = "Cancel";

/// The mark before a favourite model's name.
pub const FAVORITE_MARK: &str = "*";

/// The egui memory slot of the filter text.
fn filter_id() -> Id {
    Id::unique("codemason_model_filter")
}

/// Widget id of the filter field.
///
/// # Returns
///
/// The id of the filter `TextEdit`, whose state holds its selection.
#[must_use]
pub fn filter_field_id() -> Id {
    field_id(FILTER_LABEL)
}

/// The highlighted row last frame, to scroll a newly highlighted row into
/// view.
fn last_selected_id() -> Id {
    Id::unique("codemason_model_last_selected")
}

/// The picker's step data: the provider's name, its models and the
/// highlighted index.
fn step(app: &App) -> Option<(&str, &[ModelPickerEntry], usize)> {
    match app.provider_setup.as_ref()? {
        ProviderSetupStep::SelectModel {
            provider_name,
            models,
            selected,
            ..
        }
        | ProviderSetupStep::SelectRouterModel {
            provider_name,
            models,
            selected,
            ..
        } => Some((provider_name, models, *selected)),
        _ => None,
    }
}

/// Move the App's highlight to `index`, as its Up and Down keys do.
fn set_selected(app: &mut App, index: usize) {
    if let Some(
        ProviderSetupStep::SelectModel { selected, .. }
        | ProviderSetupStep::SelectRouterModel { selected, .. },
    ) = app.provider_setup.as_mut()
    {
        *selected = index;
    }
}

/// The current filter text.
fn filter(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(filter_id()))
        .unwrap_or_default()
}

/// Forget the filter: the picker closed.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
pub fn reset(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.remove_temp::<String>(filter_id());
        d.remove_temp::<usize>(last_selected_id());
    });
}

/// The indices of the models whose name or id fuzzy-matches `filter`, best
/// first (ties keep list order); every index, in list order, when `filter`
/// is empty.
///
/// # Arguments
///
/// * `models` - the step's models.
/// * `filter` - the typed filter text.
///
/// # Returns
///
/// The indices to show, in display order.
#[must_use]
pub fn matching(models: &[ModelPickerEntry], filter: &str) -> Vec<usize> {
    let query = filter.trim();
    let mut scored: Vec<(u32, usize)> = models
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let by_name = fuzzy_score(query, &m.name);
            let by_id = fuzzy_score(query, &m.id);
            by_name.max(by_id).map(|score| (score, i))
        })
        .collect();
    scored.sort_by_key(|&(score, i)| (std::cmp::Reverse(score), i));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Handle Up, Down and Enter before the router while the filter hides rows:
/// Up and Down move the highlight to the previous or next shown row
/// (wrapping, as the App's arrows do), and Enter is held back when no row
/// is shown. With an empty filter every row is shown and the App's own keys
/// apply.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose highlight moves.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
    let filter = filter(ctx);
    if filter.is_empty() {
        return;
    }
    let Some((_, models, selected)) = step(app) else {
        return;
    };
    let shown = matching(models, &filter);
    let mut highlight = selected;
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let egui::Event::Key {
                key, pressed: true, ..
            } = event
            else {
                return true;
            };
            match key {
                egui::Key::ArrowUp | egui::Key::ArrowDown => {
                    highlight = step_within(&shown, highlight, *key == egui::Key::ArrowDown);
                    false
                }
                egui::Key::Enter => !shown.is_empty(),
                _ => true,
            }
        });
    });
    if highlight != selected {
        set_selected(app, highlight);
    }
}

/// The shown row after (or before) `current`, wrapping; `current` itself
/// when nothing is shown.
fn step_within(shown: &[usize], current: usize, down: bool) -> usize {
    let Some(at) = shown.iter().position(|&i| i == current) else {
        return shown.first().copied().unwrap_or(current);
    };
    let next = if down {
        (at + 1) % shown.len()
    } else {
        (at + shown.len() - 1) % shown.len()
    };
    shown[next]
}

/// The TUI's short context-window figure: `200K`, `1M`.
fn context_label(tokens: usize) -> String {
    if tokens >= 1_000_000 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1_000 {
        format!("{}K", tokens / 1_000)
    } else {
        tokens.to_string()
    }
}

/// Draw the model picker while the App shows a model list.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose provider setup step is drawn.
///
/// # Returns
///
/// `true` while the picker is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some((provider_name, models, selected)) = step(app) else {
        return false;
    };
    let palette = palette_for(app, ctx);
    let provider_name = provider_name.to_string();
    let favorites = app.favorite_models();
    let rows: Vec<Row> = models
        .iter()
        .map(|m| {
            let favorite = favorites.contains(&format!("{}/{}", m.provider_id, m.id));
            let name = if favorite {
                format!("{FAVORITE_MARK} {}", m.name)
            } else {
                m.name.clone()
            };
            // The id only adds something when the name does not already
            // carry it (local and cloud Ollama names are their ids).
            let id = (!m.name.to_lowercase().contains(&m.id.to_lowercase())).then(|| m.id.clone());
            let detail = format!("{}  {}", context_label(m.context_window), m.cost_tier);
            Row { name, id, detail }
        })
        .collect();
    let mut text = filter(ctx);
    let mut clicked: Option<(usize, bool)> = None;
    let mut shown = matching(models, &text);
    let moved = ctx.data_mut(|d| {
        let last = d.get_temp::<usize>(last_selected_id());
        d.insert_temp(last_selected_id(), selected);
        last != Some(selected)
    });
    material_modal(
        ctx,
        &palette,
        "codemason_model_picker",
        MODEL_PICKER_TITLE,
        WIDTH,
        |ui| {
            ui.label(
                egui::RichText::new(&provider_name)
                    .size(text_size::BODY)
                    .color(rgb(palette.m3.on_surface_variant)),
            );
            ui.add_space(space::LG);
            let field = outlined_text_field(ui, &palette, FILTER_LABEL, &mut text, None, false);
            // The filter takes the typing while the picker is open.
            if ctx.memory(|m| m.focused().is_none()) {
                field.request_focus();
            }
            ui.add_space(space::SM);
            shown = matching(models, &text);
            let list = egui::ScrollArea::vertical()
                .max_height(LIST_HEIGHT.min(super::body_height(
                    ui.ctx(),
                    1.0,
                    super::DIALOG_CHROME,
                )))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for &i in &shown {
                            let row = &rows[i];
                            let item = list_item(
                                ui,
                                &palette,
                                &row.name,
                                row.id.as_deref(),
                                Some(&row.detail),
                                i == selected,
                            );
                            if i == selected && moved {
                                item.scroll_to_me(None);
                            }
                            if item.double_clicked() {
                                clicked = Some((i, true));
                            } else if item.clicked() {
                                clicked = Some((i, false));
                            }
                        }
                        if shown.is_empty() {
                            empty_state(ui, &palette, &provider_name, &text, models.is_empty());
                        }
                    })
                    .response
                });
            list.inner
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, MODELS_LABEL));
            ui.add_space(space::LG);
            material_hint(ui, &palette, HINT);
            ui.add_space(space::LG);
            material_actions(
                ui,
                &palette,
                &[(SELECT, plain(Key::Enter)), (CANCEL, plain(Key::Esc))],
                !shown.is_empty(),
            );
        },
    );
    ctx.data_mut(|d| d.insert_temp(filter_id(), text));
    if let Some((i, choose)) = clicked {
        set_selected(app, i);
        if choose {
            send_key(ctx, plain(Key::Enter));
        }
    } else if let Some(&first) = shown.first()
        && !shown.contains(&selected)
    {
        // Keep the App's highlight on a shown row, so Enter picks what the
        // user sees.
        set_selected(app, first);
    }
    true
}

/// One model's row: its name (starred when a favourite), its id when the
/// name does not show it, and its context window and cost tier.
struct Row {
    name: String,
    id: Option<String>,
    detail: String,
}

/// What an empty list says: that the provider listed no models (with what
/// to try), or that nothing matches the filter.
fn empty_state(ui: &mut egui::Ui, palette: &Palette, provider: &str, filter: &str, none: bool) {
    let (headline, next) = if none {
        (
            format!("No models from {provider}"),
            "Check its API key and connection with /provider, then try again.".to_string(),
        )
    } else {
        (
            format!("No models match \"{}\"", filter.trim()),
            "Change or clear the filter.".to_string(),
        )
    };
    ui.add_space(space::LG);
    ui.label(
        egui::RichText::new(headline)
            .size(text_size::BODY)
            .color(rgb(palette.m3.on_surface)),
    );
    ui.label(
        egui::RichText::new(next)
            .size(text_size::SMALL)
            .color(rgb(palette.m3.on_surface_variant)),
    );
    ui.add_space(space::LG);
}
