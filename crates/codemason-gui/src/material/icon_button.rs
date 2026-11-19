//! Material Design 3 icon buttons: standard and filled, each optionally a
//! toggle.
//!
//! [`icon_button`] draws one: a 40-point circle around a 24-point icon. The
//! icon has no visible text, so the accessible name is passed separately and
//! is also shown as the hover text.
//!
//! Dependencies: `egui`, and this crate's palette and icons.

use egui::{CornerRadius, Response, Role, Sense, Vec2, WidgetInfo};

use super::{
    DISABLED_CONTAINER_OPACITY, content_colour, icon_galley, paint_focus_ring, paint_state_layer,
};
use crate::icons::Icon;
use crate::palette::{Palette, rgb};

/// Width and height of an icon button, in points.
pub const SIZE: f32 = 40.0;

/// Size of the icon, in points.
const ICON_SIZE: f32 = 24.0;

/// The style of an icon button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconButtonKind {
    /// No container; the icon in `on_surface_variant`.
    Standard,
    /// A `primary` container with the icon in `on_primary`.
    Filled,
}

/// Draw an icon button.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; a disabled ui draws a disabled button.
/// * `palette` - the colours.
/// * `kind` - standard or filled.
/// * `icon` - the icon.
/// * `label` - the accessible name and hover text.
/// * `selected` - `None` for a plain button; `Some(on)` for a toggle, drawn
///   in its selected colours when `on`.
///
/// # Returns
///
/// The button's response, with its hover text; `clicked()` is true when it
/// was pressed. A toggle's caller flips its own state on a click.
pub fn icon_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: IconButtonKind,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
) -> Response {
    let m3 = &palette.m3;
    let on = selected.unwrap_or(true);
    let (fill, content) = match (kind, on) {
        (IconButtonKind::Standard, true) if selected.is_some() => (None, rgb(m3.primary)),
        (IconButtonKind::Standard, _) => (None, rgb(m3.on_surface_variant)),
        (IconButtonKind::Filled, true) => (Some(rgb(m3.primary)), rgb(m3.on_primary)),
        (IconButtonKind::Filled, false) => {
            (Some(rgb(m3.surface_container_highest)), rgb(m3.primary))
        }
    };
    let content = content_colour(ui, palette, content);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SIZE), Sense::click());
    response.widget_info(|| match selected {
        Some(on) => WidgetInfo::selected(Role::Button, ui.is_enabled(), on, label),
        None => WidgetInfo::labeled(Role::Button, ui.is_enabled(), label),
    });
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same((SIZE / 2.0) as u8);
        match fill {
            Some(fill) if ui.is_enabled() => {
                ui.painter().rect_filled(rect, radius, fill);
            }
            Some(_) => {
                ui.painter().rect_filled(
                    rect,
                    radius,
                    rgb(m3.on_surface).gamma_multiply(DISABLED_CONTAINER_OPACITY),
                );
            }
            None => {}
        }
        paint_state_layer(ui, &response, radius, content);
        let glyph = icon_galley(ui, icon, ICON_SIZE, content);
        let pos = rect.center() - glyph.size() / 2.0;
        ui.painter().galley(pos, glyph, content);
        paint_focus_ring(ui, &response, radius, palette);
    }
    response.on_hover_text(label)
}
