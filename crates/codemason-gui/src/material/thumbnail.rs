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

/// What there is to draw for a thumbnail's image.
///
/// The three decode states come from [`crate::image_load::picture_state`];
/// [`Picture::OffScreen`] is this widget's own, for a thumbnail that is
/// scrolled out of sight and so never asks for a decode at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picture {
    /// The image is decoded and can be drawn.
    Ready,
    /// Its bytes are still on their way; it may be drawn next frame.
    Pending,
    /// No loader could read it, so nothing will ever be drawn.
    Unavailable,
    /// The thumbnail is out of sight: nothing is loaded or drawn.
    OffScreen,
}

impl From<crate::image_load::PictureState> for Picture {
    fn from(state: crate::image_load::PictureState) -> Self {
        match state {
            crate::image_load::PictureState::Ready => Self::Ready,
            crate::image_load::PictureState::Pending => Self::Pending,
            crate::image_load::PictureState::Unavailable => Self::Unavailable,
        }
    }
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
///   [`crate::image_load::file_uri`]).
/// * `label` - the image's accessible description (its file name), used as
///   the picture's alt text and hover text.
/// * `size` - the thumbnail's side, in points; widened to at least
///   [`CLOSE_SIZE`] so the close mark always fits.
///
/// # Returns
///
/// [`ThumbnailResponse`]: the caller drops its entry when
/// [`ThumbnailResponse::close`] was clicked, and shows the image at full size
/// when [`ThumbnailResponse::picture`] was double-clicked. Only a decoded
/// picture senses a click, so a thumbnail whose image is still loading, or
/// that no loader could read, reports no double click.
pub fn thumbnail(
    ui: &mut egui::Ui,
    palette: &Palette,
    uri: &str,
    label: &str,
    size: f32,
) -> ThumbnailResponse {
    let side = size.max(CLOSE_SIZE);
    // Cull before loading: the strip can be scrolled past, and a thumbnail
    // that is not on screen has no reason to stat its file or start a decode.
    // The next widget's place is where this one would go, so the test needs no
    // allocation.
    let place = ui.next_widget_position();
    let picture = if ui.is_rect_visible(egui::Rect::from_min_size(place, Vec2::splat(side))) {
        crate::image_load::picture_state(ui.ctx(), uri).into()
    } else {
        Picture::OffScreen
    };
    // `Sense::CLICK` rather than `Sense::click()`: it reports a double click
    // without becoming focusable, so Tab never lands on a thumbnail and a
    // focused mark cannot keep the prompt from taking typed text. Hover is
    // sensed for free (its mask is empty), so the tooltip needs no sense of its
    // own.
    let sense = if picture == Picture::Ready {
        Sense::CLICK
    } else {
        Sense::hover()
    };
    let (rect, picture_response) = ui.allocate_exact_size(Vec2::splat(side), sense);
    picture_response.widget_info(|| WidgetInfo::labeled(Role::Image, ui.is_enabled(), label));
    let picture_response = picture_response.on_hover_text(label);
    if picture != Picture::OffScreen {
        let corner = CornerRadius::same(radius::SM);
        paint_picture(ui, palette, uri, label, rect, corner, picture);
        ui.painter().rect_stroke(
            rect,
            corner,
            Stroke::new(FRAME_WIDTH, rgb(palette.m3.outline_variant)),
            StrokeKind::Inside,
        );
        paint_state_layer(ui, &picture_response, corner, rgb(palette.m3.on_surface));
    }
    // The close mark gets its own ui over the picture's top-right corner, so
    // it keeps a full Material hit target inside the thumbnail's box. It stays
    // live off screen: it is how a mis-attached image is dropped.
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
    ThumbnailResponse {
        picture: picture_response,
        close,
    }
}

/// Draw `uri` to fill `rect`, a placeholder while it loads, or `label` in its
/// place when it cannot be drawn.
fn paint_picture(
    ui: &egui::Ui,
    palette: &Palette,
    uri: &str,
    label: &str,
    rect: egui::Rect,
    corner: CornerRadius,
    picture: Picture,
) {
    let content = rgb(palette.m3.on_surface_variant);
    if picture == Picture::Unavailable {
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
    if picture == Picture::Pending {
        // Nothing to show yet, but the frame and the close mark are already in
        // place, so the strip does not jump when the picture arrives.
        return;
    }
    egui::Image::new(uri)
        .corner_radius(corner)
        .alt_text(label)
        .paint_at(ui, rect);
}
