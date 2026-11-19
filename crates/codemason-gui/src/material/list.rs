//! Material Design 3 list items, in a dense size for desktop lists.
//!
//! [`list_item`] draws one row: a headline, an optional supporting line
//! under it and optional trailing text at the right edge. A selected item
//! is filled with `secondary_container`; hovering or pressing one draws the
//! state layer.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Response, Role, Sense, Vec2, WidgetInfo};

use super::{content_colour, paint_state_layer, text_galley};
use crate::metrics::{radius, text_size};
use crate::palette::{Palette, rgb};

/// Height of a one-line item, in points.
pub const ONE_LINE_HEIGHT: f32 = 40.0;

/// Height of a two-line item (headline and supporting text), in points.
pub const TWO_LINE_HEIGHT: f32 = 56.0;

/// Horizontal padding inside an item, in points.
const PADDING: f32 = 16.0;

/// Smallest gap between the text and the trailing text, in points.
const TRAILING_GAP: f32 = 16.0;

/// Draw a list item.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the item fills its available width.
/// * `palette` - the colours.
/// * `headline` - the item's text, also its accessible name.
/// * `supporting` - an optional second line under the headline.
/// * `trailing` - optional trailing text, e.g. a size or a price.
/// * `selected` - whether the item is the selected one.
///
/// # Returns
///
/// The item's response; `clicked()` and `double_clicked()` report clicks.
pub fn list_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    headline: &str,
    supporting: Option<&str>,
    trailing: Option<&str>,
    selected: bool,
) -> Response {
    let m3 = &palette.m3;
    let height = if supporting.is_some() {
        TWO_LINE_HEIGHT
    } else {
        ONE_LINE_HEIGHT
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(Role::ListItem, ui.is_enabled(), selected, headline));
    // egui passes `selected` on for tabs and options only; a list item
    // states it itself, so assistive technology hears the selected row.
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_selected(selected));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let content = content_colour(
        ui,
        palette,
        rgb(if selected {
            m3.on_secondary_container
        } else {
            m3.on_surface
        }),
    );
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    let corner = CornerRadius::same(radius::SM);
    if selected {
        ui.painter()
            .rect_filled(rect, corner, rgb(m3.secondary_container));
    }
    paint_state_layer(ui, &response, corner, content);
    let trail =
        trailing.map(|t| text_galley(ui, t, egui::FontId::proportional(text_size::SMALL), variant));
    let trail_width = trail.as_ref().map_or(0.0, |t| t.size().x + TRAILING_GAP);
    let text_right = rect.right() - PADDING - trail_width;
    let clip = rect.with_max_x(text_right.max(rect.left()));
    let head = text_galley(
        ui,
        headline,
        egui::FontId::proportional(text_size::BODY),
        content,
    );
    let sub = supporting
        .map(|s| text_galley(ui, s, egui::FontId::proportional(text_size::SMALL), variant));
    let block = head.size().y + sub.as_ref().map_or(0.0, |s| s.size().y);
    let mut y = rect.center().y - block / 2.0;
    let x = rect.left() + PADDING;
    let painter = ui.painter().with_clip_rect(clip);
    let head_height = head.size().y;
    painter.galley(egui::pos2(x, y), head, content);
    y += head_height;
    if let Some(sub) = sub {
        painter.galley(egui::pos2(x, y), sub, variant);
    }
    if let Some(trail) = trail {
        let pos = egui::pos2(
            rect.right() - PADDING - trail.size().x,
            rect.center().y - trail.size().y / 2.0,
        );
        ui.painter().galley(pos, trail, variant);
    }
    response
}
