//! A removable image thumbnail: a picture in a framed box with a close mark
//! in its top-right corner.
//!
//! [`thumbnail`] draws one attachment: the image at `uri` sized to fill a
//! square box, framed in `outline_variant`, with a Material icon button
//! ([`Icon::Cross`], labelled [`REMOVE_LABEL`]) over it. The caller owns both
//! the list and the removal: the widget reports the click and draws nothing
//! else, so a strip of thumbnails is laid out with [`egui::Ui::horizontal`]
//! and each one is dropped from the caller's `Vec` on its click.
//!
//! The picture also reports a double click ([`ThumbnailResponse`]'s `picture`
//! response), which the caller answers by opening the image at full size in
//! [`crate::dialogs::image_view`], the same view a transcript image opens.
//!
//! An image the window cannot decode (a file that went away, or a format no
//! loader reads) is drawn as its `label` inside the same frame, so a broken
//! attachment can still be seen and removed; there is no picture to enlarge,
//! so that one reports no double click.
//!
//! Dependencies: `egui`, and this crate's image loading, icons, palette and
//! metrics.

use std::path::Path;

use egui::{CornerRadius, Response, Role, Sense, Stroke, StrokeKind, Vec2, WidgetInfo};

use super::icon_button::{DENSE_SIZE, IconButtonKind, icon_button_dense};
use super::{label_large, paint_state_layer, text_galley};
use crate::icons::Icon;
use crate::metrics::{radius, space};
use crate::palette::{Palette, rgb};

/// Accessible name and hover text of a thumbnail's close mark.
pub const REMOVE_LABEL: &str = "Remove image";

/// The default side of a thumbnail, in points.
pub const SIZE: f32 = 64.0;

/// Side of the close mark, in points: Material's dense icon button, so the
/// mark is a full hit target over the picture's corner.
pub const CLOSE_SIZE: f32 = DENSE_SIZE;

/// How far the close mark is inset from the thumbnail's corner, in points.
const CLOSE_INSET: f32 = 0.0;

/// Width of the thumbnail's frame, in points.
const FRAME_WIDTH: f32 = 1.0;

/// The `file://` URI for an image file.
#[must_use]
pub fn file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Whether `uri` can be loaded as an image this frame.
///
/// A changed file is read again and an SVG is rasterised at a capped size
/// ([`crate::image_load`]), as the transcript's images are.
fn loadable(ui: &egui::Ui, uri: &str) -> bool {
    let ctx = ui.ctx();
    crate::image_load::refresh_if_changed(ctx, uri);
    let Some(hint) = crate::image_load::decode_hint(ctx, uri) else {
        ctx.request_repaint();
        return false;
    };
    ctx.try_load_image(uri, hint).is_ok()
}

/// What a thumbnail reported this frame.
#[derive(Debug)]
pub struct ThumbnailResponse {
    /// The picture's response. Its `double_clicked()` asks to show the image
    /// at full size; it is never focusable, so a thumbnail cannot take the
    /// keyboard from the prompt.
    pub picture: Response,
    /// The close mark's response; `clicked()` removes the attachment.
    pub close: Response,
}

/// Draw a removable image thumbnail.
///
/// # Arguments
///
/// * `ui` - the ui to draw in; a disabled ui reports a disabled thumbnail
///   and its close mark takes no click.
/// * `palette` - the colours.
/// * `uri` - the image's URI, as [`egui::Image::new`] takes it (see
///   [`file_uri`]).
/// * `label` - the image's accessible description (its file name), used as
///   the picture's alt text and hover text.
/// * `size` - the thumbnail's side, in points; widened to at least
///   [`CLOSE_SIZE`] so the close mark always fits.
///
/// # Returns
///
/// [`ThumbnailResponse`]: the caller drops its entry when
/// [`ThumbnailResponse::close`] was clicked, and shows the image at full size
/// when [`ThumbnailResponse::picture`] was double-clicked. A picture that
/// cannot be loaded senses nothing, so it reports no double click.
pub fn thumbnail(
    ui: &mut egui::Ui,
    palette: &Palette,
    uri: &str,
    label: &str,
    size: f32,
) -> ThumbnailResponse {
    let loadable = loadable(ui, uri);
    // `Sense::CLICK` rather than `Sense::click()`: it reports a double click
    // without becoming focusable, so Tab never lands on a thumbnail and a
    // focused mark cannot keep the prompt from taking typed text.
    let sense = if loadable {
        Sense::CLICK
    } else {
        Sense::hover()
    };
    let (rect, picture) = ui.allocate_exact_size(Vec2::splat(size.max(CLOSE_SIZE)), sense);
    picture.widget_info(|| WidgetInfo::labeled(Role::Image, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let corner = CornerRadius::same(radius::SM);
        paint_picture(ui, palette, uri, label, rect, corner, loadable);
        ui.painter().rect_stroke(
            rect,
            corner,
            Stroke::new(FRAME_WIDTH, rgb(palette.m3.outline_variant)),
            StrokeKind::Inside,
        );
        paint_state_layer(ui, &picture, corner, rgb(palette.m3.on_surface));
    }
    // The close mark gets its own ui over the picture's top-right corner, so
    // it keeps a full Material hit target inside the thumbnail's box.
    let close_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - CLOSE_SIZE - CLOSE_INSET,
            rect.top() + CLOSE_INSET,
        ),
        Vec2::splat(CLOSE_SIZE),
    );
    let close = ui
        .scope_builder(egui::UiBuilder::new().max_rect(close_rect), |ui| {
            icon_button_dense(
                ui,
                palette,
                IconButtonKind::Filled,
                Icon::Cross,
                REMOVE_LABEL,
                None,
            )
        })
        .inner;
    ThumbnailResponse { picture, close }
}

/// Draw `uri` to fill `rect`, or `label` in its place when `loadable` is
/// false.
fn paint_picture(
    ui: &egui::Ui,
    palette: &Palette,
    uri: &str,
    label: &str,
    rect: egui::Rect,
    corner: CornerRadius,
    loadable: bool,
) {
    let content = rgb(palette.m3.on_surface_variant);
    if !loadable {
        // The name, on one line, clipped to the frame: an attachment that
        // cannot be drawn still shows which image it is, so the close mark
        // removes the right one.
        let inner = rect.shrink(space::XS);
        let galley = text_galley(ui, label, label_large(ui.ctx()), content);
        let y = inner.center().y - galley.size().y / 2.0;
        ui.painter()
            .with_clip_rect(inner)
            .galley(egui::pos2(inner.left(), y), galley, content);
        return;
    }
    egui::Image::new(uri)
        .corner_radius(corner)
        .alt_text(label)
        .paint_at(ui, rect);
}
