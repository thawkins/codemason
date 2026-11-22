//! Material Design 3 feedback components: slider, linear progress
//! indicator, plain tooltip and snackbar.
//!
//! - [`slider`] edits a value in a range by dragging, clicking the track, or
//!   the arrow keys while focused.
//! - [`linear_progress`] shows how far along a task is, or that it is
//!   running when the progress is unknown.
//! - [`stacked_progress`] shows how a whole is shared out, one coloured
//!   segment per part, on the progress indicator's track; [`swatch`] is the
//!   matching legend square.
//! - [`plain_tooltip`] shows a short `inverse_surface` label under a
//!   hovered widget.
//! - [`snackbar`] shows a message with an optional action at the bottom
//!   centre of the window.
//!
//! Dependencies: `egui`, and this crate's palette and metrics.

use std::ops::RangeInclusive;

use egui::{
    Align2, Area, CornerRadius, Frame, Id, Key, Margin, Order, Rect, Response, Role, Sense, Vec2,
    WidgetInfo,
};

use super::{HOVER_OPACITY, content_colour, label_large, paint_focus_ring};
use crate::metrics::{radius, text_size};
use crate::palette::{Palette, rgb};

/// Height of the slider's touch area, in points.
pub const SLIDER_HEIGHT: f32 = 44.0;

/// Height of a slider track and of a progress indicator, in points.
pub const TRACK_HEIGHT: f32 = 4.0;

/// Diameter of the slider's handle, in points.
const HANDLE_SIZE: f32 = 20.0;

/// Diameter of the state layer around a hovered or dragged handle, in
/// points.
const HANDLE_HALO: f32 = 40.0;

/// Number of arrow-key steps across the whole range.
const KEY_STEPS: f32 = 100.0;

/// Seconds an indeterminate progress indicator takes to cross its track.
const INDETERMINATE_PERIOD: f64 = 1.5;

/// Fraction of the track the indeterminate segment covers.
const INDETERMINATE_WIDTH: f32 = 0.3;

/// Height of a stacked progress indicator, in points: taller than
/// [`TRACK_HEIGHT`] so narrow segments stay visible.
pub const STACKED_TRACK_HEIGHT: f32 = 6.0;

/// Gap between two segments of a stacked progress indicator, in points.
const STACKED_GAP: f32 = 1.0;

/// Side of a legend swatch, in points.
pub const SWATCH_SIZE: f32 = 10.0;

/// Corner radius of a legend swatch, in points.
pub(crate) const SWATCH_RADIUS: u8 = 2;

/// Distance of the snackbar from the bottom of the window, in points.
const SNACKBAR_MARGIN: f32 = 16.0;

/// Draw a slider.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the slider fills its available width.
/// * `palette` - the colours.
/// * `value` - the value; clamped into `range`.
/// * `range` - the values the slider covers, start below end.
/// * `label` - the accessible name.
///
/// # Returns
///
/// The slider's response, marked changed when `value` changed.
pub fn slider(
    ui: &mut egui::Ui,
    palette: &Palette,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
) -> Response {
    let size = Vec2::new(ui.available_width(), SLIDER_HEIGHT);
    slider_sized(ui, palette, value, range, label, size)
}

