//! Material Design 3 list items, in a dense size for desktop lists.
//!
//! [`list_item`] draws one row: a headline, an optional supporting line
//! under it and optional trailing text at the right edge. A selected item
//! is filled with `secondary_container`; hovering or pressing one draws the
//! state layer.
//!
//! [`dense_item`] draws a compact legend row for breakdowns: an optional
//! disclosure triangle, an optional colour swatch, a headline and trailing
//! value columns aligned at the right edge. An expandable row is clickable.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{Color32, CornerRadius, Response, Role, Sense, Shape, Stroke, Vec2, WidgetInfo};

use super::feedback::{SWATCH_RADIUS, SWATCH_SIZE, swatch_rect};
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

/// Height of a [`dense_item`], in points.
pub const DENSE_HEIGHT: f32 = 22.0;

/// Width of each trailing value column of a [`dense_item`], in points.
pub const DENSE_COLUMN: f32 = 52.0;

/// Width of the disclosure triangle's slot in a [`dense_item`], in points.
const DISCLOSURE_SLOT: f32 = 14.0;

/// Gap between a [`dense_item`]'s leading parts, in points.
const DENSE_GAP: f32 = 6.0;

/// Whether a [`dense_item`] opens to show rows under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disclosure {
    /// The row has nothing under it; it is not clickable.
    None,
    /// The row has rows under it, hidden; a click shows them.
    Collapsed,
    /// The row has rows under it, shown; a click hides them.
    Expanded,
}

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
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_selected(selected);
        // The painted second line and trailing text, for screen readers.
        let description: Vec<&str> = [supporting, trailing].into_iter().flatten().collect();
        if !description.is_empty() {
            node.set_description(description.join(", "));
        }
    });
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

/// Draw a dense legend row.
///
/// The row reads, left to right: `indent`, the disclosure triangle (its
/// slot is kept for [`Disclosure::None`] too, so headlines line up), the
/// `swatch` square when given, the headline (cut at the columns) and the
/// `trailing` values, each right-aligned in a [`DENSE_COLUMN`]-wide column
/// at the right edge. The accessible name is the headline followed by the
/// values.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the row fills its available width.
/// * `palette` - the colours.
/// * `indent` - space before the row's content, in points.
/// * `disclosure` - whether the row opens; an openable row is clickable.
/// * `swatch` - the colour of the leading swatch, if any.
/// * `headline` - the row's text.
/// * `trailing` - the value columns, left to right.
///
/// # Returns
///
/// The row's response; `clicked()` reports a click on an openable row.
pub fn dense_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    indent: f32,
    disclosure: Disclosure,
    swatch: Option<Color32>,
    headline: &str,
    trailing: &[&str],
) -> Response {
    let m3 = &palette.m3;
    let sense = match disclosure {
        Disclosure::None => Sense::hover(),
        Disclosure::Collapsed | Disclosure::Expanded => Sense::click(),
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), DENSE_HEIGHT), sense);
    let name: String = std::iter::once(headline)
        .chain(trailing.iter().copied())
        .collect::<Vec<_>>()
        .join("  ");
    response.widget_info(|| WidgetInfo::labeled(Role::ListItem, ui.is_enabled(), &name));
    if disclosure != Disclosure::None {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_expanded(disclosure == Disclosure::Expanded);
        });
    }
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let content = content_colour(ui, palette, rgb(m3.on_surface));
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    if disclosure != Disclosure::None {
        paint_state_layer(ui, &response, CornerRadius::same(radius::XS), content);
    }
    let centre_y = rect.center().y;
    let mut x = rect.left() + indent;
    let painter = ui.painter();
    let size = 4.0;
    let tip = egui::pos2(x + DISCLOSURE_SLOT / 2.0, centre_y);
    let points = match disclosure {
        Disclosure::None => Vec::new(),
        Disclosure::Collapsed => vec![
            tip + Vec2::new(-size / 2.0, -size),
            tip + Vec2::new(size, 0.0),
            tip + Vec2::new(-size / 2.0, size),
        ],
        Disclosure::Expanded => vec![
            tip + Vec2::new(-size, -size / 2.0),
            tip + Vec2::new(size, -size / 2.0),
            tip + Vec2::new(0.0, size),
        ],
    };
    if !points.is_empty() {
        painter.add(Shape::convex_polygon(points, variant, Stroke::NONE));
    }
    x += DISCLOSURE_SLOT;
    if let Some(colour) = swatch {
        let square = swatch_rect(egui::pos2(x, centre_y - SWATCH_SIZE / 2.0));
        painter.rect_filled(square, CornerRadius::same(SWATCH_RADIUS), colour);
        x += SWATCH_SIZE + DENSE_GAP;
    }
    let columns_left = DENSE_COLUMN.mul_add(-(trailing.len() as f32), rect.right());
    let font = egui::FontId::proportional(text_size::SMALL + 1.0);
    let head = text_galley(ui, headline, font.clone(), content);
    let clip = rect.with_max_x((columns_left - DENSE_GAP).max(x));
    painter.with_clip_rect(clip).galley(
        egui::pos2(x, centre_y - head.size().y / 2.0),
        head,
        content,
    );
    for (index, value) in trailing.iter().enumerate() {
        let galley = text_galley(ui, value, font.clone(), variant);
        let right = DENSE_COLUMN.mul_add((index + 1) as f32, columns_left);
        let pos = egui::pos2(right - galley.size().x, centre_y - galley.size().y / 2.0);
        painter.galley(pos, galley, variant);
    }
    response
}
