//! The About dialog: the CodeMason logo, the version and a link to the
//! GitHub repository.
//!
//! The Help tab's About ([`crate::ribbon`]) and the macOS app menu's About
//! ([`crate::platform::menu`]) open it ([`open`]); [`show`] draws it in a
//! Material dialog until the Close button, Esc or a click outside closes
//! it. While it is open the shell sends no keys or menu commands to the App
//! ([`is_open`]), so nothing acts on the session behind it.
//! The logo is the primary lockup drawn for the theme's background: the
//! light-on-dark one on a dark theme, the dark-on-light one on a light theme.
//!
//! Whether it is open is GUI state (egui memory), not App state: no other
//! frontend draws it.
//!
//! Dependencies: `egui`, and this crate's Material components and palette.
//! The logo PNGs are embedded from `assets/branding`.

use egui::Id;

use crate::dialogs::material_modal_response;
use crate::material::button::{ButtonKind, button};
use crate::material::dialog::dialog_actions;
use crate::metrics::space;
use crate::palette::Palette;

/// Title of the dialog, also its accessible name.
pub const ABOUT_TITLE: &str = "About CodeMason";

/// The button that closes the dialog.
pub const CLOSE_ABOUT: &str = "Close";

/// The version shown, the workspace's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The GitHub repository the dialog links to.
pub const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// The logo drawn on a dark theme: light lettering on a dark ground.
const LOGO_DARK: &[u8] =
    include_bytes!("../../../../assets/branding/CodeMason Logo-png/Primary lockup — dark.png");

/// The logo drawn on a light theme: dark lettering on a light ground.
const LOGO_LIGHT: &[u8] =
    include_bytes!("../../../../assets/branding/CodeMason Logo-png/Primary lockup — light.png");

/// The logos' size in pixels (both PNGs are the same size).
const LOGO_PIXELS: egui::Vec2 = egui::vec2(1760.0, 960.0);

/// The width the logo is drawn at, in points.
const LOGO_WIDTH: f32 = 440.0;

/// The egui URI of the logo for a dark (`dark`) or light theme.
///
/// # Arguments
///
/// * `dark` - whether the theme is dark.
///
/// # Returns
///
/// The `bytes://` URI the dialog loads the logo from.
#[must_use]
pub fn logo_uri(dark: bool) -> &'static str {
    if dark {
        "bytes://codemason-about-logo-dark.png"
    } else {
        "bytes://codemason-about-logo-light.png"
    }
}

/// The egui memory slot of whether the dialog is open.
fn open_id() -> Id {
    Id::unique("codemason_about")
}

/// Open the About dialog.
///
/// # Arguments
///
/// * `ctx` - the window's context.
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(open_id(), true));
}

/// Whether the About dialog is open.
///
/// # Arguments
///
/// * `ctx` - the window's context.
#[must_use]
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false)
}

/// Draw the dialog while it is open.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `palette` - the colours; `palette.dark` picks the logo.
///
/// # Returns
///
/// `true` while the dialog is open.
pub fn show(ctx: &egui::Context, palette: &Palette) -> bool {
    if !is_open(ctx) {
        return false;
    }
    let uri = logo_uri(palette.dark);
    let bytes = if palette.dark { LOGO_DARK } else { LOGO_LIGHT };
    let mut close = false;
    // The Material dialog keeps inside a small window: narrower than the
    // logo's width if need be, its content scrolling past the window's height.
    let modal = material_modal_response(
        ctx,
        palette,
        "codemason_about_modal",
        ABOUT_TITLE,
        LOGO_WIDTH,
        |ui| {
            // The logo shrinks with the dialog, keeping its shape.
            let width = ui.available_width().min(LOGO_WIDTH);
            ui.add(
                egui::Image::from_bytes(uri, bytes)
                    .fit_to_exact_size(LOGO_PIXELS * (width / LOGO_PIXELS.x))
                    .alt_text("CodeMason logo"),
            );
            ui.add_space(space::LG);
            ui.vertical_centered(|ui| {
                ui.label(format!("Version {VERSION}"));
                // Material has no link component; egui's hyperlink takes
                // the theme's link colour (crate::theme).
                ui.hyperlink_to(REPOSITORY_URL, REPOSITORY_URL);
            });
            ui.add_space(space::LG);
            dialog_actions(ui, |ui| {
                close = button(ui, palette, ButtonKind::Text, None, CLOSE_ABOUT).clicked();
            });
        },
    );
    // Esc or a click outside closes it too.
    if close || modal.should_close() {
        ctx.data_mut(|d| d.remove::<bool>(open_id()));
    }
    true
}
