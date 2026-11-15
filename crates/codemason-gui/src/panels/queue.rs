//! The input queue (spec `inputqueue`) and its Alt+Q queue-control menu.
//!
//! [`queue_ui`] lists the App's queued prompts above the prompt, oldest
//! first, and draws the overlays the queue-control menu opens: the menu
//! itself (`Next`, `Stop`/`Resume`, `Clear`, `Show`) and the
//! `Clear the input queue?` confirmation. Everything shown is App state;
//! the overlays' keys reach `App::handle_key` through the key router (the
//! App locks the prompt while one is open, so every key is forwarded) and
//! app-core runs them, as in the TUI.
//!
//! Dependencies: `codemason-app-core` (the queue state) and `egui`.

use codemason_app_core::app::{App, QUEUE_CLEAR_CONFIRM_YES};
use egui::{Align2, Id, Role, Vec2, WidgetInfo};

/// Accessible label of the input queue list.
pub const QUEUE_LABEL: &str = "Input queue";

/// Accessible label of the queue-control menu.
pub const QUEUE_MENU_LABEL: &str = "Queue control";

/// Accessible label of the `Clear the input queue?` confirmation.
pub const QUEUE_CLEAR_LABEL: &str = "Clear the input queue?";

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

/// Draw the input queue and the queue-control overlays.
///
/// # Arguments
///
/// * `ui` - the ui the queue list is drawn into; the overlays float over
///   the window.
/// * `app` - the App whose `session.input_queue` is shown.
pub fn queue_ui(ui: &mut egui::Ui, app: &App) {
    queue_list(ui, app);
    if app.queue_menu_open {
        queue_menu(ui.ctx(), app);
    }
    if app.queue_clear_confirm_open {
        clear_confirm(ui.ctx(), app);
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
            ui.weak("^/v move * Enter move up * Del remove * Esc close");
        }
    });
    list.response
        .widget_info(|| WidgetInfo::labeled(Role::List, true, QUEUE_LABEL));
}

/// A centred overlay holding `add_contents`, labelled `label`.
fn overlay(ctx: &egui::Context, id: &str, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(Id::unique(id))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let menu = ui.vertical(add_contents);
                menu.response
                    .widget_info(|| WidgetInfo::labeled(Role::Menu, true, label));
            });
        });
}

/// The Alt+Q queue-control menu: the four rows app-core names, with the
/// empty-queue `Next` row dimmed and never marked (FR-023).
fn queue_menu(ctx: &egui::Context, app: &App) {
    overlay(ctx, "codemason_queue_menu", QUEUE_MENU_LABEL, |ui| {
        ui.strong(format!(
            "{QUEUE_MENU_LABEL} - {} pending",
            app.input_queue_len()
        ));
        for (i, label) in app.queue_menu_labels().into_iter().enumerate() {
            let selectable = app.queue_menu_row_selectable(i);
            let selected = selectable && i == app.queue_menu_selected;
            ui.add_enabled(
                selectable,
                egui::Label::new(format!("{}{label}", marker(selected))),
            );
        }
        ui.weak("^/v move * Enter select * Esc close");
    });
}

/// The `Clear the input queue?` confirmation, `No` selected by default.
fn clear_confirm(ctx: &egui::Context, app: &App) {
    overlay(ctx, "codemason_queue_clear", QUEUE_CLEAR_LABEL, |ui| {
        ui.strong(QUEUE_CLEAR_LABEL);
        let yes = app.queue_clear_confirm_selected == QUEUE_CLEAR_CONFIRM_YES;
        ui.label(format!("{}Yes", marker(yes)));
        ui.label(format!("{}No", marker(!yes)));
    });
}
