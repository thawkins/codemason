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
//! This dialog draws the copy as the App holds it, the focused field marked
//! and an open list shown with its highlight. Clicking an entry of an open
//! list moves its highlight and sends Enter, so app-core picks it (a
//! disabled entry is greyed out and does nothing). Save sends Enter and
//! Cancel sends Esc.
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

use super::{key_buttons, modal, plain, send_key};

/// The button that saves the settings (Enter).
pub const SAVE: &str = "Save";

/// The button that discards the changes (Esc).
pub const CANCEL: &str = "Cancel";

/// Accessible label of an open choice list.
pub const CHOICES_LABEL: &str = "Choices";

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

/// The TUI's focus marker.
fn marker(active: bool) -> &'static str {
    if active { "> " } else { "  " }
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
    let Some(dialog) = app.sidebar.properties.as_ref() else {
        return false;
    };
    let title = properties_title(&dialog.project_name);
    let mut picked: Option<usize> = None;
    modal(ctx, "codemason_properties_dialog", &title, |ui| {
        egui::Grid::new("codemason_properties_fields")
            .num_columns(2)
            .show(ui, |ui| {
                let rows = [
                    (
                        "Description",
                        Field::Description,
                        dialog.description.as_str(),
                    ),
                    (
                        "Backend",
                        Field::Backend,
                        chosen(&dialog.backends, dialog.backend),
                    ),
                    ("Model", Field::Model, chosen(&dialog.models, dialog.model)),
                ];
                for (label, field, value) in rows {
                    ui.monospace(format!("{}{label}", marker(dialog.focus == field)));
                    ui.monospace(value);
                    ui.end_row();
                }
            });
        if let (Some(cursor), Some((choices, _))) = (dialog.list, dialog.focused_choices()) {
            let list = egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        for (i, choice) in choices.iter().enumerate() {
                            let row = ui.add_enabled(
                                choice.enabled,
                                egui::Button::selectable(i == cursor, choice.label.as_str()),
                            );
                            if row.clicked() {
                                picked = Some(i);
                            }
                        }
                    })
                    .response
                });
            list.inner
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, CHOICES_LABEL));
        }
        if let Some(error) = dialog.error.as_deref() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.add_space(6.0);
        key_buttons(ui, &[(SAVE, plain(Key::Enter)), (CANCEL, plain(Key::Esc))]);
        ui.weak("Tab move  Left/Right change  Space list  Enter save  Esc cancel");
    });
    if let Some(i) = picked
        && let Some(dialog) = app.sidebar.properties.as_mut()
    {
        dialog.list = Some(i);
        send_key(ctx, plain(Key::Enter));
    }
    true
}
