//! The Material Design 3 basic dialog: its surface, headline, body text
//! and action row.
//!
//! A dialog is drawn in [`dialog_frame`] (a `surface_container_high` fill
//! with extra-large corners at elevation 3) as [`dialog_headline`], then
//! [`dialog_body`], then [`dialog_actions`], which right-aligns its
//! buttons.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Frame, InnerResponse, Layout, Margin};

use super::elevation_shadow;
use crate::metrics::{radius, space, text_size};
use crate::palette::{Palette, rgb};

/// Padding inside the dialog, in points.
const PADDING: i8 = 24;

/// Size of the headline, in points (M3 "headline small").
pub const HEADLINE_SIZE: f32 = 24.0;

/// The frame of a dialog.
///
/// # Returns
///
/// A `surface_container_high` frame with extra-large corners, 24 points of
/// padding and the elevation-3 shadow.
#[must_use]
pub fn dialog_frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(rgb(palette.m3.surface_container_high))
        .corner_radius(CornerRadius::same(radius::XL))
        .inner_margin(Margin::same(PADDING))
        .shadow(elevation_shadow(3, palette))
}

/// Draw a dialog's headline, followed by the gap before its body.
pub fn dialog_headline(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(HEADLINE_SIZE)
            .color(rgb(palette.m3.on_surface)),
    );
    ui.add_space(space::LG);
}

/// Draw a dialog's body text, wrapped, followed by the gap before its
/// actions.
pub fn dialog_body(ui: &mut egui::Ui, palette: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(text_size::BODY)
            .color(rgb(palette.m3.on_surface_variant)),
    );
    ui.add_space(space::XL);
}

/// Draw a dialog's action row, one button high and as wide as the dialog:
/// `add` lays out its buttons right to left, so the first button added is
/// the rightmost (the confirming action).
///
/// # Returns
///
/// What `add` returned.
pub fn dialog_actions<R>(
    ui: &mut egui::Ui,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> InnerResponse<R> {
    let size = egui::Vec2::new(ui.available_width(), super::button::HEIGHT);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = space::SM;
        add(ui)
    })
}
