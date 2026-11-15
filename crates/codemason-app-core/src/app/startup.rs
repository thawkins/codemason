//! Startup session selection and the frontend-neutral startup sequence.
//!
//! [`StartupSession`] says which session a launch focuses first.
//! [`App::run_startup`] performs the startup work a frontend runs once before
//! its first frame: it attaches the session storage and event bus to the
//! providers that need them, starts the provider health check, adopts the
//! startup MCP connection state, opens the first session (restore, new or
//! resume), starts the Auto-fix watcher, loads the input history, records the
//! open-session list and freezes the startup timings.
//!
//! Depends on `codemason-agent` (session manager, startup timings),
//! `codemason-llm` (router and OpenRouter providers) and `codemason-config`
//! (the input-history path).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use codemason_agent::StartupTimings;
use codemason_llm::providers::openrouter::OpenRouterProvider;
use codemason_llm::providers::router::RouterProvider;

/// Which session a launch focuses first (spec MULTISESSION FR-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupSession {
    /// Reopen the sessions that were open at the last exit; start a fresh
    /// session when none can be reopened. The default for `codemason`.
    Restore,
    /// Always start a fresh session (`codemason --new`).
    New,
    /// Resume this stored session (`codemason session resume <id>`).
    Resume(String),
}

impl super::App {
    /// Attach the event bus to every provider and the session storage to the
    /// router and OpenRouter providers.
    ///
    /// The router resolves database-backed API keys (`codemason auth
    /// ollama_cloud <key>`) for the providers it routes to, and OpenRouter
    /// resolves the credential stored by `codemason auth openrouter <key>`;
    /// neither can do so until the storage is attached.
    pub fn attach_provider_storage(&self) {
        self.provider_registry
            .set_event_bus_all(Some(Arc::clone(&self.event_bus)));
        if let Some(router) = self
            .provider_registry
            .get_as_any("router")
            .and_then(|p| p.downcast_ref::<RouterProvider>())
        {
            router.set_storage(Arc::clone(&self.storage));
        }
        if let Some(openrouter) = self
            .provider_registry
            .get_as_any("openrouter")
            .and_then(|p| p.downcast_ref::<OpenRouterProvider>())
        {
            openrouter.set_storage(Arc::clone(&self.storage));
        }
    }

    /// Focus the first session of a launch.
    ///
    /// - [`StartupSession::Resume`] loads the stored session; a load failure
    ///   is logged and leaves no session focused.
    /// - [`StartupSession::Restore`] reopens the saved open sessions
    ///   ([`Self::restore_open_sessions`]).
    /// - [`StartupSession::New`], or a restore that reopened nothing, creates
    ///   a fresh session in the process working directory, maps it to the
    ///   agent name for log display and prints its startup block.
    ///
    /// The resulting open-session list is saved so the next launch can
    /// restore it.
    ///
    /// # Arguments
    ///
    /// * `startup` - which session to focus.
    pub fn open_startup_session(&mut self, startup: &StartupSession) {
        let fresh = match startup {
            StartupSession::Resume(id) => {
                if let Err(e) = self.load_session(id) {
                    tracing::error!(error = %e, session_id = %id, "Failed to resume session");
                }
                false
            }
            StartupSession::Restore => self.restore_open_sessions() == 0,
            StartupSession::New => true,
        };
        if fresh {
            let dir = std::env::current_dir().unwrap_or_default();
            match self.session_processor.session_manager.create_session(dir) {
                Ok(session) => {
                    self.session.session_id = Some(session.id);
                    self.register_primary_session_mapping();
                    // The session startup block (banner, id, config files)
                    // and the instruction-file announcement.
                    self.announce_session_startup();
                    self.status = "session created".to_string();
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to auto-create session at startup");
                }
            }
        }
        self.persist_open_sessions();
    }

    /// Run the startup sequence once, before the frontend's first frame.
    ///
    /// Must be called inside a tokio runtime: the provider health check, the
    /// MCP adoption and the Auto-fix watcher spawn tasks on it. The session
    /// and history reads are blocking, so a frontend calls this from its
    /// runtime thread rather than its UI thread.
    ///
    /// # Arguments
    ///
    /// * `startup` - which session to focus first.
    /// * `config_paths` - config files loaded at launch, shown in a new
    ///   session's startup block.
    /// * `timings` - the launch phase timings recorded so far; the startup
    ///   stages are added and the total is frozen for `/startup`.
    pub async fn run_startup(
        &mut self,
        startup: &StartupSession,
        config_paths: Vec<PathBuf>,
        mut timings: StartupTimings,
    ) {
        if let Some(ref mut app_stages) = self.startup_timings {
            timings.merge_stages(app_stages);
        }
        self.attach_provider_storage();
        self.config_paths = config_paths;

        let t0 = Instant::now();
        self.check_provider_health();
        timings.record("Provider health check", t0.elapsed());

        // The MCP connect loop in `src/main.rs` may have published its
        // per-server events before anything subscribed; the shared client is
        // the authoritative state, so adopt it now (housekeeping re-runs this
        // until the client appears).
        let processor = Arc::clone(&self.session_processor);
        self.adopt_mcp_client_state(&processor).await;

        let t0 = Instant::now();
        self.open_startup_session(startup);
        self.start_autofix_watcher();
        timings.record("Session create", t0.elapsed());

        let t0 = Instant::now();
        let history_path =
            codemason_config::user_dirs::global_history_path().unwrap_or_else(|| {
                PathBuf::from(".")
                    .join("codemason")
                    .join("input_history.txt")
            });
        self.set_history_file(history_path);
        if let Err(e) = self.load_history() {
            tracing::warn!("Failed to load input history: {}", e);
        }
        timings.record("Input history load", t0.elapsed());

        // Backfill the context-window cache for models selected before the
        // cache existed.
        self.backfill_model_ctx_window();

        timings.finish(); // Freeze the wall-clock total so /startup stays stable.
        self.startup_timings = Some(timings);
        self.status = "ready".to_string();
    }
}
