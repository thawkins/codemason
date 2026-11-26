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
//! This dialog lists the entries as Material list items with the highlight
//! (`selected`). Clicking an entry moves the highlight (the state the arrows
//! move); a double click or the Select / Restore button sends Enter, Copy
//! sends `c` and Close sends Esc.
//!
//! Dependencies: `codemason-app-core`, `egui`, this crate's Material list
//! and the palette.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::{DIALOG_CHROME, action_row, material_hint, material_modal, plain, send_key};
use crate::material::button::ButtonKind;
use crate::material::list::list_item_with;
use crate::metrics::space;
use crate::palette::palette_for;

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

/// The message a picker with no entries shows.
const NO_ENTRIES: &str = "No entries";

/// Width of a picker's content, in points.
const WIDTH: f32 = 440.0;

/// Tallest a picker's list grows before it scrolls, as a share of the
/// window.
const LIST_SHARE: f32 = 0.6;

/// Draw `rows` as a Material list labelled `label`, with `selected`
/// highlighted and a trailing slot per row.
///
/// # Arguments
///
/// * `ui` - the dialog's ui.
/// * `palette` - the colours.
/// * `label` - the list's accessible label.
/// * `rows` - the row texts, also each row's accessible name.
/// * `hints` - a supporting line per row, or `None` for a one-line row.
/// * `selected` - the highlighted row.
/// * `slot` - draws a row's trailing slot; its `(row, ui)` value is
///   returned per row.
///
/// # Returns
///
/// The clicked row and whether it was double-clicked, if any.
pub(crate) fn entry_list_with(
    ui: &mut egui::Ui,
    palette: &crate::palette::Palette,
    label: &str,
    rows: &[String],
    hints: &[Option<String>],
    selected: usize,
    mut slot: impl FnMut(usize, &mut egui::Ui),
) -> Option<(usize, bool)> {
    let mut clicked = None;
    let height = super::body_height(ui.ctx(), LIST_SHARE, DIALOG_CHROME);
    let list = egui::ScrollArea::vertical()
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, row) in rows.iter().enumerate() {
                    let hint = hints.get(i).and_then(Option::as_deref);
                    let item = list_item_with(ui, palette, row, hint, None, i == selected, |ui| {
                        slot(i, ui)
                    })
                    .0;
                    if item.double_clicked() {
                        clicked = Some((i, true));
                    } else if item.clicked() {
                        clicked = Some((i, false));
                    }
                }
            })
            .response
        });
    if rows.is_empty() {
        material_hint(ui, palette, NO_ENTRIES);
    }
    list.inner
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, label));
    clicked
}

/// The hint list for rows that carry none.
fn no_hints(len: usize) -> Vec<Option<String>> {
    vec![None; len]
}

/// The number of entries line a picker shows.
fn entries_line(count: usize, suffix: &str) -> String {
    format!("{count} entries{suffix}")
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
    let hints = no_hints(rows.len());
    let palette = palette_for(app, ctx);
    let mut clicked = None;
    material_modal(
        ctx,
        &palette,
        "codemason_history_picker",
        HISTORY_TITLE,
        WIDTH,
        |ui| {
            material_hint(ui, &palette, &entries_line(rows.len(), ""));
            ui.add_space(space::SM);
            clicked = entry_list_with(
                ui,
                &palette,
                ENTRIES_LABEL,
                &rows,
                &hints,
                selected,
                |_, _| (),
            );
            ui.add_space(space::LG);
            action_row(
                ui,
                &palette,
                &[
                    (
                        SELECT_ENTRY,
                        ButtonKind::Filled,
                        &[plain(Key::Enter)],
                        !rows.is_empty(),
                    ),
                    (
                        COPY_ENTRY,
                        ButtonKind::Text,
                        &[plain(Key::Char('c'))],
                        !rows.is_empty(),
                    ),
                    (CLOSE, ButtonKind::Text, &[plain(Key::Esc)], true),
                ],
            );
            ui.add_space(space::MD);
            material_hint(
                ui,
                &palette,
                "Up/Down navigate * Enter select * c copy * Esc close",
            );
        },
    );
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
/// A click on an entry previews that theme; a double click also applies it,
/// and the edit button on a row opens the editor for that theme. Every one of
/// those has a key equivalent (`handle_theme_picker_key`): Up/Down preview,
/// Enter applies and `e` opens the editor on the highlighted row, so the
/// picker is fully usable without a pointer.
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
    let hints = no_hints(rows.len());
    let palette = palette_for(app, ctx);
    let mut clicked = None;
    let mut edit = None;
    material_modal(
        ctx,
        &palette,
        "codemason_theme_picker",
        THEME_TITLE,
        WIDTH,
        |ui| {
            material_hint(ui, &palette, "The window previews the highlighted theme.");
            ui.add_space(space::SM);
            clicked = entry_list_with(
                ui,
                &palette,
                ENTRIES_LABEL,
                &rows,
                &hints,
                selected,
                |i, ui| {
                    if i > 0 && theme_edit_button(ui, &palette, &rows[i]).clicked() {
                        edit = Some(i);
                    }
                },
            );
            ui.add_space(space::LG);
            action_row(
                ui,
                &palette,
                &[
                    (
                        APPLY_THEME,
                        ButtonKind::Filled,
                        &[plain(Key::Enter)],
                        !rows.is_empty(),
                    ),
                    (CANCEL_THEME, ButtonKind::Text, &[plain(Key::Esc)], true),
                ],
            );
            ui.add_space(space::MD);
            material_hint(
                ui,
                &palette,
                "Up/Down preview * Enter apply * e edit * Esc cancel",
            );
        },
    );
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

/// The theme picker's per-row edit button, opening the editor on `name`.
///
/// The first row (`system`) has no colours of its own, so it has no button.
///
/// # Arguments
///
/// * `ui` - the row's trailing slot.
/// * `palette` - the colours.
/// * `name` - the row's theme.
///
/// # Returns
///
/// The button's response; `clicked()` opens the editor for that theme.
fn theme_edit_button(
    ui: &mut egui::Ui,
    palette: &crate::palette::Palette,
    name: &str,
) -> egui::Response {
    use crate::icons::Icon;
    use crate::material::icon_button::{IconButtonKind, icon_button_inline};

    icon_button_inline(
        ui,
        palette,
        IconButtonKind::Standard,
        Icon::Edit,
        &edit_theme_label(name),
        None,
    )
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
    let hints = no_hints(rows.len());
    let selected = picker.selected;
    let dir = picker.config_dir.display().to_string();
    let palette = palette_for(app, ctx);
    let mut clicked = None;
    material_modal(
        ctx,
        &palette,
        "codemason_config_save_picker",
        CONFIG_SAVES_TITLE,
        WIDTH,
        |ui| {
            material_hint(
                ui,
                &palette,
                &entries_line(rows.len(), &format!(" in {dir}")),
            );
            ui.add_space(space::SM);
            clicked = entry_list_with(
                ui,
                &palette,
                ENTRIES_LABEL,
                &rows,
                &hints,
                selected,
                |_, _| (),
            );
            ui.add_space(space::LG);
            action_row(
                ui,
                &palette,
                &[
                    (
                        RESTORE,
                        ButtonKind::Filled,
                        &[plain(Key::Enter)],
                        !rows.is_empty(),
                    ),
                    (CLOSE, ButtonKind::Text, &[plain(Key::Esc)], true),
                ],
            );
            ui.add_space(space::MD);
            material_hint(ui, &palette, "Up/Down navigate * Enter restore * Esc close");
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
