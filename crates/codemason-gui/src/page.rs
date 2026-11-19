//! The page: how the transcript and the prompt are framed in the
//! word-processor layout.
//!
//! [`page_ui`] fills its whole pane: the transcript on a `surface` page
//! ([`PageStyle::Page`]) edge to edge, so its content starts at the pane's
//! top edge, level with the panes beside it, and the prompt in an outlined
//! field ([`PageStyle::Field`]) inset by [`PANE_MARGIN`], so the content
//! uses all the room the dock gives it.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Frame, Margin, Stroke, UiBuilder, Vec2};

use crate::metrics::{radius, space};
use crate::palette::{Palette, rgb};

/// Gap between the pane's edge and the prompt's field, in points.
pub const PANE_MARGIN: f32 = space::XS;

/// Space between the transcript page's sides and its content, in points;
/// the page has none above or below, so its content is level with the top
/// of the panes beside it.
pub const PAGE_SIDE_PADDING: f32 = space::SM;

/// How the content of a pane is framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStyle {
    /// A page filling the pane, for the transcript.
    Page,
    /// An outlined field filling the pane, for the prompt.
    Field,
}

/// Draw `add` framed as `style`, filling the pane.
///
/// # Arguments
///
/// * `ui` - the pane's ui.
/// * `palette` - the colours.
/// * `style` - a page or an outlined field.
/// * `add` - draws the content.
///
/// # Returns
///
/// What `add` returned.
pub fn page_ui<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    style: PageStyle,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m3 = &palette.m3;
    let pane = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(pane, 0.0, rgb(m3.surface_container_low));
    let column = match style {
        PageStyle::Page => pane,
        PageStyle::Field => pane.shrink(PANE_MARGIN),
    };
    let frame = match style {
        PageStyle::Page => Frame::new()
            .fill(rgb(m3.surface))
            .inner_margin(Margin::symmetric(PAGE_SIDE_PADDING as i8, 0)),
        PageStyle::Field => Frame::new()
            .fill(rgb(m3.surface_container_lowest))
            .stroke(Stroke::new(1.0, rgb(m3.outline)))
            .corner_radius(CornerRadius::same(radius::XS))
            .inner_margin(Margin::symmetric(space::LG as i8, space::SM as i8)),
    };
    let inner = ui
        .scope_builder(UiBuilder::new().max_rect(column), |ui| {
            frame
                .show(ui, |ui| {
                    ui.set_min_size(Vec2::new(ui.available_width(), ui.available_height()));
                    add(ui)
                })
                .inner
        })
        .inner;
    ui.allocate_rect(pane, egui::Sense::hover());
    inner
}
