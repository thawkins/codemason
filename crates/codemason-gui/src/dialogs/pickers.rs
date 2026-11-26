//! The input history picker (`App::history_picker`), the `/config list`
//! save picker (`App::config_save_picker`) and the `/theme` picker
//! (`App::theme_picker`).
//!
//! The theme picker previews: moving its highlight restyles the window in
//! that theme, Enter keeps it and Esc restores the theme it opened on
//! (`handle_theme_picker_key`).
//!
//! app-core runs the other two (`handle_history_picker_key` and
//! `handle_config_save_picker_key` in `app/input_handler.rs`): Up/Down (or
//! k/j) move the highlight, Enter takes the highlighted entry (the history
//! picker puts it in the prompt, the save picker restores that backup over
//! the global configuration), Esc or `q` closes, and `c` copies the
//! highlighted history entry. The keys reach the App through the router, as
//! in the TUI.
//!
//! Every theme row but `system` has an edit button that opens the theme
//! editor on that theme (`App::open_theme_editor`, drawn by
//! [`super::theme_editor`]).
//!
//! This dialog lists the entries with the highlight. Clicking an entry
//! moves the highlight (the state the arrows move); a double click or the
//! Select / Restore button sends Enter, Copy sends `c` and Close sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::{key_buttons, modal, plain, send_key};

/// Title of the history picker.
pub const HISTORY_TITLE: &str = "History";

/// Title of the config save picker.
pub const CONFIG_SAVES_TITLE: &str = "Saved configurations";

/// Title of the theme picker.
pub const THEME_TITLE: &str = "Theme";

/// The theme picker's button that keeps the previewed theme (Enter).
pub const APPLY_THEME: &str = "Apply";

/// The theme picker's button that restores the previous theme (Esc).
pub const CANCEL_THEME: &str = "Cancel";

/// The accessible label (and hover text) of the edit button on the theme
/// picker's row for `name`.
#[must_use]
pub fn edit_theme_label(name: &str) -> String {
    format!("Edit {name}")
}

/// Accessible label of a picker's entry list.
pub const ENTRIES_LABEL: &str = "Entries";

/// The history picker's button that puts the entry in the prompt (Enter).
pub const SELECT_ENTRY: &str = "Select";

/// The history picker's button that copies the entry (`c`).
pub const COPY_ENTRY: &str = "Copy";

/// The save picker's button that restores the backup (Enter).
pub const RESTORE: &str = "Restore";

/// The button that closes a picker (Esc).
pub const CLOSE: &str = "Close";

/// Draw `rows` as a list labelled `label` with `selected` highlighted;
/// returns a clicked row and whether it was double-clicked.
pub(crate) fn entry_list(
    ui: &mut egui::Ui,
    label: &str,
    rows: &[String],
    selected: usize,
) -> Option<(usize, bool)> {
    let mut clicked = None;
    let height = super::body_height(ui.ctx(), 0.6, super::DIALOG_CHROME);
    let list = egui::ScrollArea::vertical()
        .max_height(height)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                for (i, row) in rows.iter().enumerate() {
                    let response = ui.selectable_label(i == selected, row.as_str());
                    if response.double_clicked() {
                        clicked = Some((i, true));
                    } else if response.clicked() {
                        clicked = Some((i, false));
                    }
                }
                if rows.is_empty() {
                    ui.weak("No entries");
                }
            })
            .response
        });
    list.inner
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, label));
    clicked
}

/// Draw the input history picker while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `history_picker` is drawn.
///
/// # Returns
///
/// `true` while the picker is open.
pub fn show_history(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(picker) = app.history_picker.as_ref() else {
        return false;
    };
    let rows = picker.entries.clone();
    let selected = picker.selected;
    let mut clicked = None;
    modal(ctx, "codemason_history_picker", HISTORY_TITLE, |ui| {
        ui.weak(format!("{} entries", rows.len()));
        clicked = entry_list(ui, ENTRIES_LABEL, &rows, selected);
        ui.add_space(crate::metrics::space::SM);
        ui.add_enabled_ui(!rows.is_empty(), |ui| {
            key_buttons(
                ui,
                &[
                    (SELECT_ENTRY, plain(Key::Enter)),
                    (COPY_ENTRY, plain(Key::Char('c'))),
                ],
            );
        });
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Up/Down navigate * Enter select * c copy * Esc close");
    });
    if let Some((i, choose)) = clicked
        && let Some(picker) = app.history_picker.as_mut()
    {
        picker.selected = i;
        if choose {
            send_key(ctx, plain(Key::Enter));
        }
    }
    true
}

