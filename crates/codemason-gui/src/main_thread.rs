//! Handoff of a built GUI session to the process main thread.
//!
//! eframe must own the process main thread (macOS winit refuses any other
//! thread), but `cdmn` runs its tokio runtime and startup work on a
//! dedicated `codemason-main` thread. The two meet here:
//!
//! - the binary creates a [`main_thread_channel`] before spawning its
//!   program thread, keeps the [`MainThreadHost`] on the main thread and
//!   hands the [`GuiLauncher`] to the program thread;
//! - [`crate::run_gui`] builds the `App` on the runtime thread and sends it,
//!   with the event and log receivers and a runtime [`Handle`], through
//!   [`GuiLauncher::launch`], then awaits the window's result;
//! - [`MainThreadHost::serve`] runs each received [`GuiLaunch`] in an eframe
//!   window and returns once every launcher is dropped (the program thread
//!   finished), so the main thread can join it without hanging.
//!
//! Dependencies: `std::sync::mpsc` for the handoff, a `tokio` oneshot for the
//! result, and `eframe` for the window.

use std::sync::mpsc;

use codemason_agent::event::Event;
use codemason_app_core::app::App;
use codemason_app_core::log_channel::TuiLogReceiver;
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use craft_window::restore::{Monitor, MonitorKey};

use crate::platform::window::{self, WindowGeometry};
use crate::{GuiLaunchError, Shell, bridge, dock, keys, signal};

/// Window title and eframe application name.
const APP_NAME: &str = "codemason";

/// Everything the main thread needs to show one GUI session.
pub struct GuiLaunch {
    /// The application state, built on the runtime thread.
    pub app: App,
    /// The agent event-bus receiver the bridge drains, subscribed before
    /// the App's startup ran so the events it published are queued.
    pub events: tokio::sync::broadcast::Receiver<Event>,
    /// Tracing records for the log panel.
    pub logs: TuiLogReceiver,
    /// The runtime that drives agent work and `App::handle_event`.
    pub runtime: Handle,
}

impl std::fmt::Debug for GuiLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiLaunch").finish_non_exhaustive()
    }
}

/// A launch plus the channel its window result goes back on.
type LaunchRequest = (GuiLaunch, oneshot::Sender<Result<(), GuiLaunchError>>);

/// Sending side of the handoff, held by the runtime thread.
#[derive(Debug, Clone)]
pub struct GuiLauncher {
    tx: mpsc::Sender<LaunchRequest>,
}

/// Receiving side of the handoff, held by the process main thread.
#[derive(Debug)]
pub struct MainThreadHost {
    rx: mpsc::Receiver<LaunchRequest>,
}

/// Create a connected launcher/host pair.
///
/// # Returns
///
/// The [`GuiLauncher`] for the runtime thread and the [`MainThreadHost`]
/// for the process main thread.
#[must_use]
pub fn main_thread_channel() -> (GuiLauncher, MainThreadHost) {
    let (tx, rx) = mpsc::channel();
    (GuiLauncher { tx }, MainThreadHost { rx })
}

impl GuiLauncher {
    /// Send `launch` to the main thread and wait until its window closes.
    ///
    /// # Arguments
    ///
    /// * `launch` - the built session to show.
    ///
    /// # Returns
    ///
    /// The window's result: `Ok(())` once it closed normally.
    ///
    /// # Errors
    ///
    /// Returns [`GuiLaunchError::Backend`] when the main-thread host is gone
    /// or dropped the request, or the error the window itself reported.
    pub async fn launch(&self, launch: GuiLaunch) -> Result<(), GuiLaunchError> {
        let (done_tx, done_rx) = oneshot::channel();
        self.tx
            .send((launch, done_tx))
            .map_err(|_| GuiLaunchError::Backend("main-thread GUI host is not running".into()))?;
        done_rx.await.map_err(|_| {
            GuiLaunchError::Backend("main-thread GUI host dropped the launch".into())
        })?
    }
}

impl MainThreadHost {
    /// Run every received launch in an eframe window on the calling thread,
    /// which must be the process main thread.
    ///
    /// Returns once every [`GuiLauncher`] is dropped. When no launch ever
    /// arrives (a non-GUI run) it simply waits for the program thread's
    /// launcher to drop.
    pub fn serve(self) {
        self.serve_with(run_window);
    }

