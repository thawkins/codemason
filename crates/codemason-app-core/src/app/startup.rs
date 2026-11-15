//! Startup session selection and the frontend-neutral startup sequence.
//!
//! [`StartupSession`] says which session a launch focuses first.
//! [`App::run_startup`] performs the startup work a frontend runs once before
//! its first frame: it attaches the session storage and event bus to the
//! providers that need them, starts the provider health check, adopts the
//! startup MCP connection state, opens the first session (restore, new or
//! resume), starts the Auto-fix watcher, loads the input history, starts the
//! background services (code index, spec manager, cron and automation
//! schedulers), records the open-session list and freezes the startup
//! timings. It also sweeps stale pasted-image temp files once.
//!
//! Depends on `codemason-agent` (session manager, startup timings),
//! `codemason-llm` (router and OpenRouter providers) and `codemason-config`
//! (the input-history path), `codemason-codeindex` and `codemason-specs`.

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
        self.open_first_session(startup);
        self.resume_startup_session(startup);
        self.persist_open_sessions();
    }

    /// Restore the saved open sessions, or create a fresh session, as
    /// `startup` asks; a resume is left to [`Self::resume_startup_session`].
    /// The Auto-fix watcher starts between the restore and the create, as in
    /// the terminal UI.
    fn open_first_session(&mut self, startup: &StartupSession) {
        let restored =
            matches!(startup, StartupSession::Restore) && self.restore_open_sessions() > 0;
        self.start_autofix_watcher();
        if matches!(startup, StartupSession::Resume(_)) || restored {
            return;
        }
        let dir = std::env::current_dir().unwrap_or_default();
        match self.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                self.session.session_id = Some(session.id);
                self.register_primary_session_mapping();
                // The session startup block (banner, id, config files) and
                // the instruction-file announcement.
                self.announce_session_startup();
                self.status = "session created".to_string();
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to auto-create session at startup");
            }
        }
    }

    /// Load the stored session a [`StartupSession::Resume`] names; a failure
    /// is logged and leaves no session focused.
    fn resume_startup_session(&mut self, startup: &StartupSession) {
        if let StartupSession::Resume(id) = startup
            && let Err(e) = self.load_session(id)
        {
            tracing::error!(error = %e, session_id = %id, "Failed to resume session");
        }
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
        // One-time, best-effort sweep of orphaned pasted-image temp files;
        // individual errors are logged.
        let _ = crate::clipboard::prune_clipboard_temp_files(
            // INTENTIONAL: best-effort cache prune
            crate::clipboard::CLIPBOARD_TEMP_MAX_AGE,
        );
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
        self.open_first_session(startup);
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

        let t0 = Instant::now();
        self.adopt_mcp_client_state(&processor).await;
        timings.record("MCP server status", t0.elapsed());

        let t0 = Instant::now();
        self.start_background_services();
        timings.record("Code index spawn & spec/cron setup", t0.elapsed());

        let t0 = Instant::now();
        self.resume_startup_session(startup);
        // Record the startup session set so the next launch can restore it.
        self.persist_open_sessions();
        // Backfill the context-window cache for models selected before the
        // cache existed.
        self.backfill_model_ctx_window();
        timings.record("Spec mgr & resume & backfill", t0.elapsed());

        timings.finish(); // Freeze the wall-clock total so /startup stays stable.
        self.startup_timings = Some(timings);
        self.status = "ready".to_string();
    }

    /// Start the background services a launch runs for its lifetime, in
    /// the order the terminal UI started them:
    ///
    /// 1. the code index (when `code_index.enabled` in the config): opened
    ///    and watched on a blocking thread, its result delivered to the App by
    ///    [`Self::poll_code_index_startup`] from housekeeping;
    /// 2. the spec manager over `<cwd>/specs`, on the App and the processor;
    /// 3. the cron scheduler (FR-010, FR-017);
    /// 4. the automation scheduler, only when the config enables automation
    ///    (spec `openhands` FR-014).
    ///
    /// Must be called inside a tokio runtime.
    pub fn start_background_services(&mut self) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let processor = Arc::clone(&self.session_processor);
        let cwd = std::env::current_dir().unwrap_or_default();
        let index_root = cwd.clone();
        tokio::task::spawn_blocking(move || {
            let result = open_code_index(&processor, index_root);
            let _ = tx.send(result); // INTENTIONAL: the App may be gone at shutdown
        });
        self.code_index_startup_rx = Some(rx);

        let specs_root = cwd.join("specs");
        let _ = self // INTENTIONAL: OnceLock already set by an earlier launch path
            .session_processor
            .spec_manager
            .set(Arc::new(codemason_specs::SpecManager::new(&specs_root)));
        self.spec_manager = Some(Arc::new(codemason_specs::SpecManager::new(&specs_root)));

        self.cron_scheduler = Some(super::cron::start_cron_scheduler(
            Arc::clone(&self.storage),
            Arc::clone(&self.session_processor),
            cwd.clone(),
        ));

        let automation_config = self
            .session_processor
            .load_config_cached()
            .automation_config();
        self.automation_scheduler = if automation_config.is_enabled() {
            let service = Arc::new(codemason_agent::automation::AutomationService::new(
                Arc::clone(&self.session_processor),
                automation_config,
                cwd,
            ));
            Some(service.spawn_scheduler())
        } else {
            None
        };
    }

    /// Whether the background code-index startup result is still awaited.
    #[doc(hidden)]
    pub fn code_index_startup_pending(&self) -> bool {
        self.code_index_startup_rx.is_some()
    }

    /// Apply the background code-index startup result if it has arrived:
    /// install the index and its watcher, keep the fallback thread, report
    /// the outcome in the transcript and record its duration in the startup
    /// timings. Called from housekeeping; does nothing once applied.
    pub fn poll_code_index_startup(&mut self) {
        let Some(rx) = self.code_index_startup_rx.as_mut() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.code_index_startup_rx = None;
                self.apply_code_index_startup(result);
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                self.code_index_startup_rx = None;
            }
        }
    }

    /// Wait for the background code-index startup result and apply it.
    #[doc(hidden)]
    pub async fn await_code_index_startup(&mut self) {
        let Some(mut rx) = self.code_index_startup_rx.take() else {
            return;
        };
        if let Some(result) = rx.recv().await {
            self.apply_code_index_startup(result);
        }
    }

    /// Wire a code-index startup result into the App.
    fn apply_code_index_startup(&mut self, result: CodeIndexStartupResult) {
        if let Some(index) = result.index {
            self.set_code_index(Some(index));
        }
        if let Some(session) = result.watch_session {
            self.code_index_watch_session = Some(session);
        }
        self.code_index_fallback_thread = result.fallback_thread;
        if !result.message.is_empty() {
            self.append_assistant_text(&result.message);
        }
        if let Some(ref mut timings) = self.startup_timings {
            timings.record("Code index (background)", result.elapsed);
        }
        self.needs_redraw = true;
    }
}

