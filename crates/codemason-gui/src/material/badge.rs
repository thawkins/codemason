//! Material Design 3 badge: a small pill that labels a state ("Installed",
//! "Signed in").
//!
//! [`badge`] draws one: [`HEIGHT`] points high with 8 point corners, the
//! label in the small label size, filled with the tone's `<tone>_container`
//! role and the text in `on_<tone>_container`. The palette's M3 roles have
//! no success container, so [`BadgeTone::Success`] uses the tertiary one.
//!
//! Dependencies: `egui`, and this crate's palette, fonts and metrics.

use egui::{Color32, CornerRadius, Response, Role, Sense, Vec2, WidgetInfo};

use super::text_galley;
use crate::fonts::bold_font;
use crate::metrics::text_size;
use crate::palette::{Palette, rgb};

/// Height of a badge, in points.
pub const HEIGHT: f32 = 18.0;

/// Corner radius of a badge, in points.
const RADIUS: u8 = 8;

/// Padding beside the label, in points.
const PADDING: f32 = 8.0;

/// The colour family of a badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeTone {
    /// A positive state such as "Installed".
    Success,
    /// The primary accent.
    Primary,
    /// The secondary accent.
    Secondary,
    /// The tertiary accent.
    Tertiary,
    /// A problem.
    Error,
    /// No emphasis.
    Neutral,
}

/// The fill and text colours of a badge.
///
/// # Arguments
///
/// * `palette` - the colours.
/// * `tone` - the badge's colour family.
///
/// # Returns
///
/// `(fill, text)`: the tone's container role and its `on_` role.
#[must_use]
pub fn badge_colours(palette: &Palette, tone: BadgeTone) -> (Color32, Color32) {
    let m3 = &palette.m3;
    let (fill, text) = match tone {
        BadgeTone::Success | BadgeTone::Tertiary => {
            (m3.tertiary_container, m3.on_tertiary_container)
        }
        BadgeTone::Primary => (m3.primary_container, m3.on_primary_container),
        BadgeTone::Secondary => (m3.secondary_container, m3.on_secondary_container),
        BadgeTone::Error => (m3.error_container, m3.on_error_container),
        BadgeTone::Neutral => (m3.surface_container_high, m3.on_surface),
    };
    (rgb(fill), rgb(text))
}

/// Draw a badge.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `palette` - the colours.
/// * `tone` - the badge's colour family.
/// * `label` - the badge's text, also its accessible name.
///
/// # Returns
///
/// The badge's response (it is not interactive).
pub fn badge(ui: &mut egui::Ui, palette: &Palette, tone: BadgeTone, label: &str) -> Response {
    let (fill, content) = badge_colours(palette, tone);
    let bold = egui::FontFamily::Name(crate::fonts::BOLD_FAMILY.into());
    let installed = ui
        .ctx()
        .fonts_mut(|f| f.definitions().families.contains_key(&bold));
    let font = if installed {
        bold_font(text_size::SMALL, false)
    } else {
        egui::FontId::proportional(text_size::SMALL)
    };
    let text = text_galley(ui, label, font, content);
    let size = Vec2::new(PADDING.mul_add(2.0, text.size().x), HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS), fill);
        let pos = egui::pos2(rect.left() + PADDING, rect.center().y - text.size().y / 2.0);
        ui.painter().galley(pos, text, content);
    }
    response
}