    /// Like [`Self::serve`], with `run` in place of the eframe window.
    ///
    /// # Arguments
    ///
    /// * `run` - shows one launch and returns its result.
    pub fn serve_with(self, mut run: impl FnMut(GuiLaunch) -> Result<(), GuiLaunchError>) {
        while let Ok((launch, done)) = self.rx.recv() {
            let result = run(launch);
            let _ = done.send(result); // INTENTIONAL: a dropped receiver means run_gui is gone
        }
    }
}

/// Show `launch` in an eframe window until the window closes.
fn run_window(launch: GuiLaunch) -> Result<(), GuiLaunchError> {
    let GuiLaunch {
        app,
        events,
        logs,
        runtime,
    } = launch;
    let mut bridge_task = None;
    let mut signal_task = None;
    // The saved geometry opens the window as the user left it (maximized
    // on first launch).
    let geometry = window::window_path().map_or_else(WindowGeometry::in_memory, |path| {
        WindowGeometry::open(&path)
    });
    // Who draws the window frame, decided once: the app's title strip is
    // the title bar where the desktop allows it ([`crate::title_bar`]).
    let frame = crate::title_bar::decide_frame().frame;
    let viewport = geometry.viewport(egui::ViewportBuilder::default().with_title(APP_NAME));
    let native_options = eframe::NativeOptions {
        viewport: craft_window::policy::apply(viewport, &frame),
        ..Default::default()
    };
    // The whole window runs inside the runtime context: the creator, every
    // eframe callback and the drop of the App when the window closes may
    // call App code that spawns on the runtime (the shell also enters it per
    // frame, for harnesses that drive it without this host).
    let host_context = runtime.enter();
    let result = eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| {
            // Bundled block/status glyphs and a bold face, then installed
            // system fonts for CJK and every other script.
            crate::fonts::install(&cc.egui_ctx, craft_ui::fonts::SystemFonts::AfterCraft);
            // `file://` image loading, for the transcript's image thumbnails.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
            let _enter = runtime.enter();
            // The bridge adds broadcast lags to the App's counter, so the
            // Agents panel reconcile repairs dropped subagent events.
            bridge_task = Some(bridge::spawn_event_bridge(
                events,
                cc.egui_ctx.clone(),
                event_tx,
                std::sync::Arc::clone(&app.tui_event_lag),
            ));
            // Ctrl+C / SIGTERM quit through the normal quit path.
            let (shutdown, task) = signal::spawn_shutdown_signal_task(cc.egui_ctx.clone());
            signal_task = Some(task);
            // Monitors are known only now that the window exists: a saved
            // rect no monitor shows opens maximized instead.
            if let Some(window) = cc.winit_window() {
                let monitors: Vec<Monitor> = window
                    .available_monitors()
                    .map(|m| {
                        let size = m.size();
                        let pos = m.position();
                        Monitor::new(
                            MonitorKey::new(
                                m.name().unwrap_or_default(),
                                [size.width, size.height],
                            ),
                            [pos.x, pos.y],
                            [size.width, size.height],
                            m.scale_factor() as f32,
                        )
                    })
                    .collect();
                if geometry.off_screen(&monitors) {
                    cc.egui_ctx
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                }
            }
            let mut shell = Shell::new(app)
                .with_title_bar(frame.clone())
                .with_sources(event_rx, logs, runtime.clone())
                .with_window_geometry(geometry)
                .with_force_exit()
                .with_shutdown_signal(shutdown);
            if let Some(path) = dock::layout_path() {
                shell = shell.with_layout_file(path);
            }
            let platform = craft_keymap::egui::platform(&cc.egui_ctx);
            let keymap = match keys::keys_path() {
                Some(path) => keys::load_keymap(platform, &path),
                None => keys::default_keymap(platform),
            };
            // The macOS menu bar (About, Quit asking first, the panel
            // toggles); nothing elsewhere.
            #[cfg(target_os = "macos")]
            if let Some(native) = crate::platform::menu::install(&cc.egui_ctx, &keymap) {
                shell = shell.with_native_menu(native, &keymap);
            }
            shell = shell.with_keymap(keymap);
            Ok(Box::new(shell))
        }),
    );
    drop(host_context);
    if let Some(task) = bridge_task {
        task.abort();
    }
    if let Some(task) = signal_task {
        task.abort();
    }
    result.map_err(|e| GuiLaunchError::Backend(e.to_string()))
}