/// Result of the background code-index startup, delivered to the App over a
/// channel so the `&mut App` updates happen on the frontend's thread.
pub(crate) struct CodeIndexStartupResult {
    /// The opened code index, if any.
    index: Option<Arc<codemason_codeindex::CodeIndex>>,
    /// The watch session, if the watcher started.
    watch_session: Option<codemason_codeindex::WatchSession>,
    /// Fallback one-shot reindex thread, when the watcher failed to start.
    fallback_thread: Option<std::thread::JoinHandle<()>>,
    /// Text to append to the transcript.
    message: String,
    /// Wall-clock duration of the background startup.
    elapsed: std::time::Duration,
}

impl std::fmt::Debug for CodeIndexStartupResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeIndexStartupResult")
            .field("has_index", &self.index.is_some())
            .field("watching", &self.watch_session.is_some())
            .field("fallback_reindex", &self.fallback_thread.is_some())
            .field("message", &self.message)
            .field("elapsed", &self.elapsed)
            .finish_non_exhaustive()
    }
}

/// Open and watch the code index for `cwd` when the config enables it
/// (blocking; runs on a blocking thread).
fn open_code_index(
    processor: &codemason_agent::session::processor::SessionProcessor,
    cwd: PathBuf,
) -> CodeIndexStartupResult {
    let start = Instant::now();
    let done = |index, watch_session, fallback_thread, message: &str| CodeIndexStartupResult {
        index,
        watch_session,
        fallback_thread,
        message: message.to_string(),
        elapsed: start.elapsed(),
    };
    let config = match codemason_agent::Config::load() {
        Ok(config) => config,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load config for code index check");
            return done(None, None, None, "");
        }
    };
    if !config.code_index.enabled {
        tracing::debug!("Code index is disabled in config");
        return done(None, None, None, "\n[ok] Code index: disabled\n");
    }
    let index_config = codemason_codeindex::types::CodeIndexConfig {
        enabled: true,
        project_root: cwd.clone(),
        index_dir: cwd.join(".codemason/codeindex"),
        scan_config: codemason_codeindex::types::ScanConfig::default(),
    };
    let index = match codemason_codeindex::CodeIndex::open(&index_config) {
        Ok(index) => Arc::new(index),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to initialize code index");
            return done(None, None, None, "\n[err] Code index: failed to open\n");
        }
    };
    // start_watching() runs an initial full reindex in the background and
    // then watches the tree for changes.
    let (watch_session, fallback_thread) = match codemason_codeindex::start_watching(
        Arc::clone(&index),
        codemason_codeindex::worker::WorkerConfig::default(),
    ) {
        Ok(session) => {
            tracing::info!("Code index watcher started");
            (Some(session), None)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to start code index watcher, falling back to one-shot reindex");
            let bg = Arc::clone(&index);
            let handle = std::thread::spawn(move || {
                if let Err(e) = bg.full_reindex() {
                    tracing::warn!(error = %e, "Background code index reindex failed");
                }
            });
            (None, Some(handle))
        }
    };
    let _ = processor.code_index.set(Arc::clone(&index)); // INTENTIONAL: OnceLock set race is benign
    tracing::info!("Code index initialized at {:?}", index_config.index_dir);
    done(
        Some(index),
        watch_session,
        fallback_thread,
        "\n[ok] Code index: enabled\n",
    )
}
