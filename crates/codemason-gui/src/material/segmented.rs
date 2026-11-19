//! Material Design 3 segmented buttons, single-select.
//!
//! [`segmented_buttons`] draws a row of 40-point segments inside one
//! outlined, fully rounded border. The selected segment is filled with
//! `secondary_container` and shows a check icon before its label.
//!
//! Dependencies: `egui`, and this crate's palette and icons.

use egui::{CornerRadius, Rect, Response, Role, Sense, Stroke, StrokeKind, Vec2, WidgetInfo};

use super::{content_colour, icon_galley, label_large, paint_state_layer, text_galley};
use crate::icons::Icon;
use crate::palette::{Palette, rgb};

/// Height of the segments, in points.
pub const HEIGHT: f32 = 40.0;

/// Horizontal padding inside a segment, in points.
const PADDING: f32 = 12.0;

/// Narrowest segment, in points.
const MIN_WIDTH: f32 = 48.0;

/// Size of the selected segment's check icon, in points.
const ICON_SIZE: f32 = 18.0;

/// Gap between the check icon and the label, in points.
const ICON_GAP: f32 = 8.0;

/// Draw a single-select row of segmented buttons.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `palette` - the colours.
/// * `options` - the segment labels, left to right.
/// * `selected` - the index of the selected segment; a click on another
///   segment sets it.
///
/// # Returns
///
/// The union of the segments' responses, marked changed when the selection
/// changed.
pub fn segmented_buttons(
    ui: &mut egui::Ui,
    palette: &Palette,
    options: &[&str],
    selected: &mut usize,
) -> Response {
    let m3 = &palette.m3;
    let check_width = ICON_SIZE + ICON_GAP;
    let font = label_large();
    let widths: Vec<f32> = options
        .iter()
        .map(|label| {
            let text = text_galley(ui, label, font.clone(), rgb(m3.on_surface));
            PADDING
                .mul_add(2.0, text.size().x + check_width)
                .max(MIN_WIDTH)
        })
        .collect();
    let total = Vec2::new(widths.iter().sum(), HEIGHT);
    let (outer, mut union) = ui.allocate_exact_size(total, Sense::hover());
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    let mut x = outer.left();
    let last = options.len().saturating_sub(1);
    for (index, (label, width)) in options.iter().zip(&widths).enumerate() {
        let rect = Rect::from_min_size(egui::pos2(x, outer.top()), Vec2::new(*width, HEIGHT));
        x += width;
        let response = ui.interact(rect, ui.scope_id().with(("segment", index)), Sense::click());
        if response.clicked() && *selected != index {
            *selected = index;
            union.mark_changed();
        }
        let on = *selected == index;
        response
            .widget_info(|| WidgetInfo::selected(Role::RadioButton, ui.is_enabled(), on, *label));
        let segment_radius = CornerRadius {
            nw: if index == 0 { radius.nw } else { 0 },
            sw: if index == 0 { radius.sw } else { 0 },
            ne: if index == last { radius.ne } else { 0 },
            se: if index == last { radius.se } else { 0 },
        };
        let content = content_colour(
            ui,
            palette,
            if on {
                rgb(m3.on_secondary_container)
            } else {
                rgb(m3.on_surface)
            },
        );
        if on {
            ui.painter()
                .rect_filled(rect, segment_radius, rgb(m3.secondary_container));
        }
        paint_state_layer(ui, &response, segment_radius, content);
        if index > 0 {
            ui.painter().vline(
                rect.left(),
                rect.y_range(),
                Stroke::new(1.0, rgb(m3.outline)),
            );
        }
        let text = text_galley(ui, label, font.clone(), content);
        let content_width = text.size().x + if on { check_width } else { 0.0 };
        let mut cx = rect.center().x - content_width / 2.0;
        if on {
            let glyph = icon_galley(ui, Icon::Check, ICON_SIZE, content);
            let y = rect.center().y - glyph.size().y / 2.0;
            ui.painter().galley(egui::pos2(cx, y), glyph, content);
            cx += check_width;
        }
        let y = rect.center().y - text.size().y / 2.0;
        ui.painter().galley(egui::pos2(cx, y), text, content);
        union = union.union(response);
    }
    ui.painter().rect_stroke(
        outer,
        radius,
        Stroke::new(1.0, rgb(m3.outline)),
        StrokeKind::Inside,
    );
    union
}
