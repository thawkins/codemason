//! The Material Design 3 outlined text field, single line.
//!
//! [`outlined_text_field`] draws a 56-point field with a 1-point `outline`
//! border (2 points of `primary` while focused, `error` in the error state)
//! and a label. The label sits inside the field while it is empty and
//! unfocused, and moves into a gap in the top border otherwise
//! ([`label_floats`]). Optional supporting text goes under the field.
//! [`outlined_value`] draws the same field read-only, for text another
//! component edits.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{
    CornerRadius, Frame, Id, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextEdit, Vec2,
};

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

/// The widget id of the editor inside the field labelled `label`.
///
/// # Returns
///
/// A window-wide id derived from the label, so a caller can focus the
/// field or read its `TextEdit` state; two fields shown at once need two
/// labels.
#[must_use]
pub fn field_id(label: &str) -> Id {
    Id::unique(("material_text_field", label))
}

/// Draw an outlined text field.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the field fills its available width. The
///   floating label's border gap is filled with `ui`'s `panel_fill`, so a
///   field on another surface sets that to the surface's colour.
/// * `palette` - the colours.
/// * `label` - the field's label, also its accessible name.
/// * `text` - the text being edited.
/// * `supporting` - optional supporting (or error) text under the field.
/// * `error` - whether the field is in the error state.
///
/// # Returns
///
/// The response of the text edit inside the field ([`field_id`]):
/// `changed()` when the text changed, `has_focus()` while it is being
/// edited.
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
    // The whole field takes clicks (not keyboard focus: Tab goes straight to
    // the editor), so a click on its padding edits it too.
    let (rect, field) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::CLICK);
    // The border is drawn once the edit's focus is known; reserve its place
    // under the edit now.
    let border_slot = ui.painter().add(Shape::Noop);
    let input = input_rect(rect);
    let on_surface = content_colour(ui, palette, rgb(m3.on_surface));
    let edit = TextEdit::singleline(text)
        .id(field_id(label))
        .frame(Frame::NONE)
        .font(egui::FontId::proportional(INPUT_SIZE))
        .text_color(on_surface)
        .desired_width(input.width());
    let response = ui.place(input, edit);
    if field.clicked() {
        ui.memory_mut(|m| m.request_focus(response.id));
    }
    field.on_hover_cursor(egui::CursorIcon::Text);
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    let look = Look {
        label,
        empty: text.is_empty(),
        focused: response.has_focus(),
        error,
        supporting,
    };
    decorate(ui, palette, rect, border_slot, &look);
    response
}

/// Draw a read-only outlined field showing `value`: the look of
/// [`outlined_text_field`] for text another component edits (the App's
/// key-driven fields, which typing reaches through the key router).
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the field fills its available width.
/// * `palette` - the colours.
/// * `label` - the field's label, also its accessible name.
/// * `value` - the text shown, as given (mask secrets before).
/// * `active` - whether typing goes to this field; drawn as focused.
/// * `supporting` - optional supporting (or error) text under the field.
/// * `error` - whether the field is in the error state.
///
/// # Returns
///
/// The field's response, labelled `label` with `value` as its value.
pub fn outlined_value(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    value: &str,
    active: bool,
    supporting: Option<&str>,
    error: bool,
) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::TextInput);
        node.set_label(label);
        node.set_value(value);
    });
    let border_slot = ui.painter().add(Shape::Noop);
    if !value.is_empty() {
        let colour = content_colour(ui, palette, rgb(palette.m3.on_surface));
        let text = text_galley(ui, value, egui::FontId::proportional(INPUT_SIZE), colour);
        let input = input_rect(rect);
        let pos = egui::pos2(input.left(), rect.center().y - text.size().y / 2.0);
        ui.painter()
            .with_clip_rect(input.expand2(Vec2::new(0.0, 4.0)))
            .galley(pos, text, colour);
    }
    let look = Look {
        label,
        empty: value.is_empty(),
        focused: active,
        error,
        supporting,
    };
    decorate(ui, palette, rect, border_slot, &look);
    response
}

/// Where the input text sits inside a field's `rect`.
fn input_rect(rect: Rect) -> Rect {
    Rect::from_center_size(
        rect.center(),
        Vec2::new(
            PADDING.mul_add(-2.0, rect.width()).max(0.0),
            INPUT_SIZE + 8.0,
        ),
    )
}

/// The state a field is drawn in.
struct Look<'a> {
    label: &'a str,
    empty: bool,
    focused: bool,
    error: bool,
    supporting: Option<&'a str>,
}

/// Paint a field's border (into `border_slot`), its label (resting or
/// floating) and its supporting text.
fn decorate(
    ui: &mut egui::Ui,
    palette: &Palette,
    rect: Rect,
    border_slot: egui::layers::ShapeIdx,
    look: &Look<'_>,
) {
    let m3 = &palette.m3;
    let accent = if look.error {
        rgb(m3.error)
    } else if look.focused {
        rgb(m3.primary)
    } else {
        rgb(m3.outline)
    };
    let accent = content_colour(ui, palette, accent);
    let corner = CornerRadius::same(radius::XS);
    let border_width = if look.focused { 2.0 } else { 1.0 };
    ui.painter().set(
        border_slot,
        Shape::rect_stroke(
            rect,
            corner,
            Stroke::new(border_width, accent),
            StrokeKind::Inside,
        ),
    );
    let label_colour = if look.error || look.focused {
        accent
    } else {
        content_colour(ui, palette, rgb(m3.on_surface_variant))
    };
    if label_floats(look.focused, look.empty) {
        let small = text_galley(
            ui,
            look.label,
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
            look.label,
            egui::FontId::proportional(INPUT_SIZE),
            label_colour,
        );
        let input = input_rect(rect);
        let pos = egui::pos2(input.left(), rect.center().y - resting.size().y / 2.0);
        ui.painter().galley(pos, resting, label_colour);
    }
    if let Some(supporting) = look.supporting {
        let colour = if look.error {
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
}