/// Draw a slider of a given size, e.g. a compact one in a status bar.
///
/// # Arguments
///
/// * `size` - the slider's touch area; the track runs across its middle.
/// * the others as for [`slider`].
///
/// # Returns
///
/// The slider's response, marked changed when `value` changed.
pub fn slider_sized(
    ui: &mut egui::Ui,
    palette: &Palette,
    value: &mut f32,
    range: RangeInclusive<f32>,
    label: &str,
    size: Vec2,
) -> Response {
    let m3 = &palette.m3;
    let (start, end) = (*range.start(), *range.end());
    let span = (end - start).max(f32::EPSILON);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let track = rect.shrink2(Vec2::new(HANDLE_SIZE / 2.0, 0.0));
    let before = *value;
    if let Some(pointer) = response.interact_pointer_pos() {
        let fraction = ((pointer.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
        *value = fraction.mul_add(span, start);
    }
    if response.has_focus() {
        let step = span / KEY_STEPS;
        ui.input(|input| {
            if input.key_pressed(Key::ArrowRight) || input.key_pressed(Key::ArrowUp) {
                *value += step;
            }
            if input.key_pressed(Key::ArrowLeft) || input.key_pressed(Key::ArrowDown) {
                *value -= step;
            }
        });
    }
    *value = value.clamp(start, end);
    if (*value - before).abs() > f32::EPSILON {
        response.mark_changed();
    }
    let shown = f64::from(*value);
    response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), shown, label));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let active = content_colour(ui, palette, rgb(m3.primary));
    let inactive = content_colour(ui, palette, rgb(m3.secondary_container));
    let fraction = (*value - start) / span;
    let handle_x = fraction.mul_add(track.width(), track.left());
    let y = rect.center().y;
    let half = TRACK_HEIGHT / 2.0;
    let rounded = CornerRadius::same(half as u8);
    let painter = ui.painter();
    painter.rect_filled(
        Rect::from_min_max(
            egui::pos2(track.left(), y - half),
            egui::pos2(track.right(), y + half),
        ),
        rounded,
        inactive,
    );
    painter.rect_filled(
        Rect::from_min_max(
            egui::pos2(track.left(), y - half),
            egui::pos2(handle_x, y + half),
        ),
        rounded,
        active,
    );
    let centre = egui::pos2(handle_x, y);
    if ui.is_enabled() && (response.hovered() || response.dragged()) {
        painter.circle_filled(
            centre,
            HANDLE_HALO / 2.0,
            active.gamma_multiply(HOVER_OPACITY),
        );
    }
    painter.circle_filled(centre, HANDLE_SIZE / 2.0, active);
    paint_focus_ring(ui, &response, CornerRadius::same(radius::SM), palette);
    response
}

/// Draw a linear progress indicator.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the indicator fills its available width.
/// * `palette` - the colours.
/// * `progress` - how far along the task is, 0.0 to 1.0, or `None` while it
///   is unknown, which animates a moving segment.
///
/// # Returns
///
/// The indicator's response.
pub fn linear_progress(ui: &mut egui::Ui, palette: &Palette, progress: Option<f32>) -> Response {
    let m3 = &palette.m3;
    let size = Vec2::new(ui.available_width(), TRACK_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, "Progress"));
    let rounded = CornerRadius::same((TRACK_HEIGHT / 2.0) as u8);
    let track = rgb(m3.secondary_container);
    let active = rgb(m3.primary);
    ui.painter().rect_filled(rect, rounded, track);
    let (from, to) = match progress {
        Some(done) => (0.0, done.clamp(0.0, 1.0)),
        None => {
            let time = ui.input(|input| input.time);
            let phase = (time / INDETERMINATE_PERIOD).fract() as f32;
            let lead = phase * (1.0 + INDETERMINATE_WIDTH);
            ui.ctx().request_repaint();
            ((lead - INDETERMINATE_WIDTH).max(0.0), lead.min(1.0))
        }
    };
    if to > from {
        let segment = Rect::from_min_max(
            egui::pos2(from.mul_add(rect.width(), rect.left()), rect.top()),
            egui::pos2(to.mul_add(rect.width(), rect.left()), rect.bottom()),
        );
        ui.painter().rect_filled(segment, rounded, active);
    }
    response
}

