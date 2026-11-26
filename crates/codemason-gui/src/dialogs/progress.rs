//! The model-list loading spinner (`App::model_loading_state`) and the model
//! download progress bar (`App::model_download_state`).
//!
//! app-core sets and clears both from agent events
//! (`ProviderLoadingStarted`/`Finished`, `ModelDownloadStarted`/
//! `Progress`/`Finished`) and its housekeeping pass reaps a latch that
//! outlived its staleness cap. Neither takes keys of its own, so, as in the
//! TUI, they are not part of the modal dispatch: each is a small labelled
//! window at the top centre, drawn whenever its state is set, over whatever
//! else is on screen.
//!
//! Each window is a Material card: a `surface_container_highest` fill at
//! elevation 2, its label in `on_surface_variant` small type, the heading as
//! the message, and the detail under it. The loading window's spinner is
//! [`circular_progress`] (the indeterminate M3 indicator); the download
//! window keeps [`linear_progress`], because its progress is known and a
//! bar shows how far it has come. While one is shown a repaint is requested
//! every quarter second so the elapsed time advances (the circular
//! indicator asks for its own next frame, at 30 frames a second).
//!
//! Dependencies: `codemason-app-core`, `egui`, and this crate's Material
//! card and feedback.

use std::time::Duration;

use codemason_app_core::app::App;
use egui::Id;

use crate::material::card::{CardKind, card_frame};
use crate::material::feedback::{circular_progress, linear_progress};
use crate::metrics::space;
use crate::palette::{Palette, palette_for, rgb};

/// Accessible label of the model-list loading window.
pub const LOADING_LABEL: &str = "Loading models";

/// Accessible label of the model download window.
pub const DOWNLOAD_LABEL: &str = "Model download";

/// How often a shown window repaints, so its elapsed time advances.
const TICK: Duration = Duration::from_millis(250);

/// Width of a window's card, in points.
const WIDTH: f32 = 320.0;

/// Accessible label of the loading window's spinner: distinct from the
/// window's own, so the card and its indicator are two nodes.
const SPINNER_LABEL: &str = "Fetching the model list";

/// The text of the loading window.
#[must_use]
pub fn loading_text(provider_name: &str) -> String {
    format!("Loading models for {provider_name}")
}

/// The text of the download window.
#[must_use]
pub fn download_text(model_id: &str) -> String {
    format!("Downloading {model_id}")
}

/// Draw a small card at the top centre labelled `label`.
///
/// The card is `surface_container_highest` at elevation 2 (Material's
/// filled card, raised from the window behind it), `label` in
/// `on_surface_variant` small type above `add`.
fn window(
    ctx: &egui::Context,
    palette: &Palette,
    id: &str,
    label: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    egui::Area::new(Id::unique(id))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .show(ctx, |ui| {
            let inner = card_frame(CardKind::Filled, palette)
                .shadow(crate::material::elevation_shadow(2, palette))
                .show(ui, |ui| {
                    ui.set_width(WIDTH);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(label)
                                .size(crate::metrics::text_size::SMALL)
                                .color(rgb(palette.m3.on_surface_variant)),
                        );
                        ui.add_space(space::XS);
                        add(ui);
                    })
                });
            inner.response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, label)
            });
        });
}

/// Draw the loading and download windows while the App has them.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose model loading and download states are drawn.
pub fn show(ctx: &egui::Context, app: &App) {
    let palette = palette_for(app, ctx);
    if let Some(state) = app.model_loading_state.as_ref() {
        window(
            ctx,
            &palette,
            "codemason_model_loading",
            LOADING_LABEL,
            |ui| {
                ui.horizontal(|ui| {
                    circular_progress(ui, &palette, SPINNER_LABEL);
                    ui.add_space(space::MD);
                    ui.vertical(|ui| {
                        ui.strong(format!(
                            "{} ({})",
                            loading_text(&state.provider_name),
                            state.provider_id
                        ));
                        ui.add_space(space::XS);
                        ui.label(
                            egui::RichText::new("Fetching model list...")
                                .color(rgb(palette.m3.on_surface_variant)),
                        );
                    });
                });
                ui.add_space(space::SM);
                ui.label(
                    egui::RichText::new("Esc to cancel")
                        .size(crate::metrics::text_size::SMALL)
                        .color(rgb(palette.m3.on_surface_variant)),
                );
            },
        );
        ctx.request_repaint_after(TICK);
    }
    if let Some(state) = app.model_download_state.as_ref() {
        let percent = state.percent.clamp(0.0, 100.0);
        let elapsed = codemason_types::strutil::format_mmss(state.started_at.elapsed().as_secs());
        window(
            ctx,
            &palette,
            "codemason_model_download",
            DOWNLOAD_LABEL,
            |ui| {
                ui.strong(format!(
                    "{} ({})",
                    download_text(&state.model_id),
                    state.provider_id
                ));
                ui.add_space(space::XS);
                ui.label(
                    egui::RichText::new(format!("{percent:.1}% * {elapsed} elapsed"))
                        .color(rgb(palette.m3.on_surface_variant)),
                );
                ui.add_space(space::SM);
                linear_progress(ui, &palette, Some(percent / 100.0));
            },
        );
        ctx.request_repaint_after(TICK);
    }
}
