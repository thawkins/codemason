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
//! # Window geometry
//!
//! [`WindowGeometry`] keeps the window's placement in [`WINDOW_FILE`]
//! beside the dock layout file (`gui-layout.json`), through craft-window's
//! restore store (`craft_window::restore::Store`, one
//! `craft_window::restore::Placement` under the key `"main"`: the last
//! normal rect, its monitor scale, and the maximized flag).
//!
//! - **First launch** (no file): the window opens maximized.
//! - **Maximized at exit**: it reopens maximized; un-maximizing returns to the
//!   last normal rect.
//! - **Normal at exit** (un-maximized or resized): it reopens at that size
//!   and position.
//! - **Corrupt file** (craft-window moves it aside as
//!   `gui-window.broken-<ts>.json`) or a saved size that is no size: opens
//!   maximized.
//! - **Off screen**: when, on the connected monitors, craft-window's
//!   placement rules would have to move the saved rect (less than 64x64 pt of
//!   its title strip on any monitor, or larger than the monitor it lands on),
//!   the window is maximized instead ([`WindowGeometry::off_screen`]).
//!
//! The saved rect is applied before the window is created (eframe's
//! `NativeOptions::viewport`, [`WindowGeometry::viewport`]); monitors are
//! only known once it exists, so the off-screen check runs in the eframe
//! creator and maximizes the window before its first frame is shown. Each
//! frame records the window's state ([`WindowGeometry::track`]); a change
//! is written once it has held for a frame (not on every frame of a drag),
//! and on exit.
//!
//! Dependencies: `codemason-app-core` (the App and its quit path), `egui`,
//! `craft-quit` and `craft-window` (restore store and placement rules).

use std::path::{Path, PathBuf};

use codemason_app_core::app::App;
use craft_window::restore::{Defaults, MAIN, Monitor, Placement, Store, place};

/// File name of the saved window geometry, beside the dock layout file.
pub const WINDOW_FILE: &str = "gui-window.json";

/// The largest window size, in points, a saved placement may hold.
const MAX_POINTS: f32 = 100_000.0;

/// Size of a window that is un-maximized before any normal rect was seen.
const DEFAULT_SIZE: [f32; 2] = [1200.0, 800.0];

/// The smallest window craft-window's placement keeps, in points.
const MIN_SIZE: [f32; 2] = [480.0, 320.0];

/// The smallest interface scale a saved window may restore (egui's own
/// minimum for `Context::set_zoom_factor`'s useful range).
pub const ZOOM_MIN: f32 = 0.2;

/// The largest interface scale a saved window may restore.
pub const ZOOM_MAX: f32 = 5.0;

/// Where the window geometry is saved: [`WINDOW_FILE`] in the codemason
/// config directory, beside the dock layout file.
///
/// # Returns
///
/// The path, or `None` when the config directory is unknown.
#[must_use]
pub fn window_path() -> Option<PathBuf> {
    crate::dock::layout_path().and_then(|layout| layout.parent().map(|dir| dir.join(WINDOW_FILE)))
}

/// The window's saved placement and what the running window reports.
#[derive(Debug)]
pub struct WindowGeometry {
    /// The restore store over the geometry file (or in memory).
    store: Store,
    /// The placement recorded last frame, to write a change once it holds.
    last: Option<Placement>,
    /// The last write failed and was reported; later failures stay quiet
    /// until a write succeeds (a persistent error would warn every frame).
    save_failed: bool,
}

impl WindowGeometry {
    /// Open the geometry saved at `path`. A missing file is a first launch;
    /// a corrupt one is moved aside by craft-window and treated as missing.
    ///
    /// # Arguments
    ///
    /// * `path` - the geometry file, e.g. [`window_path`].
    ///
    /// # Returns
    ///
    /// The geometry over that file.
    #[must_use]
    pub fn open(path: &Path) -> Self {
        Self {
            store: Store::at(path),
            last: None,
            save_failed: false,
        }
    }

