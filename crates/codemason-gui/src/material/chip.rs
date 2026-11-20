//! Material Design 3 assist and filter chips.
//!
//! [`chip`] draws one: 32 points high with small (8 point) corners. An
//! assist chip and an unselected filter chip have an `outline_variant`
//! border; a selected filter chip is filled with `secondary_container` and
//! shows a check icon. A status chip shows a service's state: a green check
//! while it is on (filled like a selected filter chip, or with
//! `error_container` for a caution service) and a red cross while it is off,
//! so it keeps one width in both states.
//! [`chip_dense`] draws the same chip at the M3 density of
//! [`DENSE_HEIGHT`], for rows as tight as the status bar.
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

/// Height of a dense chip, in points: one row of a text line, so a status
/// bar row keeps its height.
pub const DENSE_HEIGHT: f32 = 20.0;

/// Padding beside the label, in points.
const PADDING: f32 = 16.0;

/// Padding beside the label of a dense chip, in points.
const DENSE_PADDING: f32 = 8.0;

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
    /// A service's on/off state: a green check while on, a red cross while
    /// off.
    Status {
        /// Whether the service is on.
        on: bool,
        /// Whether the on state carries risk (it lets the agent act without
        /// asking); such a chip is filled with `error_container` while on.
        caution: bool,
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
    draw(
        ui,
        palette,
        kind,
        icon,
        label,
        HEIGHT,
        PADDING,
        Sense::click(),
    )
}

/// Draw a dense chip: [`chip`] at [`DENSE_HEIGHT`] with tighter padding.
///
/// A dense chip is click-only: it never takes the keyboard focus, because
/// the rows it is made for (the status bar) sit beside the prompt, and a
/// focused control there would share Enter with the prompt, which sends the
/// draft.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `palette` - the colours.
/// * `kind` - assist, filter or status, with its state.
/// * `icon` - an optional leading icon; a selected filter chip shows a
///   check instead, and a status chip its check or cross.
/// * `label` - the chip's text, also its accessible name.
///
/// # Returns
///
/// The chip's response; the caller flips a filter's state on a click.
pub fn chip_dense(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: ChipKind,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    draw(
        ui,
        palette,
        kind,
        icon,
        label,
        DENSE_HEIGHT,
        DENSE_PADDING,
        Sense::CLICK,
    )
}

/// Draw a chip `height` points high with `padding` beside its label,
/// responding to `sense`.
#[allow(clippy::too_many_arguments)] // The two public wrappers fix the last three.
fn draw(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: ChipKind,
    icon: Option<Icon>,
    label: &str,
    height: f32,
    padding: f32,
    sense: Sense,
) -> Response {
    let m3 = &palette.m3;
    let selected = matches!(
        kind,
        ChipKind::Filter { selected: true } | ChipKind::Status { on: true, .. }
    );
    let icon = match kind {
        ChipKind::Status { on: true, .. } | ChipKind::Filter { selected: true } => {
            Some(Icon::Check)
        }
        ChipKind::Status { on: false, .. } => Some(Icon::Cross),
        ChipKind::Filter { selected: false } | ChipKind::Assist => icon,
    };
    let content = content_colour(
        ui,
        palette,
        match kind {
            ChipKind::Filter { selected: true }
            | ChipKind::Status {
                on: true,
                caution: false,
            } => rgb(m3.on_secondary_container),
            ChipKind::Status {
                on: true,
                caution: true,
            } => rgb(m3.on_error_container),
            ChipKind::Filter { selected: false } | ChipKind::Status { on: false, .. } => {
                rgb(m3.on_surface_variant)
            }
            ChipKind::Assist => rgb(m3.on_surface),
        },
    );
    let text = text_galley(ui, label, label_large(ui.ctx()), content);
    let icon_size = ICON_SIZE.min(height - 6.0);
    let leading = if icon.is_some() {
        ICON_PADDING.min(padding)
    } else {
        padding
    };
    let icon_width = if icon.is_some() {
        icon_size + ICON_GAP.min(padding / 2.0)
    } else {
        0.0
    };
    let width = leading + icon_width + text.size().x + padding;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);
    response.widget_info(|| match kind {
        ChipKind::Filter { selected } | ChipKind::Status { on: selected, .. } => {
            WidgetInfo::selected(Role::CheckBox, ui.is_enabled(), selected, label)
        }
        ChipKind::Assist => WidgetInfo::labeled(Role::Button, ui.is_enabled(), label),
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let corner = CornerRadius::same(radius::SM);
    if selected {
        let container = match kind {
            ChipKind::Status { caution: true, .. } => m3.error_container,
            ChipKind::Status { caution: false, .. }
            | ChipKind::Filter { .. }
            | ChipKind::Assist => m3.secondary_container,
        };
        ui.painter().rect_filled(rect, corner, rgb(container));
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
        let tint = match kind {
            ChipKind::Status { on: true, .. } => content_colour(ui, palette, palette.success),
            ChipKind::Status { on: false, .. } => content_colour(ui, palette, palette.error),
            ChipKind::Filter { selected: true } => content,
            ChipKind::Filter { selected: false } | ChipKind::Assist => {
                content_colour(ui, palette, rgb(m3.primary))
            }
        };
        let glyph = icon_galley(ui, icon, icon_size, tint);
        let y = rect.center().y - glyph.size().y / 2.0;
        ui.painter().galley(egui::pos2(x, y), glyph, tint);
        x += icon_width;
    }
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), text, content);
    paint_focus_ring(ui, &response, corner, palette);
    response
}
