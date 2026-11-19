//! Material Design 3 navigation drawer and side sheets (standard, not
//! modal).
//!
//! The drawer and a side sheet share one surface, [`sheet_frame`]
//! (`surface_container_low`). A drawer lists [`drawer_item`]s: 56-point,
//! fully rounded rows, the selected one filled with `secondary_container`.
//! A side sheet starts with [`sheet_header`]: its title and a close button.
//!
//! Dependencies: `egui`, and this crate's palette, icons and metrics.

use egui::{CornerRadius, Frame, Margin, Response, Role, Sense, Vec2, WidgetInfo};

use super::icon_button::{IconButtonKind, icon_button};
use super::{content_colour, icon_galley, label_large, paint_state_layer, text_galley};
use crate::icons::Icon;
use crate::metrics::{space, text_size};
use crate::palette::{Palette, rgb};

/// Height of a drawer item, in points.
pub const ITEM_HEIGHT: f32 = 56.0;

/// Padding inside a drawer item, in points.
const ITEM_PADDING: f32 = 16.0;

/// Size of a drawer item's icon, in points.
const ICON_SIZE: f32 = 24.0;

/// Gap between a drawer item's icon and label, in points.
const ICON_GAP: f32 = 12.0;

/// Padding inside a drawer or side sheet, in points.
const SHEET_PADDING: i8 = 12;

/// The frame of a navigation drawer or side sheet.
///
/// # Returns
///
/// A `surface_container_low` frame with 12 points of padding.
#[must_use]
pub fn sheet_frame(palette: &Palette) -> Frame {
    Frame::new()
        .fill(rgb(palette.m3.surface_container_low))
        .inner_margin(Margin::same(SHEET_PADDING))
}

/// Draw a navigation drawer item.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the item fills its available width.
/// * `palette` - the colours.
/// * `icon` - an optional leading icon.
/// * `label` - the item's text, also its accessible name.
/// * `selected` - whether this is the current destination.
///
/// # Returns
///
/// The item's response; `clicked()` is true when it was chosen.
pub fn drawer_item(
    ui: &mut egui::Ui,
    palette: &Palette,
    icon: Option<Icon>,
    label: &str,
    selected: bool,
) -> Response {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), ITEM_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let radius = CornerRadius::same((ITEM_HEIGHT / 2.0) as u8);
    let content = content_colour(
        ui,
        palette,
        if selected {
            rgb(m3.on_secondary_container)
        } else {
            rgb(m3.on_surface_variant)
        },
    );
    if selected {
        ui.painter()
            .rect_filled(rect, radius, rgb(m3.secondary_container));
    }
    paint_state_layer(ui, &response, radius, content);
    let mut x = rect.left() + ITEM_PADDING;
    if let Some(icon) = icon {
        let glyph = icon_galley(ui, icon, ICON_SIZE, content);
        let y = rect.center().y - glyph.size().y / 2.0;
        ui.painter().galley(egui::pos2(x, y), glyph, content);
        x += ICON_SIZE + ICON_GAP;
    }
    let text = text_galley(ui, label, label_large(ui.ctx()), content);
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), text, content);
    response
}

/// Draw a side sheet's header: its title, and a close button at the right.
///
/// # Returns
///
/// `true` when the close button was clicked.
pub fn sheet_header(ui: &mut egui::Ui, palette: &Palette, title: &str) -> bool {
    let closed = ui
        .horizontal(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(text_size::TITLE)
                    .color(rgb(palette.m3.on_surface_variant)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                icon_button(
                    ui,
                    palette,
                    IconButtonKind::Standard,
                    Icon::Cross,
                    "Close",
                    None,
                )
                .clicked()
            })
            .inner
        })
        .inner;
    ui.add_space(space::SM);
    closed
}

/// Restyle egui's own selectable rows as navigation drawer items, for a
/// pane that draws its rows with egui widgets (the sessions sidebar): pill
/// corners, wider horizontal padding, and the selected row on
/// `secondary_container`.
///
/// # Arguments
///
/// * `style` - the pane's style, changed in place.
/// * `palette` - the colours.
pub fn drawer_style(style: &mut egui::Style, palette: &Palette) {
    let m3 = &palette.m3;
    let pill = CornerRadius::same((ITEM_HEIGHT / 2.0) as u8);
    let widgets = &mut style.visuals.widgets;
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = pill;
    }
    style.visuals.selection.bg_fill = rgb(m3.secondary_container);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, rgb(m3.on_secondary_container));
    // Only the horizontal padding grows: the sessions list pages and
    // scrolls by a fixed row height that assumes egui's vertical padding.
    style.spacing.button_padding.x = ITEM_PADDING;
}
