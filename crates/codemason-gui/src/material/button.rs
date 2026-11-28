//! Material Design 3 common buttons: filled, tonal, outlined and text.
//!
//! [`button`] draws one: 40 points high, fully rounded, a label in the
//! "label large" font and an optional leading icon. A disabled ui draws it
//! in the disabled colours and does not report clicks.
//!
//! Dependencies: `egui`, and this crate's palette and icons.

use egui::{CornerRadius, Response, Role, Sense, Stroke, StrokeKind, Vec2, WidgetInfo};

use super::{
    DISABLED_CONTAINER_OPACITY, content_colour, icon_galley, label_large, paint_focus_ring,
    paint_state_layer, text_galley,
};
use crate::icons::Icon;
use crate::palette::{Palette, rgb};

/// Height of a button, in points.
pub const HEIGHT: f32 = 40.0;

/// Horizontal padding beside the label, in points.
const PADDING: f32 = 24.0;

/// Padding before a leading icon, in points.
const ICON_PADDING: f32 = 16.0;

/// Size of a leading icon, in points.
const ICON_SIZE: f32 = 18.0;

/// Gap between the icon and the label, in points.
const ICON_GAP: f32 = 8.0;

/// The emphasis of a button, from highest to lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// The main action: `primary` fill.
    Filled,
    /// A secondary action that still stands out: `secondary_container` fill.
    Tonal,
    /// A medium-emphasis action: an `outline` border, no fill.
    Outlined,
    /// The lowest emphasis: no fill and no border.
    Text,
}

/// Draw a button.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; a disabled ui draws a disabled button.
/// * `palette` - the colours.
/// * `kind` - the button's emphasis.
/// * `icon` - an optional leading icon.
/// * `label` - the button's text, also its accessible name.
///
/// # Returns
///
/// The button's response; `clicked()` is true when it was pressed.
pub fn button(
    ui: &mut egui::Ui,
    palette: &Palette,
    kind: ButtonKind,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    let m3 = &palette.m3;
    let (fill, content) = match kind {
        ButtonKind::Filled => (Some(rgb(m3.primary)), rgb(m3.on_primary)),
        ButtonKind::Tonal => (
            Some(rgb(m3.secondary_container)),
            rgb(m3.on_secondary_container),
        ),
        ButtonKind::Outlined | ButtonKind::Text => (None, rgb(m3.primary)),
    };
    let content = content_colour(ui, palette, content);
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
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    let painter = ui.painter();
    match fill {
        Some(fill) if ui.is_enabled() => {
            painter.rect_filled(rect, radius, fill);
        }
        Some(_) => {
            painter.rect_filled(
                rect,
                radius,
                rgb(m3.on_surface).gamma_multiply(DISABLED_CONTAINER_OPACITY),
            );
        }
        None => {}
    }
    if kind == ButtonKind::Outlined {
        let outline = if ui.is_enabled() {
            rgb(m3.outline)
        } else {
            rgb(m3.on_surface).gamma_multiply(DISABLED_CONTAINER_OPACITY)
        };
        painter.rect_stroke(rect, radius, Stroke::new(1.0, outline), StrokeKind::Inside);
    }
    paint_state_layer(ui, &response, radius, content);
    let mut x = rect.left() + leading;
    if let Some(icon) = icon {
        let glyph = icon_galley(ui, icon, ICON_SIZE, content);
        let y = rect.center().y - glyph.size().y / 2.0;
        ui.painter().galley(egui::pos2(x, y), glyph, content);
        x += icon_width;
    }
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter().galley(egui::pos2(x, y), text, content);
    paint_focus_ring(ui, &response, radius, palette);
    response
}

/// Height of a [`small_button`], in points: one small text line and padding.
pub const SMALL_HEIGHT: f32 = 24.0;

/// Horizontal padding beside a small button's label, in points.
const SMALL_PADDING: f32 = 12.0;

/// Draw a small, click-only button for a toolbar or a row of actions (UI-13).
///
/// It is outlined, or filled `secondary_container` when it is a toggle in its
/// on state, with its label in the small text size. It senses clicks only and
/// so never takes the keyboard focus: a focusable button would share Enter
/// with the prompt, which sends the draft.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; a disabled ui draws a disabled button.
/// * `palette` - the colours.
/// * `label` - the button's text, also its accessible name.
/// * `selected` - `Some(on)` for a toggle, reported to accessibility as
///   toggled; `None` for a plain button.
///
/// # Returns
///
/// The button's response; `clicked()` is true when it was pressed.
pub fn small_button(
    ui: &mut egui::Ui,
    palette: &Palette,
    label: &str,
    selected: Option<bool>,
) -> Response {
    let m3 = &palette.m3;
    let on = selected == Some(true);
    let content = if on {
        rgb(m3.on_secondary_container)
    } else {
        rgb(m3.primary)
    };
    let content = content_colour(ui, palette, content);
    let font = super::label_font(ui.ctx(), crate::metrics::text_size::SMALL);
    let text = text_galley(ui, label, font, content);
    let width = SMALL_PADDING + text.size().x + SMALL_PADDING;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, SMALL_HEIGHT), Sense::CLICK);
    response.widget_info(|| match selected {
        Some(on) => WidgetInfo::selected(Role::Button, ui.is_enabled(), on, label),
        None => WidgetInfo::labeled(Role::Button, ui.is_enabled(), label),
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let radius = CornerRadius::same((SMALL_HEIGHT / 2.0) as u8);
    let painter = ui.painter();
    if on {
        let fill = if ui.is_enabled() {
            rgb(m3.secondary_container)
        } else {
            rgb(m3.on_surface).gamma_multiply(DISABLED_CONTAINER_OPACITY)
        };
        painter.rect_filled(rect, radius, fill);
    } else {
        let outline = if ui.is_enabled() {
            rgb(m3.outline)
        } else {
            rgb(m3.on_surface).gamma_multiply(DISABLED_CONTAINER_OPACITY)
        };
        painter.rect_stroke(rect, radius, Stroke::new(1.0, outline), StrokeKind::Inside);
    }
    paint_state_layer(ui, &response, radius, content);
    let y = rect.center().y - text.size().y / 2.0;
    ui.painter()
        .galley(egui::pos2(rect.left() + SMALL_PADDING, y), text, content);
    response
}
