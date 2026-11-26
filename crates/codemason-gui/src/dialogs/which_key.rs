//! The which-key popup: what each key does after the leader.
//!
//! While the leader key waits for its second key and has waited at least
//! [`WHICH_KEY_DELAY`], [`show`] lists every key bound after the leader with
//! its action's title, at the bottom of the window. It takes no input: the
//! next key press ends the wait whether the popup shows or not.
//!
//! The popup is a Material menu surface: [`menu_frame`] with the title over
//! its items, and one [`menu_item`] per binding carrying the action's title
//! with its key as the trailing text, as a menu shows a shortcut. The action
//! leads so it has the room, and every key lines up at the surface's
//! trailing edge.
//!
//! Dependencies: `egui`, and this crate's Material menu.

use egui::Id;

use crate::material::label_large;
use crate::material::menu::{menu_frame, menu_item};
use crate::metrics::space;
use crate::palette::{Palette, rgb};

/// How long the leader waits before the popup shows, in seconds.
pub const WHICH_KEY_DELAY: f64 = 0.4;

/// Title and accessible label of the popup.
pub const WHICH_KEY_TITLE: &str = "Leader keys";

/// Widest the popup grows before its titles are cut, in points.
const MAX_WIDTH: f32 = 320.0;

/// Draw the popup listing `entries` (key, title).
pub fn show(ctx: &egui::Context, palette: &Palette, entries: &[(char, String)]) {
    egui::Area::new(Id::unique("codemason_which_key"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -48.0))
        .show(ctx, |ui| {
            let surface = menu_frame(palette).show(ui, |ui| {
                ui.set_max_width(MAX_WIDTH);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(space::MD);
                        ui.label(
                            egui::RichText::new(WHICH_KEY_TITLE)
                                .font(label_large(ui.ctx()))
                                .color(rgb(palette.m3.on_surface_variant)),
                        );
                    });
                    ui.add_space(space::SM);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (key, title) in entries {
                        let chord = key.to_string();
                        menu_item(ui, palette, None, title, Some(&chord));
                    }
                });
            });
            surface.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Dialog, true, WHICH_KEY_TITLE)
            });
        });
}
