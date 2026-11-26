//! Material Design 3 menus: the menu surface and its items.
//!
//! [`menu_frame`] is the frame a menu's items are drawn in (a
//! `surface_container` fill at elevation 2 with extra-small corners), and
//! [`menu_item`] draws one 48-point item with an optional leading icon and
//! trailing text such as a shortcut, and [`menu_item_selected`] draws one as
//! the menu's current row, the one a key press runs.
//!
//! Dependencies: `egui`, and this crate's palette, icons and metrics.

use egui::{CornerRadius, Frame, Margin, Response, Role, Sense, Vec2, WidgetInfo};

use super::{content_colour, elevation_shadow, icon_galley, paint_state_layer, text_galley};
use crate::icons::Icon;
use crate::metrics::{radius, text_size};
use crate::palette::{Palette, rgb};

/// Height of a menu item, in points.
pub const ITEM_HEIGHT: f32 = 48.0;

/// Height of a dense menu item, in points: the desktop context menu, where
/// 48-point rows would make a long menu taller than it needs to be.
pub const DENSE_ITEM_HEIGHT: f32 = 32.0;

/// Narrowest menu item, in points.
const MIN_WIDTH: f32 = 112.0;

/// Horizontal padding inside an item, in points.
const PADDING: f32 = 12.0;

/// Size of a leading icon, in points.
const ICON_SIZE: f32 = 24.0;

/// Gap between an item's parts, in points.
const GAP: f32 = 12.0;

/// The frame of a menu surface.
///
/// # Returns
///
/// A `surface_container` frame with extra-small corners, 8 points of
/// vertical padding and the elevation-2 shadow.
#[must_use]
pub fn menu_frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(rgb(palette.m3.surface_container))
        .corner_radius(CornerRadius::same(radius::XS))
        .inner_margin(Margin::symmetric(0, 8))
        .shadow(elevation_shadow(2, palette))
}

/// Draw a menu item.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the item fills its available width (at least
///   112 points).
/// * `palette` - the colours.
/// * `icon` - an optional leading icon.
/// * `label` - the item's text, also its accessible name.
/// * `trailing` - optional trailing text, e.g. a keyboard shortcut.
///
/// # Returns
///
/// The item's response; `clicked()` is true when it was chosen.
pub fn menu_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    trailing: Option<&str>,
) -> Response {
    menu_item_at(ui, palette, icon, label, trailing, false)
}

/// Draw a menu item as the menu's current row: the one the keyboard would
/// run, filled like a selected list item.
///
/// The arguments and the returned response are those of [`menu_item`].
pub fn menu_item_selected(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    trailing: Option<&str>,
) -> Response {
    menu_item_at(ui, palette, icon, label, trailing, true)
}

/// Draw a menu item, highlighted when `selected`.
fn menu_item_at(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    trailing: Option<&str>,
    selected: bool,
) -> Response {
    let m3 = &palette.m3;
    let content = content_colour(ui, palette, rgb(m3.on_surface));
    let variant = content_colour(ui, palette, rgb(m3.on_surface_variant));
    let font = egui::FontId::proportional(text_size::BODY);
    let text = text_galley(ui, label, font.clone(), content);
    let trail = trailing.map(|t| text_galley(ui, t, font.clone(), variant));
    let icon_width = if icon.is_some() { ICON_SIZE + GAP } else { 0.0 };
    let trail_width = trail.as_ref().map_or(0.0, |t| t.size().x + GAP);
    let natural = PADDING.mul_add(2.0, icon_width + text.size().x + trail_width);
    let width = natural.max(MIN_WIDTH).max(ui.available_width());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ITEM_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::MenuItem, ui.is_enabled(), selected, label));
    // egui passes `selected` on for tabs and options only; a menu item states
    // it itself, so assistive technology hears the menu's current row. The
    // trailing shortcut is painted, not added, so it is the row's description
    // (as a list item's trailing text is).
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_selected(selected);
        if let Some(trailing) = trailing {
            node.set_description(trailing);
        }
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    if selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, rgb(m3.secondary_container));
    } else {
        paint_state_layer(ui, &response, CornerRadius::ZERO, content);
    }
    let mut x = rect.left() + PADDING;
    if let Some(icon) = icon {
        let glyph = icon_galley(ui, icon, ICON_SIZE, variant);
        let y = rect.center().y - glyph.size().y / 2.0;
        ui.painter().galley(egui::pos2(x, y), glyph, variant);
        x += icon_width;
    }
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), text, content);
    if let Some(trail) = trail {
        let pos = egui::pos2(
            rect.right() - PADDING - trail.size().x,
            rect.center().y - trail.size().y / 2.0,
        );
        ui.painter().galley(pos, trail, variant);
    }
    response
}
