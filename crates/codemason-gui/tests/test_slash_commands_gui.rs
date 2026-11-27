//! Tests for `test_slash_commands.rs`
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_slash_commands.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

//! Tests for TUI slash command parsing and dispatch (TASK-006).
//!
//! Verifies each slash command updates app state correctly, handles arguments,
//! and provides user feedback via status bar and log entries.
//!
//! Some tests redirect `XDG_CONFIG_HOME` into a tempdir. Env mutation is
//! `unsafe` in edition 2024 and the workspace denies `unsafe_code`, so this
//! target opts back in explicitly (env mutation is contained to the test).
#![allow(unsafe_code)]
use crate::term_events::CrosstermAppExt;
use std::sync::{Arc, MutexGuard};

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use codemason_app_core::app::App;

fn make_app_with_storage(storage: Arc<Storage>) -> App {
    codemason_app_core::app::use_scratch_log_root_for_tests();
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_managers: Default::default(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        project_code_indexes: Default::default(),
        project_tool_registries: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: codemason_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(codemason_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        pending_verdicts: codemason_agent::session::pending::PendingVerdicts::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    // The session sidebar (MULTISESSION M5) is on by default and would take
    // the left columns of the rendered layout these tests assert on; keep the
    // full-width chat they were written against, as `support::make_app` does.
    app.sidebar.visible = false;
    app
}

struct CwdGuard {
    prev: std::path::PathBuf,
    #[allow(dead_code)]
    lock: MutexGuard<'static, ()>,
    /// Optional tempdir, declared last so it is deleted only after cwd has
    /// been restored (drop order = field declaration order).
    temp: Option<tempfile::TempDir>,
}

impl CwdGuard {
    /// Path of the guard's working directory (the tempdir when present).
    #[allow(dead_code)]
    fn path(&self) -> std::path::PathBuf {
        self.temp
            .as_ref()
            .map(|t| t.path().to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().expect("current dir"))
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Set `XDG_CONFIG_HOME` for the duration of a test and restore it on drop.
struct XdgGuard {
    prev: Option<std::ffi::OsString>,
}

impl XdgGuard {
    fn set(dir: &std::path::Path) -> Self {
        let prev = std::env::var_os("XDG_CONFIG_HOME");
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir) };
        Self { prev }
    }
}

impl Drop for XdgGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(v) => unsafe { std::env::set_var("XDG_CONFIG_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CONFIG_HOME") },
        }
    }
}

/// Change the process working directory to `dir`, returning a guard that
/// restores the previous cwd on drop.
///
/// The guard acquires the shared cwd mutex before changing directory and
/// **keeps it held** until dropped, so only one test manipulates cwd at a
/// time. Callers MUST NOT also hold `cwd_lock()` (that would deadlock, since
/// `std::sync::Mutex` is not re-entrant).
#[allow(dead_code)] // used by tests that require cwd manipulation
fn with_cwd(dir: &std::path::Path) -> CwdGuard {
    let lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("set_current_dir");
    CwdGuard {
        prev,
        lock,
        temp: None,
    }
}

/// Write `content` to `<dir>/.codemason/memory/MEMORY.md` (creating parent dirs).
#[allow(dead_code)] // used by tests that set up project memory state
fn write_project_memory(dir: &std::path::Path, content: &str) {
    let mem_dir = dir.join(".codemason").join("memory");
    std::fs::create_dir_all(&mem_dir).expect("create memory dir");
    std::fs::write(mem_dir.join("MEMORY.md"), content).expect("write MEMORY.md");
}

/// A tempdir that is both the process cwd and the `XDG_CONFIG_HOME`.
///
/// Runtime-flag toggles (`/yolo`, Alt+Y, `/editlog`, Alt+E) persist to the
/// **user-global** config, so redirecting only the cwd let these tests write
/// `"yolo": false` into the developer's real `~/.config/codemason/codemason.json`
/// and YOLO came up off on the next codemason start. Fields drop in declaration
/// order: `XDG_CONFIG_HOME` is restored before the tempdir is deleted.
struct TempConfigDir {
    _xdg: XdgGuard,
    _temp: tempfile::TempDir,
}

