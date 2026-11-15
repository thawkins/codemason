//! The window lifecycle: what closing the window does.
//!
//! # Close and quit
//!
//! A close request (the close button, Cmd+Q from the macOS menu, the OS)
//! reaches [`CloseGuard::logic`], which the shell runs in `eframe::App::logic`
//! (so it also runs while the window is hidden):
//!
//! - while an agent turn runs (`App::turn_running`) the close is cancelled
//!   (`ViewportCommand::CancelClose`) and [`CloseGuard::show`] asks
//!   ([`QUIT_TITLE`]). [`CONFIRM_QUIT`] quits through app-core's quit path
//!   (`App::quit`, what `/quit` runs); [`CANCEL_QUIT`] (or Esc) keeps the
//!   window and the turn. A hidden or minimised window is brought back so
//!   the question can be answered;
//! - otherwise the close quits at once, through the same `App::quit`.
//!
//! Once the App has stopped, by any path (`/quit`, `/exit`, Ctrl+C then
//! Ctrl+D, a confirmed or idle close), the shell runs app-core's exit steps
//! once (`App::save_on_quit`, `App::stop_services_on_quit`: the input queue
//! dropped, dirty history saved, the turn cancelled, the swarm hook jobs
//! aborted, the MCP servers and code indexes stopped) and closes the window.
//! A close that arrives after the App stopped is never questioned, so
//! `/quit` and Ctrl+D never show the dialog. [`craft_quit::on_close_request`]
//! runs last: on macOS it turns an accepted close into
//! `-[NSApplication terminate:]`, which ends the process before AppKit can
//! crash on the dropped window; elsewhere it does nothing.
//!
//! Dependencies: `codemason-app-core` (the App and its quit path), `egui`,
//! and `craft-quit`.

use codemason_app_core::app::App;

/// Title of the quit confirmation, shown when the window is closed while an
/// agent turn runs.
pub const QUIT_TITLE: &str = "Quit codemason?";

/// The quit confirmation's button that cancels the turn and quits.
pub const CONFIRM_QUIT: &str = "Quit";

/// The quit confirmation's button that keeps the window and the turn.
pub const CANCEL_QUIT: &str = "Cancel";

/// Decides what a close request does, and asks when an agent turn would be
/// cancelled.
#[derive(Debug, Default)]
pub struct CloseGuard {
    /// The quit confirmation is shown.
    asking: bool,
}

impl CloseGuard {
    /// Act on this frame's close request: ask while a turn runs, otherwise
    /// quit through `App::quit`. Nothing happens without a request, or once
    /// the App has stopped (the close is then the one the shell sent).
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    /// * `app` - the App to quit.
    pub fn logic(&mut self, ctx: &egui::Context, app: &mut App) {
        if !app.is_running || !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if app.turn_running() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.asking {
                self.asking = true;
                // The question must be answerable: bring the window back.
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            ctx.request_repaint();
        } else {
            app.quit();
        }
    }

    /// Whether the quit confirmation is shown (the shell routes no keys to
    /// the App meanwhile, so Esc cancels the dialog, not the turn).
    ///
    /// # Returns
    ///
    /// `true` while the dialog asks.
    #[must_use]
    pub fn is_asking(&self) -> bool {
        self.asking
    }

    /// Draw the quit confirmation while it asks. [`CONFIRM_QUIT`] quits
    /// through `App::quit`; [`CANCEL_QUIT`], Esc or a click outside closes the
    /// dialog and keeps the window.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the frame's context.
    /// * `app` - the App to quit.
    pub fn show(&mut self, ctx: &egui::Context, app: &mut App) {
        if !self.asking {
            return;
        }
        if !app.is_running {
            self.asking = false;
            return;
        }
        let response =
            egui::Modal::new(egui::Id::unique("codemason_quit_confirmation")).show(ctx, |ui| {
                let inner = ui.vertical(|ui| {
                    ui.heading(QUIT_TITLE);
                    ui.add_space(4.0);
                    ui.label("An agent turn is running. Quitting cancels it.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let quit = ui.button(CONFIRM_QUIT).clicked();
                        let cancel = ui.button(CANCEL_QUIT).clicked();
                        (quit, cancel)
                    })
                    .inner
                });
                inner.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Dialog, true, QUIT_TITLE)
                });
                inner.inner
            });
        let (quit, cancel) = response.inner;
        if quit {
            self.asking = false;
            app.quit();
            ctx.request_repaint();
        } else if cancel || response.should_close() {
            self.asking = false;
            ctx.request_repaint();
        }
    }
}
