//! The system clipboard for the window.
//!
//! - Text paste stays egui's `Event::Paste`, which the prompt and the
//!   dialogs' paste hooks already take.
//! - Image paste: when the clipboard holds an image and no text, egui-winit
//!   turns Ctrl+V / Cmd+V into `Event::PasteImage` (it reads the image with
//!   `arboard`, or over Wayland's non-blocking transport) and the key press
//!   itself never reaches the window. [`attach_pasted_images`] hands those
//!   pixels to app-core's `App::attach_clipboard_image`, the step `Alt+V`
//!   takes with raw clipboard pixels, so the image is saved and staged the
//!   same way. `Alt+V` itself still reaches app-core as a TUI chord and
//!   reads the clipboard with `arboard` there.
//! - Copy: app-core writes copied text (history entries, `/clip`,
//!   `/actionloop clip`, device codes, field selections) through
//!   `codemason_app_core::clipboard::set_clipboard_text`. On the UI thread
//!   that write is kept for the window ([`route_copies_to_window`]) and
//!   [`forward_copies`] sends it to `Context::copy_text`, so it lands on the
//!   clipboard egui-winit owns.
//!
//! - App paste: app-core's clipboard reads on the UI thread (Ctrl+Shift+V,
//!   `Alt+V`) never use `arboard` there: on Wayland the window serves its
//!   own selection from this thread, so that read deadlocked the window.
//!   They leave a paste request instead, and [`forward_copies`] answers it
//!   with egui's `RequestPaste`, which arrives as `Event::Paste` (or
//!   `Event::PasteImage`) next frame.
//!
//! Dependencies: `egui` (input events, `copy_text`) and
//! `codemason-app-core` (the attachment step and the copy routing).

use codemason_app_core::app::App;
use codemason_app_core::clipboard::{
    ImageData, route_copies_to_frontend, take_frontend_copies, take_frontend_paste_request,
};

use crate::dialogs;

/// Keep the clipboard text app-core writes on this thread for the window
/// instead of `arboard`. Idempotent; the shell calls it on the thread that
/// runs every App action.
pub fn route_copies_to_window() {
    route_copies_to_frontend(true);
}

/// Send app-core's text writes back to `arboard` on this thread (the shell
/// is gone).
pub fn stop_routing_copies() {
    route_copies_to_frontend(false);
}

/// Put the text app-core copied since the last call on the window's
/// clipboard, and answer app-core's paste request with egui's
/// `RequestPaste`. Only the last copy is sent: each would replace the one
/// before.
///
/// # Arguments
///
/// * `ctx` - the window's context.
pub fn forward_copies(ctx: &egui::Context) {
    if let Some(text) = take_frontend_copies().pop() {
        ctx.copy_text(text);
    }
    if take_frontend_paste_request() {
        ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }
}

/// Stage the images pasted this frame as attachments, through app-core's
/// clipboard-image step, and remove their events from the input.
///
/// While an app-core dialog owns the window ([`dialogs::locks_prompt`]) the
/// events are left alone, as `Alt+V` reaches no attachment there either.
///
/// # Arguments
///
/// * `ctx` - the window's context, before any widget reads the frame's
///   input.
/// * `app` - the application whose pending attachments receive the images.
///
/// # Returns
///
/// How many images were attached.
pub fn attach_pasted_images(ctx: &egui::Context, app: &mut App) -> usize {
    if dialogs::locks_prompt(app) {
        return 0;
    }
    let images = ctx.input_mut(|input| {
        let mut images = Vec::new();
        input.events.retain(|event| match event {
            egui::Event::PasteImage(image) => {
                images.push(std::sync::Arc::clone(image));
                false
            }
            _ => true,
        });
        images
    });
    images
        .iter()
        .filter(|image| app.attach_clipboard_image(&rgba_of(image)))
        .count()
}

/// The image's pixels as `arboard` RGBA (straight alpha, row by row).
fn rgba_of(image: &egui::ColorImage) -> ImageData<'static> {
    let bytes = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect::<Vec<u8>>();
    ImageData {
        width: image.size[0],
        height: image.size[1],
        bytes: std::borrow::Cow::Owned(bytes),
    }
}
