//! The diff overlay: a transcript diff shown whole, over the window.
//!
//! Open diff under an inline diff ([`crate::panels::diff::inline_diff_ui`])
//! opens it ([`open`]) on the message's diffs in `App::diff_pane`; [`show`]
//! draws them in a modal with the Diff pane's own view
//! ([`crate::panels::diff::diff_view_ui`]), whose close button sits at the
//! top right. The close button or a click outside closes it. While it is
//! open the Diff pane's keys (`]`, `[`, `n`, `p`, `v`, `m`) act on it.
//!
//! Whether it is open is GUI state (egui memory), not App state: the
//! terminal frontend has no overlay.
//!
//! Dependencies: `egui`, `codemason-app-core` (the diff pane state), and
//! this crate's Material dialog frame and palette.

use codemason_app_core::app::App;
use egui::Id;

use crate::material::dialog::dialog_frame;
use crate::palette::Palette;

/// The accessible name of the overlay.
pub const DIFF_VIEW_TITLE: &str = "Diff overlay";

/// The share of the window the overlay takes, each way.
const BODY_SHARE: f32 = 0.85;

/// The egui memory slot of whether the overlay is open.
fn open_id() -> Id {
    Id::unique("codemason_diff_view_open")
}

/// Open the overlay on the files `App::diff_pane` holds.
///
/// # Arguments
///
/// * `ctx` - the window's context.
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(open_id(), true));
}

/// Whether the overlay is open.
///
/// # Arguments
///
/// * `ctx` - the window's context.
#[must_use]
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false)
}

/// Draw the overlay while it is open.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `diff_pane` holds the files.
/// * `palette` - the colours.
///
/// # Returns
///
/// `true` while the overlay is open.
pub fn show(ctx: &egui::Context, app: &mut App, palette: &Palette) -> bool {
    if !is_open(ctx) {
        return false;
    }
    let size = ctx.content_rect().size() * BODY_SHARE;
    let mut closed = false;
    let modal = egui::Modal::new(Id::unique("codemason_diff_view_modal"))
        .frame(dialog_frame(palette))
        .show(ctx, |ui| {
            // An exact size, so the hunks' scroll area fills it rather than
            // growing the overlay frame after frame.
            let inner = ui.allocate_ui(size, |ui| {
                ui.set_min_size(size);
                closed = crate::panels::diff::diff_view_ui(ui, &mut app.diff_pane, palette);
            });
            inner.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Dialog, true, DIFF_VIEW_TITLE)
            });
        });
    if closed || modal.backdrop_response.clicked() {
        ctx.data_mut(|d| d.remove::<bool>(open_id()));
    }
    true
}
