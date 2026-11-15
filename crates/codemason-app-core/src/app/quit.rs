//! Quitting the app: the one quit path both frontends use.
//!
//! `/quit`, `/exit` and Ctrl+C then Ctrl+D call [`App::quit`], which stops the
//! app (`is_running = false`). The frontend then leaves its loop or closes its
//! window and runs the exit steps in order:
//!
//! 1. [`App::save_on_quit`]: drop the never-persisted input queue, save the
//!    input history if it changed, and cancel a running agent turn;
//! 2. [`App::stop_services_on_quit`]: abort the swarm hook jobs (decided in
//!    task 4.1: aborted, not awaited), stop every MCP stdio server child and
//!    every code index.
//!
//! The terminal frontend restores the terminal between the two steps; the
//! GUI runs them back to back. [`App::turn_running`] tells the GUI whether
//! closing the window must ask first.
//!
//! Depends on `codemason_agent`'s `SessionProcessor` for the MCP shutdown.

use codemason_agent::session::processor::SessionProcessor;

use super::App;

impl App {
    /// Stop the app, as `/quit` and `/exit` do: the frontend leaves its loop
    /// (the TUI) or closes its window (the GUI) and then runs the exit steps
    /// ([`App::save_on_quit`], [`App::stop_services_on_quit`]).
    pub fn quit(&mut self) {
        self.is_running = false;
    }

    /// Whether an agent turn is running in the focused session or in any
    /// background session, so quitting now would cancel it.
    ///
    /// # Returns
    ///
    /// `true` while any open session is processing a message.
    #[must_use]
    pub fn turn_running(&self) -> bool {
        std::iter::once(&self.session)
            .chain(self.background.iter())
            .any(|view| view.is_processing)
    }

    /// The first exit step: empty the input queue (never persisted, spec
    /// `inputqueue` NFR-005), save the input history if it changed since the
    /// last flush (a debounced flush may still be pending), and set the
    /// focused turn's cancel flag so its LLM stream stops.
    pub fn save_on_quit(&mut self) {
        self.clear_input_queue();
        if self.history_dirty
            && let Err(e) = self.save_history()
        {
            tracing::warn!("Failed to save input history: {}", e);
        }
        if let Some(ref flag) = self.session.cancel_flag {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The second exit step: abort every session's swarm auto-complete job
    /// (its running hook is killed; the tasks it had not reached stay open),
    /// kill every MCP stdio server child codemason spawned, cancel any
    /// in-flight code-index reindex, then stop the code index watcher and
    /// the parked projects' indexes.
    ///
    /// # Arguments
    ///
    /// * `session_processor` - the processor whose MCP servers are stopped
    ///   (the TUI passes the one it was started with).
    pub async fn stop_services_on_quit(&mut self, session_processor: &SessionProcessor) {
        // Dropping a job aborts it (`Drop for SwarmAutoComplete`); doing it
        // here rather than when the App drops also covers a platform whose
        // process ends without dropping the App (macOS `terminate:`).
        drop(self.session.swarm_autocomplete.take());
        for index in 0..self.background.len() {
            if let Some(view) = self.background.view_mut(index) {
                drop(view.swarm_autocomplete.take());
            }
        }
        // Without this the MCP child outlives codemason as an orphan, keeps
        // the stdio pipe open and prints its own teardown errors after
        // codemason exits; `McpClient::drop` repeats the kill if this is cut
        // short.
        session_processor.shutdown_mcp().await;
        // Cancel an in-flight reindex BEFORE stopping the watcher, so a
        // detached CPU-bound reindex thread stops between chunks instead of
        // running for the whole teardown (exit 200%-CPU fix).
        if let Some(ref index) = self.code_index {
            index.request_stop();
        }
        // The watch session's Drop stops the watcher.
        drop(self.code_index_watch_session.take());
        // ...and the parked indexes of other open projects (MULTISESSION M8).
        self.stop_project_services();
    }
}
