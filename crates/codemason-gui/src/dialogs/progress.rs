//! The model-list loading spinner (`App::model_loading_state`) and the model
//! download progress bar (`App::model_download_state`).
//!
//! app-core sets and clears both from agent events
//! (`ProviderLoadingStarted`/`Finished`, `ModelDownloadStarted`/
//! `Progress`/`Finished`) and its housekeeping pass reaps a latch that
//! outlived its staleness cap. Neither takes keys of its own, so, as in the
//! TUI, they are not part of the modal dispatch: each is a small labelled
//! window at the top centre, drawn whenever its state is set, over whatever
//! else is on screen. While one is shown a repaint is requested every
//! quarter second so the spinner and elapsed time advance.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use std::time::Duration;

use codemason_app_core::app::App;
use egui::Id;

/// Accessible label of the model-list loading window.
pub const LOADING_LABEL: &str = "Loading models";

/// Accessible label of the model download window.
pub const DOWNLOAD_LABEL: &str = "Model download";

/// The TUI's spinner frames, in ASCII.
const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

/// How often a shown window repaints, so its spinner and timer advance.
const TICK: Duration = Duration::from_millis(250);

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

/// Draw a small window at the top centre labelled `label`.
fn window(ctx: &egui::Context, id: &str, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Area::new(Id::unique(id))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let inner = ui.vertical(|ui| add(ui));
                inner.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, label)
                });
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
    if let Some(state) = app.model_loading_state.as_ref() {
        let elapsed = state.started_at.elapsed();
        let frame = SPINNER[(elapsed.as_millis() / TICK.as_millis()) as usize % SPINNER.len()];
        window(ctx, "codemason_model_loading", LOADING_LABEL, |ui| {
            ui.strong(format!(
                "{} ({})",
                loading_text(&state.provider_name),
                state.provider_id
            ));
            ui.label(format!("{frame} Fetching model list..."));
            ui.weak("Esc to cancel");
        });
        ctx.request_repaint_after(TICK);
    }
    if let Some(state) = app.model_download_state.as_ref() {
        let percent = state.percent.clamp(0.0, 100.0);
        let elapsed = codemason_types::strutil::format_mmss(state.started_at.elapsed().as_secs());
        window(ctx, "codemason_model_download", DOWNLOAD_LABEL, |ui| {
            ui.strong(format!(
                "{} ({})",
                download_text(&state.model_id),
                state.provider_id
            ));
            ui.label(format!("{percent:.1}% * {elapsed} elapsed"));
            ui.add(
                egui::ProgressBar::new(percent / 100.0)
                    .text(format!("{percent:.0}%"))
                    .desired_width(240.0),
            );
        });
        ctx.request_repaint_after(TICK);
    }
}
