//! The page on a desk: how the transcript and the prompt are framed in the
//! word-processor layout.
//!
//! [`page_ui`] fills its pane with the desk colour (`surface_container_low`)
//! and draws the content in a centred column at most [`PAGE_WIDTH`] wide: a
//! raised `surface` page ([`PageStyle::Page`], the transcript) or an outlined
//! field ([`PageStyle::Field`], the prompt), so the two line up one above the
//! other.
//!
//! Dependencies: `egui`, and this crate's palette, material shadows and
//! metrics.

use egui::{CornerRadius, Frame, Margin, Rect, Stroke, UiBuilder, Vec2};

use crate::material::elevation_shadow;
use crate::metrics::{radius, space};
use crate::palette::{Palette, rgb};

/// The widest a page is, in points: a Letter page at 96 dots per inch.
pub const PAGE_WIDTH: f32 = 816.0;

/// Desk visible on each side of the page, at least, in points.
pub const DESK_MARGIN: f32 = 24.0;

/// Horizontal padding inside the page, in points.
const PAGE_PADDING: i8 = 48;

/// How the content of a column is framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStyle {
    /// A raised page that runs to the bottom of the pane.
    Page,
    /// An outlined field, for the prompt.
    Field,
}

/// The width of the column in a pane `available` points wide.
///
/// # Returns
///
/// [`PAGE_WIDTH`], or less when the pane cannot fit it with
/// [`DESK_MARGIN`] on each side; never negative.
#[must_use]
pub fn page_width(available: f32) -> f32 {
    DESK_MARGIN.mul_add(-2.0, available).clamp(0.0, PAGE_WIDTH)
}

/// Draw `add` in a centred column on the desk.
///
/// # Arguments
///
/// * `ui` - the pane's ui; the whole pane becomes desk.
/// * `palette` - the colours.
/// * `style` - a raised page or an outlined field.
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
    let width = page_width(pane.width());
    let top = match style {
        PageStyle::Page => pane.top() + DESK_MARGIN / 2.0,
        PageStyle::Field => pane.top() + space::XS,
    };
    let bottom = pane.bottom() - space::XS;
    let column = Rect::from_min_max(
        egui::pos2(pane.center().x - width / 2.0, top),
        egui::pos2(pane.center().x + width / 2.0, bottom.max(top)),
    );
    let frame = match style {
        // `surface`, the transcript's own background, so the two merge.
        PageStyle::Page => Frame::new()
            .fill(rgb(m3.surface))
            .shadow(elevation_shadow(1, palette))
            .inner_margin(Margin::symmetric(PAGE_PADDING, space::LG as i8)),
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