/// Draw the `/theme` picker while the App shows it.
///
/// A click on an entry previews that theme; a double click also applies it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `theme_picker` is drawn.
///
/// # Returns
///
/// `true` while the picker is open.
pub fn show_theme(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(picker) = app.theme_picker.as_ref() else {
        return false;
    };
    let rows = picker.entries.clone();
    let selected = picker.selected;
    let palette = crate::palette::palette_for(app, ctx);
    let mut clicked = None;
    let mut edit = None;
    modal(ctx, "codemason_theme_picker", THEME_TITLE, |ui| {
        ui.weak("The window previews the highlighted theme.");
        (clicked, edit) = theme_entry_list(ui, &palette, &rows, selected);
        ui.add_space(crate::metrics::space::SM);
        key_buttons(
            ui,
            &[
                (APPLY_THEME, plain(Key::Enter)),
                (CANCEL_THEME, plain(Key::Esc)),
            ],
        );
        ui.weak("Up/Down preview * Enter apply * Esc cancel");
    });
    if let Some(i) = edit
        && let Some(name) = rows.get(i)
    {
        app.open_theme_editor(name);
    } else if let Some((i, choose)) = clicked {
        app.theme_picker_select(i);
        if choose {
            send_key(ctx, plain(Key::Enter));
        }
    }
    true
}

/// Draw the theme picker's rows as [`entry_list`] does, with an edit button
/// on every row but the first (`system`, which has no colours of its own).
///
/// # Returns
///
/// The clicked row as [`entry_list`] reports it, and the row whose edit
/// button was pressed.
fn theme_entry_list(
    ui: &mut egui::Ui,
    palette: &crate::palette::Palette,
    rows: &[String],
    selected: usize,
) -> (Option<(usize, bool)>, Option<usize>) {
    use crate::icons::Icon;
    use crate::material::icon_button::{IconButtonKind, icon_button_inline};

    let mut clicked = None;
    let mut edit = None;
    let height = super::body_height(ui.ctx(), 0.6, super::DIALOG_CHROME);
    let list = egui::ScrollArea::vertical()
        .max_height(height)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                for (i, row) in rows.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let response = ui.selectable_label(i == selected, row.as_str());
                        if response.double_clicked() {
                            clicked = Some((i, true));
                        } else if response.clicked() {
                            clicked = Some((i, false));
                        }
                        if i > 0
                            && icon_button_inline(
                                ui,
                                palette,
                                IconButtonKind::Standard,
                                Icon::Edit,
                                &edit_theme_label(row),
                                None,
                            )
                            .clicked()
                        {
                            edit = Some(i);
                        }
                    });
                }
            })
            .response
        });
    list.inner
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, ENTRIES_LABEL));
    (clicked, edit)
}

/// Draw the `/config list` save picker while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `config_save_picker` is drawn.
///
/// # Returns
///
/// `true` while the picker is open.
pub fn show_config_saves(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(picker) = app.config_save_picker.as_ref() else {
        return false;
    };
    let rows: Vec<String> = picker
        .entries
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>")
                .to_string()
        })
        .collect();
    let selected = picker.selected;
    let dir = picker.config_dir.display().to_string();
    let mut clicked = None;
    modal(
        ctx,
        "codemason_config_save_picker",
        CONFIG_SAVES_TITLE,
        |ui| {
            ui.weak(format!("{} entries in {dir}", rows.len()));
            clicked = entry_list(ui, ENTRIES_LABEL, &rows, selected);
            ui.add_space(crate::metrics::space::SM);
            ui.add_enabled_ui(!rows.is_empty(), |ui| {
                key_buttons(ui, &[(RESTORE, plain(Key::Enter))]);
            });
            key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
            ui.weak("Up/Down navigate * Enter restore * Esc close");
        },
    );
    if let Some((i, choose)) = clicked
        && let Some(picker) = app.config_save_picker.as_mut()
    {
        picker.selected = i;
        if choose {
            send_key(ctx, plain(Key::Enter));
        }
    }
    true
}
