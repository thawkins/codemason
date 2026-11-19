//! The which-key popup: what each key does after the leader.
//!
//! While the leader key waits for its second key and has waited at least
//! [`WHICH_KEY_DELAY`], [`show`] lists every key bound after the leader with
//! its action's title, at the bottom of the window. It takes no input: the
//! next key press ends the wait whether the popup shows or not.
//!
//! Dependencies: `egui`.

use egui::{Id, RichText};

/// How long the leader waits before the popup shows, in seconds.
pub const WHICH_KEY_DELAY: f64 = 0.4;

/// Title and accessible label of the popup.
pub const WHICH_KEY_TITLE: &str = "Leader keys";

/// Draw the popup listing `entries` (key, title).
pub fn show(ctx: &egui::Context, entries: &[(char, String)]) {
    egui::Area::new(Id::unique("codemason_which_key"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -48.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let inner = ui.vertical(|ui| {
                    ui.label(RichText::new(WHICH_KEY_TITLE).strong());
                    egui::Grid::new("which_key_grid")
                        .num_columns(2)
                        .spacing(egui::vec2(
                            crate::metrics::space::MD,
                            crate::metrics::space::XS,
                        ))
                        .show(ui, |ui| {
                            for (key, title) in entries {
                                ui.label(RichText::new(key.to_string()).monospace().strong());
                                ui.label(title);
                                ui.end_row();
                            }
                        });
                });
                inner.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Dialog, true, WHICH_KEY_TITLE)
                });
            });
        });
}
