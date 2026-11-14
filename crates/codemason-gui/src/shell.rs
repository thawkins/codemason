//! The eframe application that draws the codemason window.
//!
//! The shell draws the message transcript ([`MessagesView`]) above the
//! input queue ([`queue_ui`]) and the prompt editor ([`prompt_ui`]), which
//! edits the App's own input buffer, and over them the App's active modal
//! dialog ([`dialogs::show`]); later tasks render the other panes from the
//! `App` state.
//! Each frame ([`Shell::update`]) it first drains the agent-event queue and
//! the tracing log channel into the `App`, prepares the prompt's input
//! ([`PromptInput::prepare`]) and the active dialog's ([`dialogs::prepare`]),
//! then routes the frame's key presses to `App::handle_key` through
//! [`KeyRouter`] before any widget reads them. After drawing it runs the key
//! presses the dialogs' buttons sent ([`dialogs::take_keys`]).

use codemason_agent::event::Event;
use codemason_app_core::app::{App, LogLevel};
use codemason_app_core::log_channel::TuiLogReceiver;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::dialogs;
use crate::keys::{Focus, KeyRouter, default_keymap};
use crate::palette::palette_for;
use crate::panels::messages::MessagesView;
use crate::panels::prompt::{PromptInput, prompt_ui};
use crate::panels::queue::queue_ui;

/// Most log records moved into the app per frame, so a burst cannot stall
/// a frame (the terminal frontend uses the same limit).
const LOG_DRAIN_LIMIT: usize = 50;

pub use crate::panels::prompt::PROMPT_LABEL;

/// Event and log sources a running window drains each frame.
struct Sources {
    events: UnboundedReceiver<Event>,
    logs: TuiLogReceiver,
    runtime: Handle,
}

/// The codemason window: an [`eframe::App`] over the frontend-neutral
/// [`App`].
pub struct Shell {
    app: App,
    prompt: PromptInput,
    sources: Option<Sources>,
    keys: KeyRouter,
    messages: MessagesView,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("input", &self.app.session.input)
            .field("has_sources", &self.sources.is_some())
            .finish_non_exhaustive()
    }
}

impl Shell {
    /// Create a shell over `app` with no event or log sources attached.
    ///
    /// # Arguments
    ///
    /// * `app` - the application state to render.
    ///
    /// # Returns
    ///
    /// A shell over the App's prompt and the default keymap of the platform
    /// this build runs on.
    #[must_use]
    pub fn new(mut app: App) -> Self {
        // The GUI draws the keyboard shortcuts panel
        // (`dialogs::shortcuts`), so `?` on an empty prompt opens it, as in
        // the TUI.
        app.shortcuts_panel_enabled = true;
        Self {
            app,
            prompt: PromptInput::default(),
            sources: None,
            keys: KeyRouter::new(default_keymap(craft_keymap::Platform::current())),
            messages: MessagesView::default(),
        }
    }

    /// Replace the keyboard shortcuts.
    ///
    /// # Arguments
    ///
    /// * `keymap` - the bindings to use, e.g. from [`crate::keys::load_keymap`].
    ///
    /// # Returns
    ///
    /// The shell routing keys through `keymap`.
    #[must_use]
    pub fn with_keymap(mut self, keymap: craft_keymap::Keymap) -> Self {
        self.keys = KeyRouter::new(keymap);
        self
    }

    /// Attach the queues the shell drains each frame.
    ///
    /// # Arguments
    ///
    /// * `events` - agent events from [`crate::bridge::spawn_event_bridge`].
    /// * `logs` - tracing records for the log panel.
    /// * `runtime` - the tokio runtime that drives `App::handle_event`.
    ///
    /// # Returns
    ///
    /// The shell with its sources attached.
    #[must_use]
    pub fn with_sources(
        mut self,
        events: UnboundedReceiver<Event>,
        logs: TuiLogReceiver,
        runtime: Handle,
    ) -> Self {
        self.sources = Some(Sources {
            events,
            logs,
            runtime,
        });
        self
    }

    /// Draw the window contents into `ui`.
    ///
    /// # Arguments
    ///
    /// * `ui` - the root ui of the window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("prompt_panel").show(ui, |ui| {
            queue_ui(ui, &self.app);
            prompt_ui(ui, &mut self.app);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let palette = palette_for(self.app.theme_mode);
            self.messages.show(ui, &self.app, &palette);
        });
        dialogs::show(ui.ctx(), &mut self.app);
    }

    /// Run one frame: drain the sources, route the keys, draw.
    ///
    /// # Arguments
    ///
    /// * `ui` - the root ui of the window.
    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.drain_sources();
        let focus = self.prompt.prepare(ui.ctx(), &mut self.app);
        dialogs::prepare(ui.ctx(), &mut self.app);
        self.route_keys(ui.ctx(), focus);
        // `/quit` (or any other path that stops the app) closes the window,
        // which ends `run_gui` the same way closing the window does.
        if !self.app.is_running {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.show(ui);
        self.run_dialog_keys(ui.ctx());
    }

    /// The application state the shell renders.
    ///
    /// # Returns
    ///
    /// The App, including the input buffer the prompt edits.
    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Route this frame's key presses to `App::handle_key`.
    ///
    /// Runs before any widget reads input, so a press that runs a keymap
    /// command is gone before the prompt sees it. While the prompt has the
    /// keyboard (`focus`), text it types stays with it (see
    /// [`KeyRouter::route`]). Without attached sources there is no runtime
    /// to drive the async handler, so the presses are left to the widgets.
    fn route_keys(&mut self, ctx: &egui::Context, focus: Focus) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        let keys = ctx.input_mut(|input| self.keys.route(input, focus));
        for key in keys {
            sources.runtime.block_on(self.app.handle_key(key));
        }
    }

    /// Run the key presses the dialogs' buttons sent this frame through
    /// `App::handle_key`, as typed keys, and repaint to show the result.
    /// Without attached sources there is no runtime, so they are dropped.
    fn run_dialog_keys(&mut self, ctx: &egui::Context) {
        let keys = dialogs::take_keys(ctx);
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        if keys.is_empty() {
            return;
        }
        for key in keys {
            sources.runtime.block_on(self.app.handle_key(key));
        }
        ctx.request_repaint();
    }

    /// Move pending agent events and log records into the [`App`].
    fn drain_sources(&mut self) {
        let Some(sources) = self.sources.as_mut() else {
            return;
        };
        while let Ok(event) = sources.events.try_recv() {
            sources.runtime.block_on(self.app.handle_event(event));
        }
        for record in sources.logs.try_iter().take(LOG_DRAIN_LIMIT) {
            if record.message.is_empty() {
                continue;
            }
            let level = match record.level {
                tracing::Level::ERROR => LogLevel::Error,
                tracing::Level::WARN => LogLevel::Warn,
                _ => LogLevel::Info,
            };
            // Redact before display, as the terminal frontend does.
            let message = codemason_types::sanitize::redact_secrets(&record.message);
            self.app.push_log(level, message, None);
        }
    }
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update(ui);
    }
}
