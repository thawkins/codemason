//! The input queue (spec `inputqueue`) and its Alt+Q queue-control menu.
//!
//! [`queue_ui`] lists the App's queued prompts above the prompt, oldest
//! first, and draws the queue-control menu (`Next`, `Stop`/`Resume`, `Clear`,
//! `Show`). Everything shown is App state; the menu's keys reach
//! `App::handle_key` through the key router (the App locks the prompt while it
//! is open, so every key is forwarded) and app-core runs them, as in the TUI.
//!
//! The menu is a Material menu surface ([`menu_frame`]) with one
//! [`menu_item`] per row app-core names; the row its cursor is on is
//! [`menu_item_selected`], and the empty-queue `Next` row is drawn disabled.
//! The rows are what the App's keys act on, so the menu carries no hit
//! targets of its own: a row is a label, not a button.
//!
//! The `Clear` row's `Clear the input queue?` confirmation is a Material
//! dialog like every other confirmation, drawn by `crate::dialogs`
//! (`ActiveDialog::QueueClear`).
//!
//! Dependencies: `codemason-app-core` (the queue state), `egui`, and this
//! crate's Material menu.

use codemason_app_core::app::App;
use egui::{Align2, Id, Role, Vec2, WidgetInfo};

use crate::material::label_large;
use crate::material::menu::{menu_frame, menu_item, menu_item_selected};
use crate::metrics::space;
use crate::palette::{palette_for, rgb};

/// Accessible label of the input queue list.
pub const QUEUE_LABEL: &str = "Input queue";

/// Accessible label of the queue-control menu.
pub const QUEUE_MENU_LABEL: &str = "Queue control";

/// Width of the queue-control menu, in points: wide enough for the longest
/// row and its title, and narrow enough not to crowd the prompt.
const MENU_WIDTH: f32 = 280.0;

/// Characters of a queued prompt shown in the list.
const PREVIEW_CHARS: usize = 80;

/// The marker before the highlighted row, as the TUI draws it.
fn marker(selected: bool) -> &'static str {
    if selected { "> " } else { "  " }
}

/// The first line of `text`, cut to [`PREVIEW_CHARS`] characters.
fn preview(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    let mut out: String = line.chars().take(PREVIEW_CHARS).collect();
    if line.chars().count() > PREVIEW_CHARS || text.lines().nth(1).is_some() {
        out.push_str("...");
    }
    out
}

/// Draw the input queue and the queue-control menu.
///
/// The `Clear the input queue?` confirmation is a Material dialog drawn by
/// `crate::dialogs` (`ActiveDialog::QueueClear`), so it carries the same
/// surface, action row and accessible role as every other confirmation.
///
/// # Arguments
///
/// * `ui` - the ui the queue list is drawn into; the menu floats over the
///   window.
/// * `app` - the App whose `session.input_queue` is shown.
pub fn queue_ui(ui: &mut egui::Ui, app: &App) {
    queue_list(ui, app);
    if app.queue_menu_open {
        queue_menu(ui.ctx(), app);
    }
}

/// The queued prompts, oldest first. While the menu's `Show` panel is open
/// its highlighted entry is marked.
fn queue_list(ui: &mut egui::Ui, app: &App) {
    let queue = &app.session.input_queue;
    if queue.is_empty() {
        return;
    }
    let highlighted = app
        .queue_show_open
        .then(|| app.queue_show_selected.min(queue.len() - 1));
    let list = ui.vertical(|ui| {
        ui.weak(format!("{} queued", queue.len()));
        for (i, entry) in queue.iter().enumerate() {
            let selected = highlighted == Some(i);
            ui.label(format!(
                "{}{}. {}",
                marker(selected),
                i + 1,
                preview(&entry.text)
            ));
        }
        if app.queue_show_open {
            let palette = palette_for(app, ui.ctx());
            crate::dialogs::material_hint(
                ui,
                &palette,
                "^/v move * Enter move up * Del remove * Esc close",
            );
        }
    });
    list.response
        .widget_info(|| WidgetInfo::labeled(Role::List, true, QUEUE_LABEL));
}

/// The Alt+Q queue-control menu: the four rows app-core names, with the
/// empty-queue `Next` row dimmed and never marked (FR-023).
///
/// The menu is a Material menu surface: its title leads, then one item per
/// row, the row app-core's cursor is on drawn as the menu's current row. The
/// rows carry no click action - app-core's keys run them - so each is a plain
/// item, and a disabled row is drawn disabled.
fn queue_menu(ctx: &egui::Context, app: &App) {
    let palette = palette_for(app, ctx);
    egui::Area::new(Id::unique("codemason_queue_menu"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            menu_frame(&palette).show(ui, |ui| {
                ui.set_max_width(MENU_WIDTH);
                let menu = ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(space::MD);
                        ui.label(
                            egui::RichText::new(format!(
                                "{QUEUE_MENU_LABEL} - {} pending",
                                app.input_queue_len()
                            ))
                            .font(label_large(ui.ctx()))
                            .color(rgb(palette.m3.on_surface_variant)),
                        );
                    });
                    ui.add_space(space::SM);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, label) in app.queue_menu_labels().into_iter().enumerate() {
                        let selectable = app.queue_menu_row_selectable(i);
                        let selected = selectable && i == app.queue_menu_selected;
                        ui.add_enabled_ui(selectable, |ui| {
                            if selected {
                                menu_item_selected(ui, &palette, None, label, None);
                            } else {
                                menu_item(ui, &palette, None, label, None);
                            }
                        });
                    }
                    ui.add_space(space::SM);
                    crate::dialogs::material_hint(
                        ui,
                        &palette,
                        "^/v move * Enter select * Esc close",
                    );
                });
                // The menu surface is the column the rows are in, so they
                // read as its items; the frame's own response is not their
                // ancestor.
                menu.response
                    .widget_info(|| WidgetInfo::labeled(Role::Menu, true, QUEUE_MENU_LABEL));
            });
        });
}