    /// Geometry that is neither read nor written (no config directory).
    ///
    /// # Returns
    ///
    /// An empty in-memory geometry: the window opens maximized.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            store: Store::in_memory(),
            last: None,
            save_failed: false,
        }
    }

    /// The saved placement, when it holds a usable size.
    fn saved(&self) -> Option<Placement> {
        self.store.get(MAIN).filter(|p| {
            p.normal
                .size
                .iter()
                .all(|v| v.is_finite() && (1.0..=MAX_POINTS).contains(v))
        })
    }

    /// Apply the saved geometry to the window before it is created: the
    /// saved normal rect, maximized as it was left; maximized when nothing
    /// usable is saved (first launch, corrupt file).
    ///
    /// # Arguments
    ///
    /// * `builder` - the viewport eframe will create.
    ///
    /// # Returns
    ///
    /// The viewport with its size, position and maximized state set.
    #[must_use]
    pub fn viewport(&self, builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
        let Some(saved) = self.saved() else {
            return builder.with_maximized(true);
        };
        let mut builder = builder
            .with_inner_size(saved.normal.size)
            .with_maximized(saved.maximized);
        if let Some(px) = saved.position_px {
            let scale = if saved.scale.is_finite() && saved.scale > 0.0 {
                saved.scale
            } else {
                1.0
            };
            builder = builder.with_position([px[0] as f32 / scale, px[1] as f32 / scale]);
        }
        builder
    }

    /// Whether the saved normal rect cannot be shown as saved on
    /// `monitors`: craft-window's placement would move it (its title strip
    /// is on no monitor, or it is larger than the monitor it lands on). The
    /// window is then maximized instead. `false` without a saved position or
    /// without monitors (nothing to check against).
    ///
    /// # Arguments
    ///
    /// * `monitors` - the connected monitors, in physical pixels.
    ///
    /// # Returns
    ///
    /// `true` when the saved rect is off screen.
    #[must_use]
    pub fn off_screen(&self, monitors: &[Monitor]) -> bool {
        let saved = self.last.clone().or_else(|| self.saved());
        let Some(saved) = saved.filter(|p| p.position_px.is_some()) else {
            return false;
        };
        if monitors.is_empty() {
            return false;
        }
        let placed = place(
            Some(&saved),
            monitors,
            None,
            &Defaults::new(DEFAULT_SIZE, MIN_SIZE),
        );
        placed.position_px != saved.position_px
            || placed
                .normal
                .size
                .iter()
                .zip(saved.normal.size)
                .any(|(placed, saved)| (placed - saved).abs() > 0.5)
    }

    /// Record the window state eframe reports this frame. A minimized
    /// window records nothing; a maximized or full-screen one keeps the last
    /// normal rect and records only its flags.
    ///
    /// # Arguments
    ///
    /// * `info` - the root viewport's info.
    /// * `zoom` - the egui zoom factor (`Context::zoom_factor`): sizes are
    ///   saved at zoom 1, and the factor itself is kept so the interface
    ///   scale survives a restart ([`Self::saved_zoom`]).
    pub fn record(&mut self, info: &egui::ViewportInfo, zoom: f32) {
        if info.minimized == Some(true) {
            return;
        }
        let maximized = info.maximized == Some(true);
        let fullscreen = info.fullscreen == Some(true);
        let scale = info.native_pixels_per_point.unwrap_or(1.0);
        let mut placement = self
            .last
            .clone()
            .or_else(|| self.saved())
            .unwrap_or_else(|| Placement::new(DEFAULT_SIZE, scale));
        if !maximized && !fullscreen {
            let Some(inner) = info.inner_rect else {
                return;
            };
            let mut normal = Placement::new((inner.size() * zoom).into(), scale);
            if let Some(outer) = info.outer_rect {
                let px = outer.min.to_vec2() * zoom * scale;
                normal.position_px = Some([px.x.round() as i32, px.y.round() as i32]);
                normal.normal.pos = (outer.min.to_vec2() * zoom).into();
            }
            placement = normal;
        }
        // Recorded whether or not the window is maximized: the scale is the
        // user's interface setting, not part of the rect.
        placement.zoom = zoom;
        placement.maximized = maximized;
        placement.fullscreen = fullscreen;
        self.store.set(MAIN, &placement);
        self.last = Some(placement);
    }

    /// The interface scale saved with the geometry: the zoom factor the
    /// window last ran at, `1.0` when nothing usable is saved.
    ///
    /// The status bar's slider and Ctrl+Plus / Ctrl+Minus set egui's zoom
    /// factor at run time, and eframe does not persist it, so without this
    /// the scale was back to 100% on the next start.
    ///
    /// # Returns
    ///
    /// The saved zoom factor, clamped to the range egui accepts.
    #[must_use]
    pub fn saved_zoom(&self) -> f32 {
        let zoom = self.saved().map(|p| p.zoom).unwrap_or(1.0);
        if zoom.is_finite() {
            zoom.clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            1.0
        }
    }

    /// Record this frame's window state ([`Self::record`]) and write a
    /// change once it has held since the previous frame (so a drag is not
    /// written on every frame).
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    pub fn track(&mut self, ctx: &egui::Context) {
        let before = self.last.clone();
        let info = ctx.input(|i| i.viewport().clone());
        self.record(&info, ctx.zoom_factor());
        if before.is_some() && before == self.last {
            self.save();
        }
    }

    /// Write the geometry file if it changed since the last write. A
    /// failure is warned about once, until a write succeeds again.
    pub fn save(&mut self) {
        match self.store.save() {
            Ok(_) => self.save_failed = false,
            Err(err) => {
                if !self.save_failed {
                    tracing::warn!(%err, "cannot save the window geometry");
                }
                self.save_failed = true;
            }
        }
    }
}

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
                    ui.add_space(crate::metrics::space::XS);
                    ui.label("An agent turn is running. Quitting cancels it.");
                    ui.add_space(crate::metrics::space::SM);
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
