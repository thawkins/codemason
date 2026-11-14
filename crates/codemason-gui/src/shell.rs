//! The eframe application that draws the codemason window.
//!
//! For now the shell draws a central placeholder and the prompt input; later
//! tasks render the transcript, panels and dialogs from the `App` state.
//! Each frame it first drains the agent-event queue and the tracing log
//! channel into the `App`.

use codemason_agent::event::Event;
use codemason_app_core::app::{App, LogLevel};
use codemason_app_core::log_channel::TuiLogReceiver;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;

/// Most log records moved into the app per frame, so a burst cannot stall
/// a frame (the terminal frontend uses the same limit).
const LOG_DRAIN_LIMIT: usize = 50;

/// Accessible label of the prompt input.
pub const PROMPT_LABEL: &str = "Prompt";

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
    prompt: String,
    sources: Option<Sources>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("prompt", &self.prompt)
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
    /// A shell with an empty prompt.
    #[must_use]
    pub fn new(app: App) -> Self {
        Self {
            app,
            prompt: String::new(),
            sources: None,
        }
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
            ui.horizontal(|ui| {
                let label = ui.label(PROMPT_LABEL);
                ui.add(
                    egui::TextEdit::singleline(&mut self.prompt)
                        .desired_width(f32::INFINITY)
                        .hint_text("Ask codemason..."),
                )
                .labelled_by(label.id);
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label(format!("codemason - {}", self.app.cwd));
            });
        });
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
        self.drain_sources();
        // `/quit` (or any other path that stops the app) closes the window,
        // which ends `run_gui` the same way closing the window does.
        if !self.app.is_running {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.show(ui);
    }
}
