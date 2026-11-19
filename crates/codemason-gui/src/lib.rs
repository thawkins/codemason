//! Desktop GUI frontend for codemason, built on eframe/egui.
//!
//! The crate renders the frontend-neutral [`codemason_app_core::app::App`]
//! in a native window. It is the interactive frontend of `cdmn`.
//!
//! - [`run_gui`] is the entry point the interactive `cdmn` launch path calls.
//! - [`main_thread`] hands the built session to the process main thread,
//!   which runs the eframe window ([`MainThreadHost::serve`]).
//! - [`Shell`] is the `eframe::App` that draws the window.
//! - [`dock`] is the dockable pane layout model (craft-ui docking) the
//!   shell renders and keeps in step with the App's panel flags.
//! - [`keys`] is the keyboard shortcut map (craft-keymap).
//! - [`dialogs`] draws the App's modal dialogs (permission, provider
//!   setup, model picker, keyboard shortcuts) from one dispatch.
//! - [`panels`] holds the pane contents ([`panels::messages`] is the
//!   transcript, [`panels::prompt`] the prompt editor over the App's input
//!   buffer, [`panels::queue`] the input queue, [`panels::sessions`] the
//!   sessions sidebar, [`panels::log`] the log panel) and the
//!   [`panels::status_bar`], drawn with [`palette`] colours and
//!   [`styled_text`] layout.
//! - [`fonts`] builds the font stack (block and status glyphs, a bold
//!   face, system fonts for CJK and the rest) the window installs at start.
//! - [`platform`] holds the native file dialogs ([`platform::files`]) and
//!   the clipboard paste and copy wiring ([`platform::clipboard`]).
//! - [`theme`] installs the egui visuals of the App's theme and
//!   restyles the window when the mode changes.
//! - [`bridge::spawn_event_bridge`] forwards agent events from the
//!   [`EventBus`] into the shell's queue and wakes egui.
//! - [`signal`] turns Ctrl+C and SIGTERM into the window's normal quit.
//!
//! Dependencies: `eframe`/`egui` (the storytold forks, via the workspace
//! `[patch.crates-io]` table), `craft-ui` (docking, IME, combo boxes),
//! `craft-keymap` (keyboard shortcuts), `craft-dialogs` (native file
//! dialogs), `tokio`, `thiserror`
//! and `tracing`.

#![deny(unsafe_code)]

pub mod bridge;
pub mod dialogs;
pub mod dock;
pub mod external_edit;
pub mod fonts;
pub mod icons;
pub mod keys;
pub mod main_thread;
pub mod material;
pub mod metrics;
pub mod page;
pub mod palette;
pub mod panels;
pub mod platform;
pub mod ribbon;
pub mod shell;
pub mod signal;
pub mod styled_text;
pub mod theme;
pub mod title_bar;

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

pub use main_thread::{GuiLaunch, GuiLauncher, MainThreadHost, main_thread_channel};
pub use shell::Shell;

/// Why the GUI could not be started.
#[derive(Debug, thiserror::Error)]
pub enum GuiLaunchError {
    /// No graphical display is reachable (neither `DISPLAY` nor
    /// `WAYLAND_DISPLAY` is set on an X11/Wayland platform).
    #[error(
        "no graphical display found: neither DISPLAY nor WAYLAND_DISPLAY is set; \
         use `cdmn run <prompt>` for headless mode"
    )]
    NoDisplay,
    /// The windowing or rendering backend failed to start or run.
    #[error("GUI backend error: {0}")]
    Backend(String),
}

/// Everything [`run_gui`] needs, gathered in one struct.
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
/// the display, builds the [`App`] and runs its startup sequence
/// ([`App::run_startup`]: provider wiring, health check, and the first
/// session restored, created or resumed per `opts.startup`) here, then hands
/// the `App`, the log
/// receiver, an event-bus receiver (subscribed before startup, so startup
/// events are kept) and a runtime [`tokio::runtime::Handle`] to the
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
        config_paths,
        timings,
    } = opts;
    tracing::info!(?startup, "starting GUI frontend");

    // Subscribe before startup: the bus does not replay, and startup
    // publishes events (the instruction-file notice of a fresh session,
    // early watcher and scheduler events) the window must show.
    let events = event_bus.subscribe();
    let mut app = App::new(
        Arc::clone(&event_bus),
        storage,
        provider_registry,
        session_processor,
        agent,
        show_log,
        db_path,
    );
    // Runs here, on the runtime thread and inside its context, before the
    // window exists: it spawns the health check and Auto-fix watcher on the
    // runtime and does the blocking session and history reads off the UI
    // thread.
    app.run_startup(&startup, config_paths, timings).await;
    launcher
        .launch(GuiLaunch {
            app,
            events,
            logs: log_rx,
            runtime: tokio::runtime::Handle::current(),
        })
        .await?;
    Ok(())
}
