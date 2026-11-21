//! The page: how the transcript and the prompt are framed in the
//! word-processor layout.
//!
//! [`page_ui`] fills its whole pane: the transcript on a `surface` page
//! ([`PageStyle::Page`]) edge to edge, so its content starts at the pane's
//! top edge, level with the panes beside it, and the prompt in an outlined
//! field ([`PageStyle::Field`]) sat on the pane's bottom edge, as tall as its
//! content plus [`FIELD_PADDING`]: it grows upwards only, and the shell fits
//! the pane to [`field_height`].
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use egui::{CornerRadius, Frame, Id, Margin, Rect, Stroke, UiBuilder, Vec2, pos2};

use crate::metrics::{radius, space};
use crate::palette::{Palette, rgb};

/// Space between the prompt field's outline and its controls, in points.
pub const FIELD_PADDING: f32 = space::SM;

/// The most of the window's height the prompt field may take, as a share.
const FIELD_MAX_SHARE: f32 = 0.5;

/// Space between the transcript page's sides and its content, in points;
/// the page has none above or below, so its content is level with the top
/// of the panes beside it.
pub const PAGE_SIDE_PADDING: f32 = space::SM;

/// How the content of a pane is framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStyle {
    /// A page filling the pane, for the transcript.
    Page,
    /// An outlined field on the pane's bottom edge, for the prompt.
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
    let inner = match style {
        PageStyle::Page => {
            let frame = Frame::new()
                .fill(rgb(m3.surface))
                .inner_margin(Margin::symmetric(PAGE_SIDE_PADDING as i8, 0));
            ui.scope_builder(UiBuilder::new().max_rect(pane), |ui| {
                frame
                    .show(ui, |ui| {
                        ui.set_min_size(Vec2::new(ui.available_width(), ui.available_height()));
                        add(ui)
                    })
                    .inner
            })
            .inner
        }
        PageStyle::Field => field_ui(ui, palette, pane, add),
    };
    ui.allocate_rect(pane, egui::Sense::hover());
    inner
}

/// The prompt field's height last frame, in points: what the shell sizes the
/// prompt pane to, so the field's bottom stays on the pane's bottom edge.
///
/// # Arguments
///
/// * `ctx` - the window's context.
///
/// # Returns
///
/// The height, or `None` before the field is first drawn.
#[must_use]
pub fn field_height(ctx: &egui::Context) -> Option<f32> {
    ctx.data(|data| data.get_temp(field_height_id()))
}

fn field_height_id() -> Id {
    Id::unique("codemason_prompt_field_height")
}

/// Draw `add` in an outlined field whose bottom is `pane`'s bottom edge.
///
/// The field is laid out from last frame's height upwards, so a taller
/// content moves only its top. Its content may use up to half the window's
/// height; a change of height redoes the pass before it is painted.
fn field_ui<R>(
    ui: &mut egui::Ui,
    palette: &Palette,
    pane: Rect,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let m3 = &palette.m3;
    let ctx = ui.ctx().clone();
    let last = field_height(&ctx).unwrap_or(pane.height());
    let room = (ctx.content_rect().height() * FIELD_MAX_SHARE).max(last);
    let top = pane.bottom() - last;
    let column = Rect::from_min_max(pos2(pane.left(), top), pos2(pane.right(), top + room));
    let frame = Frame::new()
        .fill(rgb(m3.surface_container_lowest))
        .stroke(Stroke::new(1.0, rgb(m3.outline)))
        .corner_radius(CornerRadius::same(radius::XS))
        .inner_margin(Margin::same(FIELD_PADDING as i8));
    let shown = ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
        ui.set_clip_rect(pane.intersect(ui.clip_rect()));
        frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            add(ui)
        })
    });
    let height = shown.inner.response.rect.height();
    if (height - last).abs() > 0.5 {
        ctx.data_mut(|data| data.insert_temp(field_height_id(), height));
        ctx.request_discard("prompt field height changed");
    }
    shown.inner.inner
}