/// Draw a stacked progress indicator: the track with one segment per part,
/// left to right, each as wide as its fraction of the track.
///
/// Fractions are clamped to `0.0..=1.0` and the segments stop at the end of
/// the track, so parts that add up to more than the whole are cut. Segments
/// narrower than a point are skipped; the others are parted by a hairline.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; the indicator fills its available width.
/// * `palette` - the colours (the track is `secondary_container`).
/// * `segments` - `(fraction, colour)` of each part, in drawing order.
/// * `label` - the accessible name, e.g. `31% used`.
///
/// # Returns
///
/// The indicator's response.
pub fn stacked_progress(
    ui: &mut egui::Ui,
    palette: &Palette,
    segments: &[(f32, egui::Color32)],
    label: &str,
) -> Response {
    let size = Vec2::new(ui.available_width(), STACKED_TRACK_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, label));
    let rounded = CornerRadius::same((STACKED_TRACK_HEIGHT / 2.0) as u8);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, rounded, rgb(palette.m3.secondary_container));
    let mut left = rect.left();
    for &(fraction, colour) in segments {
        let width = (fraction.clamp(0.0, 1.0) * rect.width()).min(rect.right() - left);
        if width < 1.0 {
            continue;
        }
        let segment = Rect::from_min_max(
            egui::pos2(left, rect.top()),
            egui::pos2((left + width - STACKED_GAP).max(left + 1.0), rect.bottom()),
        );
        painter.rect_filled(segment, CornerRadius::ZERO, colour);
        left += width;
    }
    response
}

/// Draw a legend swatch: a small rounded square of `colour`, centred on
/// the line.
///
/// # Arguments
///
/// * `ui` - the ui to draw in.
/// * `colour` - the swatch's colour.
///
/// # Returns
///
/// The swatch's response.
pub fn swatch(ui: &mut egui::Ui, colour: egui::Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(SWATCH_SIZE), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(SWATCH_RADIUS), colour);
    response
}

/// The rectangle of a legend swatch whose top-left corner is `min`.
pub(crate) fn swatch_rect(min: egui::Pos2) -> Rect {
    Rect::from_min_size(min, Vec2::splat(SWATCH_SIZE))
}

/// Show a plain tooltip under `response` while it is hovered.
///
/// # Arguments
///
/// * `response` - the widget the tooltip describes.
/// * `palette` - the colours.
/// * `text` - the tooltip's text.
///
/// # Returns
///
/// `response`, for chaining.
pub fn plain_tooltip(response: Response, palette: &Palette, text: &str) -> Response {
    if response.hovered() {
        let m3 = &palette.m3;
        Area::new(response.id.with("material_tooltip"))
            .order(Order::Tooltip)
            .fixed_pos(response.rect.center_bottom() + Vec2::new(0.0, 4.0))
            .pivot(Align2::CENTER_TOP)
            .interactable(false)
            .show(&response.ctx, |ui| {
                Frame::new()
                    .fill(rgb(m3.inverse_surface))
                    .corner_radius(CornerRadius::same(radius::XS))
                    .inner_margin(Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(text_size::SMALL)
                                .color(rgb(m3.inverse_on_surface)),
                        );
                    });
            });
    }
    response
}

/// Show a snackbar at the bottom centre of the window.
///
/// # Arguments
///
/// * `ctx` - the window's context.
/// * `palette` - the colours.
/// * `id` - a stable id for this snackbar.
/// * `text` - the message.
/// * `action` - an optional action label, drawn as a text button.
///
/// # Returns
///
/// `true` when the action was clicked this frame.
pub fn snackbar(
    ctx: &egui::Context,
    palette: &Palette,
    id: Id,
    text: &str,
    action: Option<&str>,
) -> bool {
    let m3 = &palette.m3;
    let anchor = ctx.content_rect().center_bottom() - Vec2::new(0.0, SNACKBAR_MARGIN);
    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(anchor)
        .pivot(Align2::CENTER_BOTTOM)
        .show(ctx, |ui| {
            Frame::new()
                .fill(rgb(m3.inverse_surface))
                .corner_radius(CornerRadius::same(radius::XS))
                .inner_margin(Margin::symmetric(16, 14))
                .shadow(super::elevation_shadow(3, palette))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(text)
                                .size(text_size::BODY)
                                .color(rgb(m3.inverse_on_surface)),
                        );
                        action.is_some_and(|action| {
                            ui.add_space(24.0);
                            let label = egui::RichText::new(action)
                                .font(label_large(ui.ctx()))
                                .color(rgb(m3.inverse_primary));
                            ui.add(egui::Button::new(label).frame(false)).clicked()
                        })
                    })
                    .inner
                })
                .inner
        })
        .inner
}
