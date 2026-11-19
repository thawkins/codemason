//! Material Design 3 primary tabs.
//!
//! [`primary_tabs`] draws a row of 48-point tabs sharing the available
//! width, a divider under them, and a 3-point `primary` indicator under the
//! active tab's label. Each tab is a craft-ui [`Tab`], which gives it the
//! accessible tab role and selected state and draws its keyboard focus.
//!
//! Dependencies: `egui`, `craft-ui` (tabs), and this crate's palette.

use craft_ui::tabs::Tab;
use egui::{CornerRadius, Rect, Response, Sense, Stroke, Vec2};

use super::{content_colour, label_large, paint_state_layer, text_galley};
use crate::palette::{Palette, rgb};

/// Height of the tab row, in points.
pub const HEIGHT: f32 = 48.0;

/// Height of the active indicator, in points.
pub const INDICATOR_HEIGHT: f32 = 3.0;

/// Narrowest active indicator, in points.
const MIN_INDICATOR_WIDTH: f32 = 24.0;

/// Draw a row of primary tabs.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the tabs share its available width.
/// * `palette` - the colours.
/// * `labels` - the tab labels, left to right.
/// * `selected` - the index of the active tab; a click on another tab sets
///   it.
///
/// # Returns
///
/// The union of the tabs' responses, marked changed when the active tab
/// changed.
pub fn primary_tabs(
    ui: &mut egui::Ui,
    palette: &Palette,
    labels: &[&str],
    selected: &mut usize,
) -> Response {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), HEIGHT);
    let (outer, mut union) = ui.allocate_exact_size(size, Sense::hover());
    let count = labels.len().max(1) as f32;
    let tab_width = outer.width() / count;
    for (index, label) in labels.iter().enumerate() {
        let left = outer.left() + tab_width * index as f32;
        let rect = Rect::from_min_size(egui::pos2(left, outer.top()), Vec2::new(tab_width, HEIGHT));
        let on = *selected == index;
        let id = ui.scope_id().with(("tab", index));
        let response = Tab::new(id, label, on)
            .focus_stroke(Stroke::new(2.0, rgb(m3.secondary)))
            .show_at(ui, rect, |ui, response| {
                paint_tab(ui, palette, response, label, on);
            });
        if response.clicked() && !on {
            *selected = index;
            union.mark_changed();
        }
        union = union.union(response);
    }
    ui.painter().hline(
        outer.x_range(),
        outer.bottom() - 0.5,
        Stroke::new(1.0, rgb(m3.outline_variant)),
    );
    union
}

/// Paint one tab: the state layer, its label, and the active indicator
/// under the label when it is the active tab.
fn paint_tab(ui: &egui::Ui, palette: &Palette, response: &Response, label: &str, on: bool) {
    let m3 = &palette.m3;
    let rect = response.rect;
    let content = content_colour(
        ui,
        palette,
        if on {
            rgb(m3.primary)
        } else {
            rgb(m3.on_surface_variant)
        },
    );
    paint_state_layer(ui, response, CornerRadius::ZERO, content);
    let text = text_galley(ui, label, label_large(), content);
    let text_width = text.size().x;
    ui.painter()
        .galley(rect.center() - text.size() / 2.0, text, content);
    if on {
        let width = text_width.max(MIN_INDICATOR_WIDTH);
        let indicator = Rect::from_min_size(
            egui::pos2(
                rect.center().x - width / 2.0,
                rect.bottom() - INDICATOR_HEIGHT,
            ),
            Vec2::new(width, INDICATOR_HEIGHT),
        );
        let top = INDICATOR_HEIGHT as u8;
        ui.painter().rect_filled(
            indicator,
            CornerRadius {
                nw: top,
                ne: top,
                sw: 0,
                se: 0,
            },
            rgb(m3.primary),
        );
    }
}
