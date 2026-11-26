//! The theme editor overlay (`App::theme_editor`), opened from an edit
//! button in the `/theme` picker.
//!
//! Every colour of the theme is listed under its group (the first part of
//! its key: `text`, `bg`, `syntax`, `m3`, ...). A row has the colour's key,
//! a swatch that opens egui's colour picker and a hex field; a hex value is
//! taken once it is a valid `#rrggbb`. The window previews the edited theme
//! while the overlay is open, and the draft's contrast warnings are listed at
//! the top. Save writes the theme (`App::save_theme_edit`), Cancel drops the
//! edit and Reset to default (built-in themes only) deletes the user's copy.
//! Esc reaches app-core through the key router and cancels.
//!
//! The rows use plain egui widgets: `material` has no dense colour row (an
//! outlined text field is 56 points tall per colour), and egui's colour
//! picker has no Material equivalent.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::theme_tokens::Rgb;

use super::material_modal;
use crate::material::button::{ButtonKind, button};
use crate::metrics::space;
use crate::palette::rgb;

/// The editor's button that saves the edit.
pub const SAVE_THEME: &str = "Save";

/// The editor's button that drops the edit.
pub const CANCEL_EDIT: &str = "Cancel edit";

/// The editor's button that restores a built-in theme's default colours.
pub const RESET_THEME: &str = "Reset to default";

/// Width of the editor, in points.
const WIDTH: f32 = 520.0;

/// Height of the scrolling colour list, in points.
const LIST_HEIGHT: f32 = 420.0;

/// Width of a hex field, in points.
const HEX_WIDTH: f32 = 80.0;

/// The editor's title for the theme called `name`.
#[must_use]
pub fn editor_title(name: &str) -> String {
    format!("Edit theme: {name}")
}

/// The accessible label of the hex field of the colour at `key`.
#[must_use]
pub fn hex_label(key: &str) -> String {
    format!("{key} hex")
}

/// What a button of the editor asked for this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Save,
    Cancel,
    Reset,
}

/// Draw the theme editor while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `theme_editor` is drawn and edited.
///
/// # Returns
///
/// `true` while the editor is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(editor) = app.theme_editor.as_ref() else {
        return false;
    };
    let palette = crate::palette::palette_for(app, ctx);
    let colours = editor.draft.colours();
    let warnings = editor.warnings.clone();
    let builtin = editor.builtin;
    let title = editor_title(&editor.name);
    let mut edits: Vec<(String, Rgb)> = Vec::new();
    let mut action = None;
    material_modal(
        ctx,
        &palette,
        "codemason_theme_editor",
        &title,
        WIDTH,
        |ui| {
            for warning in &warnings {
                ui.colored_label(rgb(palette.m3.error), warning);
            }
            if !warnings.is_empty() {
                ui.add_space(space::SM);
            }
            // The colours scroll on their own, so the buttons stay in view.
            egui::ScrollArea::vertical()
                .id_salt("theme_editor_colours")
                .max_height(LIST_HEIGHT.min(super::body_height(
                    ui.ctx(),
                    1.0,
                    super::DIALOG_CHROME,
                )))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for group in colours.chunk_by(|a, b| group_of(&a.0) == group_of(&b.0)) {
                        let name = group_of(&group[0].0);
                        egui::CollapsingHeader::new(name)
                            .default_open(true)
                            .show(ui, |ui| {
                                for (key, colour) in group {
                                    if let Some(value) = colour_row(ui, key, *colour) {
                                        edits.push((key.clone(), value));
                                    }
                                }
                            });
                    }
                });
            ui.add_space(space::SM);
            ui.horizontal(|ui| {
                if button(ui, &palette, ButtonKind::Filled, None, SAVE_THEME).clicked() {
                    action = Some(Action::Save);
                }
                if button(ui, &palette, ButtonKind::Text, None, CANCEL_EDIT).clicked() {
                    action = Some(Action::Cancel);
                }
                if builtin
                    && button(ui, &palette, ButtonKind::Outlined, None, RESET_THEME).clicked()
                {
                    action = Some(Action::Reset);
                }
            });
        },
    );
    for (key, value) in edits {
        app.set_theme_colour(&key, value);
    }
    match action {
        Some(Action::Save) => app.save_theme_edit(),
        Some(Action::Cancel) => app.cancel_theme_edit(),
        Some(Action::Reset) => app.reset_theme_to_default(),
        None => {}
    }
    true
}

/// The group a colour key belongs to: its first dotted part.
fn group_of(key: &str) -> &str {
    key.split('.').next().unwrap_or(key)
}

/// Draw one colour row: the key, a swatch with egui's colour picker and a
/// hex field.
///
/// The hex field keeps what is being typed in egui's memory while it has the
/// keyboard, so a half-typed value is not replaced by the theme's colour on
/// the next frame.
///
/// # Returns
///
/// The new colour when the swatch or a valid hex value changed it.
fn colour_row(ui: &mut egui::Ui, key: &str, colour: Rgb) -> Option<Rgb> {
    let mut changed = None;
    ui.horizontal(|ui| {
        let mut srgb = [colour.0, colour.1, colour.2];
        if egui::widgets::color_picker::color_edit_button_srgb(ui, &mut srgb).changed() {
            changed = Some(Rgb(srgb[0], srgb[1], srgb[2]));
        }
        let buffer_id = ui.scope_id().with(("theme_hex", key));
        let mut text = ui
            .data(|d| d.get_temp::<String>(buffer_id))
            .unwrap_or_else(|| colour.to_hex());
        let label = hex_label(key);
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(HEX_WIDTH)
                .font(egui::TextStyle::Monospace),
        );
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label.as_str()));
        if response.changed()
            && let Some(value) = Rgb::from_hex(text.trim())
        {
            changed = Some(value);
        }
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(buffer_id, text));
        } else {
            ui.data_mut(|d| d.remove::<String>(buffer_id));
        }
        ui.label(key);
    });
    changed
}