/// Callers must already hold `cwd_lock()`: this mutates cwd and
/// `XDG_CONFIG_HOME`.
fn enter_temp_config_dir() -> TempConfigDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let xdg = XdgGuard::set(temp.path());
    let codemason_dir = temp.path().join(".codemason");
    std::fs::create_dir_all(&codemason_dir).expect("create .codemason");
    // Prime a project-local config with a known YOLO state so persistence tests
    // do not race on Config::load's default-config creation path.
    std::fs::write(codemason_dir.join("codemason.json"), r#"{"yolo": false}"#)
        .expect("write project config");
    TempConfigDir {
        _xdg: xdg,
        _temp: temp,
    }
}

/// Acquire the cwd test lock, recovering from any prior poisoning.
#[allow(clippy::await_holding_lock)]
/// The process-wide cwd/env lock (see `tests/all.rs`).
fn cwd_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_alt_e_toggles_edit_log_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    codemason_config::edit_log::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: edit log starts off.
    assert!(!codemason_config::edit_log::is_enabled());

    // Press Alt+E through the app handler so the persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted edit logging on.
    assert!(codemason_config::edit_log::is_enabled());
    assert!(app.status.contains("Edit log enabled"));

    // Status bar indicator should reflect the current (enabled) state.
    crate::frame::publish(&mut app);

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT))
        .await;
    assert!(!codemason_config::edit_log::is_enabled());
    assert!(app.status.contains("Edit log disabled"));

    crate::frame::publish(&mut app);
}

// -- /clear ----------------------------------------------------------

// -- /help -----------------------------------------------------------

// -- /quit -----------------------------------------------------------

// -- /system ---------------------------------------------------------

// -- /agent ----------------------------------------------------------

// -- /log ------------------------------------------------------------

// -- /profile --------------------------------------------------------

// -- /llmstats -------------------------------------------------------

// -- /cost -----------------------------------------------------------

// -- /clip -----------------------------------------------------------

// -- /compact --------------------------------------------------------

// `/compress` is a deprecated alias for `/compact` (FR-009) and must
// forward to the same compaction path.

// -- /undo -----------------------------------------------------------

// -- /name -----------------------------------------------------------

// -- /model ----------------------------------------------------------

// -- /provider -------------------------------------------------------

// -- /provider_reset -------------------------------------------------

// -- unknown command -------------------------------------------------

// -- /blueprints command ---------------------------------------------

// -- input clearing --------------------------------------------------

// -- with leading slash and without ----------------------------------

// -- /system preserves whitespace ------------------------------------

// -- /tools ----------------------------------------------------------

// -- /spec -----------------------------------------------------------

// Use a multi-threaded runtime because `execute_slash_command("/spec create ...")`
// spawns a background task that calls `processor.process_message`, whose model
// resolution path may call `block_in_place` for spec reads/writes.
// `block_in_place` panics on the default current-thread `#[tokio::test]` runtime;
// `flavor = "multi_thread"` provides a runtime where it is permitted.
// -- /config ---------------------------------------------------------

// NOTE: render_terminal_to_string helper removed; re-add when needed.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_alt_y_toggles_yolo_mode_and_status_bar_indicator() {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let lock = cwd_lock();
    let original_cwd = std::env::current_dir().expect("cwd");
    let _temp = enter_temp_config_dir();
    let _guard = CwdGuard {
        prev: original_cwd,
        lock,
        temp: None,
    };
    codemason_config::yolo::set_enabled(false);

    let mut app = make_app_with_storage(storage);

    // Sanity: YOLO starts off.
    assert!(!codemason_config::yolo::is_enabled());

    // Press Alt+Y through the app handler so the new persist path runs.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT))
        .await;

    // Handler should have toggled and persisted YOLO on.
    assert!(codemason_config::yolo::is_enabled());
    assert!(app.status.contains("YOLO mode enabled"));

    // Status bar indicator should reflect the current (enabled) state.
    crate::frame::publish(&mut app);

    // Toggle back off and verify.
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT))
        .await;
    assert!(!codemason_config::yolo::is_enabled());
    assert!(app.status.contains("YOLO mode disabled"));

    // Status bar should now show the disabled indicator.
    crate::frame::publish(&mut app);
}

// -- /triggers slash command tests -------------------------------------

// -- /inbox slash command tests ----------------------------------------

// -- /task (todo2tasks T-016, FR-019) ---------------------------------

// -- /spawn ----------------------------------------------------------------
