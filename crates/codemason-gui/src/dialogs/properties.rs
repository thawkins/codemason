//! The project `Properties` dialog, over `App::sidebar.properties`.
//!
//! app-core edits a copy of the project's description, backend and model
//! (`codemason_app_core::project_properties`): Tab/Shift+Tab (or Up/Down)
//! move between the fields, the usual editing keys edit the description,
//! Left/Right step a choice field through its enabled entries and Space
//! opens its full list (Up/Down, Enter picks, Esc closes the list), Enter
//! saves and Esc discards. The keys reach the App through the router, as in
//! the TUI; a paste goes to `App::handle_paste_text`, which puts it in the
//! description ([`prepare`]).
//!
//! The dialog is drawn in a Material dialog: each field is an outlined field
//! showing the copy as the App holds it, the focused one drawn focused (as
//! the TUI marks it with `> `), and an open choice list is a Material list
//! with its highlight. Clicking an entry of an open list moves its highlight
//! and sends Enter, so app-core picks it (a disabled entry is greyed out and
//! does nothing). Save sends Enter and Cancel sends Esc.
//!
//! Typed text the router cannot pair with a key (AltGr and Option
//! characters, dead keys, IME commits) is forwarded as its characters
//! (`Focus::Modal`), so the description gets every character once.
//!
//! The dialog is not one of the App's input locks, but it takes every key
//! while open, so the prompt treats it as one
//! ([`super::locks_prompt`]).
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::project_properties::{Choice, Field};

use super::{material_actions, material_hint, material_modal, plain, send_key};
use crate::material::list::list_item;
use crate::material::text_field::outlined_value;
use crate::metrics::{space, text_size};
use crate::palette::{palette_for, rgb};

/// The button that saves the settings (Enter).
pub const SAVE: &str = "Save";

/// The button that discards the changes (Esc).
pub const CANCEL: &str = "Cancel";

/// Label, and accessible name, of the description field.
pub const DESCRIPTION_LABEL: &str = "Description";

/// Label, and accessible name, of the backend choice.
pub const BACKEND_LABEL: &str = "Backend";

/// Label, and accessible name, of the model choice.
pub const MODEL_LABEL: &str = "Model";

/// Accessible label of an open choice list.
pub const CHOICES_LABEL: &str = "Choices";

/// Width of the dialog's content, in points.
const WIDTH: f32 = 480.0;

/// Tallest an open choice list grows before it scrolls, in points.
const LIST_HEIGHT: f32 = 240.0;

/// The keys the dialog answers to.
const HINT: &str = "Tab next field  Left/Right change  Space list  Enter save  Esc cancel";

/// Title of the dialog for project `name` (the TUI's border title).
///
/// # Arguments
///
/// * `name` - the project's name.
///
/// # Returns
///
/// The dialog title.
#[must_use]
pub fn properties_title(name: &str) -> String {
    format!("Properties: {name}")
}

/// Send this frame's pastes to the App, which puts them in the
/// description (`App::handle_paste_text`).
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose properties dialog is open.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
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
        app.handle_paste_text(&text);
    }
}

/// The label of the chosen entry of `choices`.
fn chosen(choices: &[Choice], index: usize) -> &str {
    choices.get(index).map_or("", |c| c.label.as_str())
}

/// Draw the properties dialog while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `sidebar.properties` is drawn.
///
/// # Returns
///
/// `true` while the dialog is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let palette = palette_for(app, ctx);
    let Some(dialog) = app.sidebar.properties.as_ref() else {
        return false;
    };
    let title = properties_title(&dialog.project_name);
    let mut picked: Option<usize> = None;
    material_modal(
        ctx,
        &palette,
        "codemason_properties_dialog",
        &title,
        WIDTH,
        |ui| {
            for (label, field, value) in [
                (
                    DESCRIPTION_LABEL,
                    Field::Description,
                    dialog.description.as_str(),
                ),
                (
                    BACKEND_LABEL,
                    Field::Backend,
                    chosen(&dialog.backends, dialog.backend),
                ),
                (
                    MODEL_LABEL,
                    Field::Model,
                    chosen(&dialog.models, dialog.model),
                ),
            ] {
                outlined_value(
                    ui,
                    &palette,
                    label,
                    value,
                    dialog.focus == field,
                    None,
                    false,
                );
                ui.add_space(space::LG);
            }
            if let (Some(cursor), Some((choices, _))) = (dialog.list, dialog.focused_choices()) {
                let list = egui::ScrollArea::vertical()
                    .id_salt("codemason_properties_choices")
                    .max_height(LIST_HEIGHT.min(super::body_height(
                        ui.ctx(),
                        1.0,
                        super::DIALOG_CHROME,
                    )))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (i, choice) in choices.iter().enumerate() {
                                let row = ui
                                    .add_enabled_ui(choice.enabled, |ui| {
                                        list_item(
                                            ui,
                                            &palette,
                                            &choice.label,
                                            None,
                                            None,
                                            i == cursor,
                                        )
                                    })
                                    .inner;
                                if row.clicked() {
                                    picked = Some(i);
                                }
                            }
                        })
                        .response
                    });
                list.inner.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::List, true, CHOICES_LABEL)
                });
                ui.add_space(space::SM);
            }
            if let Some(error) = dialog.error.as_deref() {
                ui.label(
                    egui::RichText::new(error)
                        .size(text_size::BODY)
                        .color(rgb(palette.m3.error)),
                );
                ui.add_space(space::SM);
            }
            material_hint(ui, &palette, HINT);
            ui.add_space(space::LG);
            material_actions(
                ui,
                &palette,
                &[(SAVE, plain(Key::Enter)), (CANCEL, plain(Key::Esc))],
                true,
            );
        },
    );
    if let Some(i) = picked
        && let Some(dialog) = app.sidebar.properties.as_mut()
    {
        dialog.list = Some(i);
        send_key(ctx, plain(Key::Enter));
    }
    true
}
