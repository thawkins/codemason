//! The full-size view of a transcript image.
//!
//! A click on an image thumbnail in the transcript opens it here
//! ([`open`]); [`show`] draws it, scaled to fit 80% of the window, until
//! the Close button or a click outside closes it. Esc is left to app-core,
//! which owns the keyboard, so it cannot both close the view and act on the
//! session.
//!
//! The view is GUI state (egui memory), not App state: no other frontend
//! shows images.
//!
//! Dependencies: `egui`.

use std::path::PathBuf;

use egui::Id;

/// Title of the view.
pub const IMAGE_VIEW_TITLE: &str = "Image";

/// The button that closes the view.
pub const CLOSE_IMAGE: &str = "Close";

/// The egui memory slot of the image shown.
fn shown_id() -> Id {
    Id::unique("codemason_image_view")
}

/// Show the image at `path`.
pub fn open(ctx: &egui::Context, path: PathBuf) {
    ctx.data_mut(|d| d.insert_temp(shown_id(), path));
}

/// The image the view shows, if it is open.
#[must_use]
pub fn shown(ctx: &egui::Context) -> Option<PathBuf> {
    ctx.data(|d| d.get_temp::<PathBuf>(shown_id()))
}

/// Draw the view while an image is open.
///
/// # Returns
///
/// `true` while the view is open.
pub fn show(ctx: &egui::Context) -> bool {
    let Some(path) = shown(ctx) else {
        return false;
    };
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();
    let fit = ctx.content_rect().size() * 0.8;
    let mut close = false;
    let modal = egui::Modal::new(Id::unique("codemason_image_view_modal")).show(ctx, |ui| {
        let inner = ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading(&name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = egui::Button::new(CLOSE_IMAGE).sense(egui::Sense::CLICK);
                    close = ui.add(button).clicked();
                });
            });
            ui.add(
                egui::Image::new(format!("file://{}", path.display()))
                    .max_size(fit)
                    .alt_text(format!("Image: {name}")),
            );
        });
        inner
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Dialog, true, IMAGE_VIEW_TITLE));
    });
    if close || modal.backdrop_response.clicked() {
        ctx.data_mut(|d| d.remove::<PathBuf>(shown_id()));
    }
    true
}
