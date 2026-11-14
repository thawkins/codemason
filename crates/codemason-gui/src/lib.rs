//! Desktop GUI frontend for codemason, built on eframe/egui.
//!
//! The crate renders the frontend-neutral [`codemason_app_core::app::App`]
//! in a native window. It is the GUI counterpart of `codemason-tui` and must
//! not depend on it.
//!
//! - [`run_gui`] is the entry point the `cdmn --gui` launch path calls.
//! - [`main_thread`] hands the built session to the process main thread,
//!   which runs the eframe window ([`MainThreadHost::serve`]).
//! - [`Shell`] is the `eframe::App` that draws the window.
//! - [`dock`] is the dockable pane layout model (craft-ui docking).
//! - [`keys`] is the keyboard shortcut map (craft-keymap).
//! - [`panels`] holds the pane contents ([`panels::messages`] is the
//!   transcript), drawn with [`palette`] colours and [`styled_text`] layout.
//! - [`bridge::spawn_event_bridge`] forwards agent events from the
//!   [`EventBus`] into the shell's queue and wakes egui.
//!
//! Dependencies: `eframe`/`egui` (the storytold forks, via the workspace
//! `[patch.crates-io]` table), `craft-ui` (docking),
//! `craft-keymap` (keyboard shortcuts), `tokio`, `thiserror`
//! and `tracing`.

#![deny(unsafe_code)]

pub mod bridge;
pub mod dock;
pub mod keys;
pub mod main_thread;
pub mod palette;
pub mod panels;
pub mod shell;
pub mod styled_text;

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::StartupTimings;
use codemason_agent::agent::AgentInfo;
use codemason_agent::event::EventBus;
use codemason_agent::provider::ProviderRegistry;
use codemason_agent::session::processor::SessionProcessor;
use codemason_agent::storage::Storage;
use codemason_app_core::app::{App, StartupSession};
use codemason_app_core::log_channel::TuiLogReceiver;
use codemason_types::stderr_spool::Spool as StderrSpool;

pub use main_thread::{GuiLaunch, GuiLauncher, MainThreadHost, main_thread_channel};
pub use shell::Shell;

/// Why the GUI could not be started.
#[derive(Debug, thiserror::Error)]
pub enum GuiLaunchError {
    /// No graphical display is reachable (neither `DISPLAY` nor
    /// `WAYLAND_DISPLAY` is set on an X11/Wayland platform).
    #[error(
        "no graphical display found: neither DISPLAY nor WAYLAND_DISPLAY is set; \
         run cdmn without --gui to use the terminal UI"
    )]
    NoDisplay,
    /// The windowing or rendering backend failed to start or run.
    #[error("GUI backend error: {0}")]
    Backend(String),
}

/// Everything [`run_gui`] needs: the same inputs `codemason_tui::run_tui`
/// takes, gathered in one struct.
pub struct GuiOptions {
    /// Shared agent event bus.
    pub event_bus: Arc<EventBus>,
    /// Session and message storage.
    pub storage: Arc<Storage>,
    /// Registry of configured LLM providers.
    pub provider_registry: Arc<ProviderRegistry>,
    /// The session processor that runs agent turns.
    pub session_processor: Arc<SessionProcessor>,
    /// The agent the first session starts with.
    pub agent: AgentInfo,
    /// Whether the log panel starts visible.
    pub show_log: bool,
    /// Which session to focus first (restore, new, or resume by id).
    pub startup: StartupSession,
    /// Tracing records captured for the log panel.
    pub log_rx: TuiLogReceiver,
    /// Path of the session database.
    pub db_path: PathBuf,
    /// Config files that were loaded, for the startup announcement.
    pub config_paths: Vec<PathBuf>,
    /// Startup phase timings recorded so far.
    pub timings: StartupTimings,
    /// Raw-stderr spool, when the binary installed one.
    pub stderr_spool: Option<StderrSpool>,
    /// Hands the built session to the process main thread's
    /// [`MainThreadHost`], which runs the window.
    pub launcher: GuiLauncher,
}

impl std::fmt::Debug for GuiOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuiOptions")
            .field("agent", &self.agent.name)
            .field("show_log", &self.show_log)
            .field("db_path", &self.db_path)
            .field("config_paths", &self.config_paths)
            .finish_non_exhaustive()
    }
}

/// Check that a graphical display is reachable before opening a window.
///
/// On Linux and the BSDs a window needs an X11 (`DISPLAY`) or Wayland
/// (`WAYLAND_DISPLAY`) server; a non-empty value of either is accepted.
/// Other platforms always have a display and return `Ok`.
///
/// # Errors
///
/// Returns [`GuiLaunchError::NoDisplay`] when neither variable is set to a
/// non-empty value on an X11/Wayland platform.
pub fn check_display() -> Result<(), GuiLaunchError> {
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        if !set("DISPLAY") && !set("WAYLAND_DISPLAY") {
            return Err(GuiLaunchError::NoDisplay);
        }
    }
    Ok(())
}

/// Run the GUI application until its window is closed.
///
/// Runs on the tokio runtime thread (`codemason-main` in `cdmn`). It checks
/// the display and builds the [`App`] here, then hands the `App`, the log
/// receiver, the event bus and a runtime [`tokio::runtime::Handle`] to the
/// process main thread through `opts.launcher`. The main thread's
/// [`MainThreadHost`] runs the eframe window there, while agent work keeps
/// running on this runtime and reaches the window through the event bridge.
/// This future resolves when the window closes.
///
/// # Arguments
///
/// * `opts` - the shared services and startup choices; see [`GuiOptions`].
///
/// # Returns
///
/// `Ok(())` once the window is closed.
///
/// # Errors
///
/// Returns [`GuiLaunchError::NoDisplay`] when no display is reachable and
/// [`GuiLaunchError::Backend`] when the main-thread host is not serving or
/// eframe fails to start or run.
pub async fn run_gui(opts: GuiOptions) -> anyhow::Result<()> {
    check_display()?;

    let GuiOptions {
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent,
        show_log,
        startup,
        log_rx,
        db_path,
        launcher,
        // Consumed by the later session-startup and log-panel tasks.
        config_paths: _,
        timings: _,
        stderr_spool: _,
    } = opts;
    tracing::info!(?startup, "starting GUI frontend");

    let app = App::new(
        Arc::clone(&event_bus),
        storage,
        provider_registry,
        session_processor,
        agent,
        show_log,
        db_path,
    );
    launcher
        .launch(GuiLaunch {
            app,
            event_bus: (*event_bus).clone(),
            logs: log_rx,
            runtime: tokio::runtime::Handle::current(),
        })
        .await?;
    Ok(())
}
