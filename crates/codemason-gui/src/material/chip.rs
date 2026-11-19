//! Material Design 3 assist and filter chips.
//!
//! [`chip`] draws one: 32 points high with small (8 point) corners. An
//! assist chip and an unselected filter chip have an `outline_variant`
//! border; a selected filter chip is filled with `secondary_container` and
//! shows a check icon.
//!
//! Dependencies: `egui`, and this crate's palette and icons.

use egui::{CornerRadius, Response, Role, Sense, Stroke, StrokeKind, Vec2, WidgetInfo};

use super::{
    content_colour, icon_galley, label_large, paint_focus_ring, paint_state_layer, text_galley,
};
use crate::icons::Icon;
use crate::metrics::radius;
use crate::palette::{Palette, rgb};

/// Height of a chip, in points.
pub const HEIGHT: f32 = 32.0;

/// Padding beside the label, in points.
const PADDING: f32 = 16.0;

/// Padding before a leading icon, in points.
const ICON_PADDING: f32 = 8.0;

/// Size of a leading icon, in points.
const ICON_SIZE: f32 = 18.0;

/// Gap between the icon and the label, in points.
const ICON_GAP: f32 = 8.0;

/// The kind of a chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    /// A suggested action.
    Assist,
    /// A filter that is on or off.
    Filter {
        /// Whether the filter is on.
        selected: bool,
    },
}

/// Draw a chip.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `palette` - the colours.
/// * `kind` - assist, or filter with its state.
/// * `icon` - an optional leading icon; a selected filter chip shows a
///   check instead.
/// * `label` - the chip's text, also its accessible name.
///
/// # Returns
///
/// The chip's response; a filter chip's caller flips its state on a click.
pub fn chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: ChipKind,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    let m3 = &palette.m3;
    let selected = matches!(kind, ChipKind::Filter { selected: true });
    let icon = if selected { Some(Icon::Check) } else { icon };
    let content = content_colour(
        ui,
        palette,
        match kind {
            ChipKind::Filter { selected: true } => rgb(m3.on_secondary_container),
            ChipKind::Filter { selected: false } => rgb(m3.on_surface_variant),
            ChipKind::Assist => rgb(m3.on_surface),
        },
    );
    let text = text_galley(ui, label, label_large(ui.ctx()), content);
    let leading = if icon.is_some() {
        ICON_PADDING
    } else {
        PADDING
    };
    let icon_width = if icon.is_some() {
        ICON_SIZE + ICON_GAP
    } else {
        0.0
    };
    let width = leading + icon_width + text.size().x + PADDING;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::click());
    response.widget_info(|| match kind {
        ChipKind::Filter { selected } => {
            WidgetInfo::selected(Role::CheckBox, ui.is_enabled(), selected, label)
        }
        ChipKind::Assist => WidgetInfo::labeled(Role::Button, ui.is_enabled(), label),
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let corner = CornerRadius::same(radius::SM);
    if selected {
        ui.painter()
            .rect_filled(rect, corner, rgb(m3.secondary_container));
    } else {
        ui.painter().rect_stroke(
            rect,
            corner,
            Stroke::new(1.0, rgb(m3.outline_variant)),
            StrokeKind::Inside,
        );
    }
    paint_state_layer(ui, &response, corner, content);
    let mut x = rect.left() + leading;
    if let Some(icon) = icon {
        let tint = if selected {
            content
        } else {
            content_colour(ui, palette, rgb(m3.primary))
        };
        let glyph = icon_galley(ui, icon, ICON_SIZE, tint);
        let y = rect.center().y - glyph.size().y / 2.0;
        ui.painter().galley(egui::pos2(x, y), glyph, tint);
        x += icon_width;
    }
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), text, content);
    paint_focus_ring(ui, &response, corner, palette);
    response
}
