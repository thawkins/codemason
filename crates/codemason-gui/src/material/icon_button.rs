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

/// Width and height of a dense icon button, in points: a toolbar row.
pub const DENSE_SIZE: f32 = 32.0;

/// Size of a dense button's icon, in points.
pub const DENSE_ICON_SIZE: f32 = 20.0;

/// Width and height of an inline icon button, in points: one text row.
pub const INLINE_SIZE: f32 = 18.0;

/// Size of an inline button's icon, in points: just above the body text.
pub const INLINE_ICON_SIZE: f32 = 16.0;

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
    draw(ui, palette, kind, icon, label, selected, SIZE, ICON_SIZE)
}

/// Draw an icon button at Material's dense size: a toolbar row's control.
///
/// The dense size is for a strip of controls over content, where a
/// [`SIZE`]-point target per control would cost more height than the
/// content it governs. It keeps a comfortable 32-point target and a 20-point
/// glyph.
///
/// # Arguments
///
/// As [`icon_button`]; the button is [`DENSE_SIZE`] square with a
/// [`DENSE_ICON_SIZE`] icon.
///
/// # Returns
///
/// The button's response, as [`icon_button`].
pub fn icon_button_dense(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: IconButtonKind,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
) -> Response {
    draw(
        ui,
        palette,
        kind,
        icon,
        label,
        selected,
        DENSE_SIZE,
        DENSE_ICON_SIZE,
    )
}

/// Draw an icon button small enough to sit on a one-row line of text.
///
/// The inline size is for a control that shares a row with text the reader
/// is typing or reading: the row keeps its own height rather than growing
/// to a larger target it does not need.
///
/// # Arguments
///
/// As [`icon_button`]; the button is [`INLINE_SIZE`] square with an
/// [`INLINE_ICON_SIZE`] icon.
///
/// # Returns
///
/// The button's response, as [`icon_button`].
pub fn icon_button_inline(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: IconButtonKind,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
) -> Response {
    draw(
        ui,
        palette,
        kind,
        icon,
        label,
        selected,
        INLINE_SIZE,
        INLINE_ICON_SIZE,
    )
}

/// Draw one icon button at `size` with an `icon_size` icon.
#[allow(clippy::too_many_arguments)]
fn draw(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: IconButtonKind,
    icon: Icon,
    label: &str,
    selected: Option<bool>,
    size: f32,
    icon_size: f32,
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
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    response.widget_info(|| match selected {
        Some(on) => WidgetInfo::selected(Role::Button, ui.is_enabled(), on, label),
        None => WidgetInfo::labeled(Role::Button, ui.is_enabled(), label),
    });
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same((size / 2.0) as u8);
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
        let glyph = icon_galley(ui, icon, icon_size, content);
        let pos = rect.center() - glyph.size() / 2.0;
        ui.painter().galley(pos, glyph, content);
        paint_focus_ring(ui, &response, radius, palette);
    }
    response.on_hover_text(label)
}
