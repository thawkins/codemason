//! The eframe application that draws the codemason window.
//!
//! The shell draws the message transcript ([`MessagesView`]) above the
//! input queue ([`queue_ui`]) and the prompt editor ([`prompt_ui`]), which
//! edits the App's own input buffer, and over them the App's active modal
//! dialog ([`dialogs::show`]); later tasks render the other panes from the
//! `App` state.
//! [`Shell::logic`], which eframe also runs while the window is hidden,
//! drains the agent-event queue and the tracing log channel into the `App`
//! and runs app-core's background housekeeping pass (`App::housekeeping`)
//! and the App's timers (`App::poll_timers`). Each shown frame then
//! prepares the prompt's input
//! ([`PromptInput::prepare`]) and the active dialog's ([`dialogs::prepare`]),
//! then routes the frame's key presses to `App::handle_key` through
//! [`KeyRouter`] before any widget reads them. After drawing it runs the key
//! presses the dialogs' buttons sent ([`dialogs::take_keys`]).

use codemason_agent::event::Event;
use codemason_app_core::app::{App, LogLevel, SelectionPane};
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
            let transcript = self.messages.show(ui, &self.app, &palette);
            let right_click =
                ui.input(|i| i.pointer.button_clicked(egui::PointerButton::Secondary));
            if right_click && transcript.hovered() && !dialogs::locks_prompt(&self.app) {
                // The transcript has no text selection in the GUI, so its
                // menu has nothing enabled, as the TUI's without one.
                dialogs::context_menu::open(
                    ui.ctx(),
                    &mut self.app,
                    SelectionPane::Messages,
                    false,
                );
            }
        });
        dialogs::show(ui.ctx(), &mut self.app);
    }

    /// Run one whole frame: [`Self::logic`], then route the keys and draw.
    ///
    /// eframe calls the two halves separately (`eframe::App::logic` and
    /// `eframe::App::ui`); harnesses that drive only a `Ui` call this.
    ///
    /// # Arguments
    ///
    /// * `ui` - the root ui of the window.
    pub fn update(&mut self, ui: &mut egui::Ui) {
        self.logic(ui.ctx());
        self.draw_frame(ui);
    }

    /// The work that must go on whether or not the window is shown: drain
    /// the agent events and log records into the App, run app-core's
    /// housekeeping pass and the App's timers, and keep a wake scheduled for
    /// the next pass.
    ///
    /// eframe calls `eframe::App::logic` before every frame and, while the
    /// window is minimised or hidden (when it runs no frame at all), on
    /// every requested repaint, at most every 100 ms for an invisible
    /// window. The pass always requests the next wake (at most
    /// `HOUSEKEEPING_INTERVAL` away), so swarm unblocking and completion,
    /// autopilot, timed-question defaults and the history flush keep going
    /// while the window is hidden, as in the TUI. It reads no input and
    /// draws nothing.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the window's context.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.drain_sources();
        self.housekeeping(ctx);
        self.poll_timers(ctx);
        // `/quit` (or any other path that stops the app) closes the window.
        if !self.app.is_running {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Route this frame's keys and draw it (the shown half of a frame).
    fn draw_frame(&mut self, ui: &mut egui::Ui) {
        // A context-menu item chosen last frame acts on the prompt editor
        // before the prompt reads this frame's input.
        dialogs::context_menu::apply_pending(ui.ctx());
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

    /// The application state, for tests that change it between frames
    /// (e.g. to move app-core's test clock).
    ///
    /// # Returns
    ///
    /// The App the shell renders.
    #[doc(hidden)]
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Run app-core's background housekeeping pass (`App::housekeeping`):
    /// the polls that apply what background jobs deposited (compaction,
    /// rollback, `/swarm`, `/bench`, sub-agent launches, background
    /// sessions, store fetches, ...), gated by `HOUSEKEEPING_INTERVAL`, then
    /// the history flush. It runs on every [`Self::logic`] call, as the
    /// terminal frontend runs it every loop wake, and asks for a wake when
    /// the next pass is due: background jobs fill shared slots without
    /// waking anything, so that wake (the TUI's loop keeps the same one) is
    /// what lands their results while the user is idle or the window is
    /// hidden.
    ///
    /// It runs on the UI thread through `Handle::block_on`, like
    /// `App::handle_event`, the same pass the TUI runs on its render loop.
    /// Most awaits are short (lock `try_read`s that skip when held, small
    /// task-store writes), but not all: when a finished `/swarm`
    /// decomposition is applied or a swarm finalises, each task-store write
    /// takes the store's blocking file lock and runs the team's
    /// `TaskCreated` / `TaskCompleted` hook inline, each hook with a 30 s
    /// timeout, one task after another. With such hooks configured the
    /// window does not repaint or take input until they finish, exactly as
    /// the TUI stalls. Moving that work off the thread is a recorded
    /// follow-up. Without attached sources there is no runtime, so nothing
    /// runs.
    fn housekeeping(&mut self, ctx: &egui::Context) {
        let Some(sources) = self.sources.as_ref() else {
            return;
        };
        let processor = std::sync::Arc::clone(&self.app.session_processor);
        sources.runtime.block_on(self.app.housekeeping(&processor));
        ctx.request_repaint_after(
            self.app
                .housekeeping_due_at()
                .saturating_duration_since(std::time::Instant::now()),
        );
    }

    /// Run the App's wall-clock timers (`App::poll_timers`: status expiry,
    /// run-cost banner, timed question) every frame, whatever is drawn, and
    /// ask for a frame when the next one is due, so they fire while the
    /// window is idle too. A timed question behind another modal still
    /// answers its default, as in the TUI.
    fn poll_timers(&mut self, ctx: &egui::Context) {
        self.app.poll_timers();
        if let Some(due) = self.app.next_timer_due() {
            ctx.request_repaint_after(due);
        }
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
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The inherent `Shell::logic` (inherent methods take precedence).
        Self::logic(self, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe has just called `logic` for this frame.
        self.draw_frame(ui);
    }
}
