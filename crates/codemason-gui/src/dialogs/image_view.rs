//! The full-size view of a transcript image.
//!
//! A click on an image in the transcript (an attachment's thumbnail, or an
//! image in a reply) opens it here ([`open`]); [`show`] draws it in a
//! Material dialog, fitted to the window, or at its own size (scrolling)
//! after Actual size, until the Close button or a click outside closes it.
//! Esc is left to app-core, which owns the keyboard, so it cannot both close
//! the view and act on the session.
//!
//! The view is GUI state (egui memory), not App state: no other frontend
//! shows images.
//!
//! Dependencies: `egui`, and this crate's Material components and palette.

use egui::Id;

use crate::material::button::{ButtonKind, button};
use crate::material::dialog::{dialog_actions, dialog_frame, dialog_headline};
use crate::metrics::space;
use crate::palette::Palette;

/// Title of the view.
pub const IMAGE_VIEW_TITLE: &str = "Image";

/// The button that closes the view.
pub const CLOSE_IMAGE: &str = "Close";

/// The button that shows the image at its own size (scrolling).
pub const ACTUAL_SIZE: &str = "Actual size";

/// The button that fits the image to the window again.
pub const FIT_TO_WINDOW: &str = "Fit to window";

/// The share of the window the image may take, each way.
const BODY_SHARE: f32 = 0.75;

/// The smallest the image area gets, each way, in points.
const MIN_BODY: f32 = 160.0;

/// The narrowest the dialog gets, in points: room for its headline and
/// buttons beside a small image.
const MIN_DIALOG_WIDTH: f32 = 280.0;

/// `size` scaled, keeping its shape, to fit inside `room`. A raster image
/// that already fits keeps its own size (enlarging it would blur it); a
/// vector image (`vector`) is scaled up to fill the room too.
fn fitted(size: egui::Vec2, room: egui::Vec2, vector: bool) -> egui::Vec2 {
    let scale = (room.x / size.x).min(room.y / size.y);
    size * if vector { scale } else { scale.min(1.0) }
}

/// An image the view can show: where egui loads it from, and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// The image's egui URI: `file://...`, or `bytes://...` for bytes the
    /// window already holds (a rendered diagram).
    pub uri: String,
    /// The name shown as the view's headline.
    pub name: String,
}

impl ImageRef {
    /// The image file at `path`, named by its file name.
    #[must_use]
    pub fn file(path: &std::path::Path) -> Self {
        Self {
            uri: crate::image_load::file_uri(path),
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image")
                .to_string(),
        }
    }
}

/// The egui memory slot of the image shown.
fn shown_id() -> Id {
    Id::unique("codemason_image_view")
}

/// The egui memory slot of whether the image is at its own size.
fn actual_id() -> Id {
    Id::unique("codemason_image_view_actual")
}

/// Show `image`, fitted to the window.
pub fn open(ctx: &egui::Context, image: ImageRef) {
    ctx.data_mut(|d| {
        d.insert_temp(shown_id(), image);
        d.remove::<bool>(actual_id());
    });
}

/// The image the view shows, if it is open.
#[must_use]
pub fn shown(ctx: &egui::Context) -> Option<ImageRef> {
    ctx.data(|d| d.get_temp::<ImageRef>(shown_id()))
}

/// Draw the view while an image is open.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `palette` - the colours.
///
/// # Returns
///
/// `true` while the view is open.
pub fn show(ctx: &egui::Context, palette: &Palette) -> bool {
    let Some(ImageRef { uri, name }) = shown(ctx) else {
        return false;
    };
    let window = ctx.content_rect().size();
    // The image's own size, once loaded, for Actual size.
    crate::image_load::refresh_if_changed(ctx, &uri);
    let natural = crate::image_load::decode_hint(ctx, &uri).and_then(|hint| {
        ctx.try_load_texture(&uri, egui::TextureOptions::default(), hint)
            .ok()
            .and_then(|poll| poll.size())
    });
    let mut actual = ctx
        .data(|d| d.get_temp::<bool>(actual_id()))
        .unwrap_or(false);
    let mut close = false;
    let modal = egui::Modal::new(Id::unique("codemason_image_view_modal"))
        .frame(dialog_frame(palette))
        .show(ctx, |ui| {
            let inner = ui.vertical(|ui| {
                let body = (window * BODY_SHARE).max(egui::Vec2::splat(MIN_BODY));
                // Exact sizes, so the dialog does not grow into the room
                // it is given frame after frame.
                let shown = natural.map_or(egui::Vec2::splat(MIN_BODY), |size| {
                    if actual {
                        size.min(body)
                    } else {
                        fitted(size, body, crate::image_load::is_svg(&uri))
                    }
                });
                ui.set_width(shown.x.max(MIN_DIALOG_WIDTH));
                dialog_headline(ui, palette, &name);
                let image = egui::Image::new(uri.as_str()).alt_text(format!("Image: {name}"));
                match natural {
                    Some(size) if actual => {
                        egui::ScrollArea::both()
                            .max_width(body.x)
                            .max_height(body.y)
                            .show(ui, |ui| ui.add(image.fit_to_exact_size(size)));
                    }
                    _ => {
                        ui.add(image.fit_to_exact_size(shown));
                    }
                }
                ui.add_space(space::LG);
                dialog_actions(ui, |ui| {
                    close = button(ui, palette, ButtonKind::Text, None, CLOSE_IMAGE).clicked();
                    let toggle = if actual { FIT_TO_WINDOW } else { ACTUAL_SIZE };
                    let toggled = ui
                        .add_enabled_ui(natural.is_some(), |ui| {
                            button(ui, palette, ButtonKind::Text, None, toggle)
                        })
                        .inner
                        .clicked();
                    if toggled {
                        actual = !actual;
                    }
                });
            });
            inner.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Dialog, true, IMAGE_VIEW_TITLE)
            });
        });
    if close || modal.backdrop_response.clicked() {
        ctx.data_mut(|d| {
            d.remove::<ImageRef>(shown_id());
            d.remove::<bool>(actual_id());
        });
    } else {
        ctx.data_mut(|d| d.insert_temp(actual_id(), actual));
    }
    true
}
