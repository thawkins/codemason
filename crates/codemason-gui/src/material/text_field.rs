//! The Material Design 3 outlined text field, single line.
//!
//! [`outlined_text_field`] draws a 56-point field with a 1-point `outline`
//! border (2 points of `primary` while focused, `error` in the error state)
//! and a label. The label sits inside the field while it is empty and
//! unfocused, and moves into a gap in the top border otherwise
//! ([`label_floats`]). Optional supporting text goes under the field.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Frame, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextEdit, Vec2};

use super::{content_colour, text_galley};
use crate::metrics::{radius, text_size};
use crate::palette::{Palette, rgb};

/// Height of the field, in points.
pub const HEIGHT: f32 = 56.0;

/// Horizontal padding inside the field, in points.
const PADDING: f32 = 16.0;

/// Size of the input text and of the resting label, in points.
const INPUT_SIZE: f32 = 16.0;

/// Size of the floating label and of the supporting text, in points.
const SMALL_SIZE: f32 = 12.0;

/// Space around the floating label in the border gap, in points.
const LABEL_GAP: f32 = 4.0;

/// Whether the label floats in the top border.
///
/// # Returns
///
/// `true` when the field has focus or holds text.
#[must_use]
pub const fn label_floats(focused: bool, empty: bool) -> bool {
    focused || !empty
}

/// Draw an outlined text field.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the field fills its available width.
/// * `palette` - the colours.
/// * `label` - the field's label, also its accessible name.
/// * `text` - the text being edited.
/// * `supporting` - optional supporting (or error) text under the field.
/// * `error` - whether the field is in the error state.
///
/// # Returns
///
/// The response of the text edit inside the field: `changed()` when the
/// text changed, `has_focus()` while it is being edited.
pub fn outlined_text_field(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    text: &mut String,
    supporting: Option<&str>,
    error: bool,
) -> Response {
    let m3 = &palette.m3;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    // The border is drawn once the edit's focus is known; reserve its place
    // under the edit now.
    let border_slot = ui.painter().add(Shape::Noop);
    let input = Rect::from_center_size(
        rect.center(),
        Vec2::new(PADDING.mul_add(-2.0, width).max(0.0), INPUT_SIZE + 8.0),
    );
    let on_surface = content_colour(ui, palette, rgb(m3.on_surface));
    let edit = TextEdit::singleline(text)
        .id_salt(("material_text_field", label))
        .frame(Frame::NONE)
        .font(egui::FontId::proportional(INPUT_SIZE))
        .text_color(on_surface)
        .desired_width(input.width());
    let response = ui.place(input, edit);
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    let focused = response.has_focus();
    let accent = if error {
        rgb(m3.error)
    } else if focused {
        rgb(m3.primary)
    } else {
        rgb(m3.outline)
    };
    let accent = content_colour(ui, palette, accent);
    let corner = CornerRadius::same(radius::XS);
    let border_width = if focused { 2.0 } else { 1.0 };
    ui.painter().set(
        border_slot,
        Shape::rect_stroke(
            rect,
            corner,
            Stroke::new(border_width, accent),
            StrokeKind::Inside,
        ),
    );
    let label_colour = if error || focused {
        accent
    } else {
        content_colour(ui, palette, rgb(m3.on_surface_variant))
    };
    if label_floats(focused, text.is_empty()) {
        let small = text_galley(
            ui,
            label,
            egui::FontId::proportional(SMALL_SIZE),
            label_colour,
        );
        let pos = egui::pos2(rect.left() + PADDING, rect.top() - small.size().y / 2.0);
        let gap = Rect::from_min_size(
            egui::pos2(pos.x - LABEL_GAP, rect.top() - border_width),
            Vec2::new(LABEL_GAP.mul_add(2.0, small.size().x), 2.0 * border_width),
        );
        ui.painter().rect_filled(gap, 0.0, ui.visuals().panel_fill);
        ui.painter().galley(pos, small, label_colour);
    } else {
        let resting = text_galley(
            ui,
            label,
            egui::FontId::proportional(INPUT_SIZE),
            label_colour,
        );
        let pos = egui::pos2(input.left(), rect.center().y - resting.size().y / 2.0);
        ui.painter().galley(pos, resting, label_colour);
    }
    if let Some(supporting) = supporting {
        let colour = if error {
            accent
        } else {
            content_colour(ui, palette, rgb(m3.on_surface_variant))
        };
        ui.horizontal(|ui| {
            ui.add_space(PADDING);
            ui.label(
                egui::RichText::new(supporting)
                    .size(text_size::SMALL)
                    .color(colour),
            );
        });
    }
    response
}
