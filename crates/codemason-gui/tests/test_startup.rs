//! `run_gui` honours the launch's startup choice: the App handed to the
//! window has the resumed session focused for `cdmn session resume <id>`,
//! and a fresh session for `cdmn --new`. The window itself needs a display,
//! so a stand-in host receives the launch.

// Rust 2024 makes `std::env::set_var`/`remove_var` unsafe; this is the one
// sanctioned test use (AGENTS-LANGUAGE.md, Core Code Rules).
#![allow(unsafe_code)]

#[path = "support/mod.rs"]
mod support;

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::StartupTimings;
use codemason_app_core::app::StartupSession;
use codemason_gui::{GuiOptions, main_thread_channel, run_gui};

/// Restores the environment variables a test changed, even if it panics,
/// then releases the env lock it holds (the lock field is declared last).
struct EnvRestore {
    vars: Vec<(&'static str, Option<OsString>)>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, value) in &self.vars {
            match value {
                // SAFETY: `self._lock` serialises every env mutation.
                Some(v) => unsafe { std::env::set_var(name, v) },
                // SAFETY: `self._lock` serialises every env mutation.
                None => unsafe { std::env::remove_var(name) },
            }
        }
    }
}

/// Take the binary-wide env lock, point the display check at a dummy display
/// and the user config dirs at a scratch tree, and return the guard that
/// restores them and holds the lock (a test must not take it again).
fn scratch_env(tag: &str) -> EnvRestore {
    let lock = crate::global_env_lock();
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp")
        .join(format!("gui-startup-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");
    let names = ["DISPLAY", "XDG_CONFIG_HOME", "HOME"];
    let restore = EnvRestore {
        vars: names.iter().map(|n| (*n, std::env::var_os(n))).collect(),
        _lock: lock,
    };
    // SAFETY: `restore` holds the env lock.
    unsafe {
        std::env::set_var("DISPLAY", ":99");
        std::env::set_var("XDG_CONFIG_HOME", scratch.join("config"));
        std::env::set_var("HOME", scratch.join("home"));
    }
    restore
}

/// Run `run_gui` with `startup` against a stand-in host, returning the
/// focused session id of the App the host received and the id of a session
/// that was stored before launch.
async fn launched_session(
    startup: impl FnOnce(&str) -> StartupSession,
) -> (Option<String>, String) {
    let seed = support::make_app();
    let stored = seed
        .session_processor
        .session_manager
        .create_session(std::env::current_dir().expect("cwd"))
        .expect("create session")
        .id;
    let agent = codemason_agent::agent::resolve_agent("general", &Default::default())
        .expect("resolve general agent");

    let (launcher, host) = main_thread_channel();
    let host_thread = std::thread::spawn(move || {
        let mut focused = None;
        host.serve_with(|launch| {
            focused = Some(launch.app.session.session_id.clone());
            Ok(())
        });
        focused.flatten()
    });

    let (_log_tx, log_rx) = codemason_app_core::log_channel::tui_log_channel(4);
    let result = Box::pin(run_gui(GuiOptions {
        event_bus: Arc::clone(&seed.event_bus),
        storage: Arc::clone(&seed.storage),
        provider_registry: Arc::clone(&seed.provider_registry),
        session_processor: Arc::clone(&seed.session_processor),
        agent: Arc::unwrap_or_clone(agent),
        show_log: false,
        startup: startup(&stored),
        log_rx,
        db_path: PathBuf::new(),
        config_paths: Vec::new(),
        timings: StartupTimings::new(),
        launcher,
    }))
    .await;
    assert!(result.is_ok(), "run_gui: {result:?}");
    (host_thread.join().expect("host thread"), stored)
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins DISPLAY/HOME for the whole launch
async fn test_run_gui_resume_focuses_the_requested_session() {
    let _env = scratch_env("resume");

    let (focused, stored) = Box::pin(launched_session(|id| {
        StartupSession::Resume(id.to_string())
    }))
    .await;

    assert_eq!(focused.as_deref(), Some(stored.as_str()));
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins DISPLAY/HOME for the whole launch
async fn test_run_gui_new_starts_a_fresh_session() {
    let _env = scratch_env("new");

    let (focused, stored) = Box::pin(launched_session(|_| StartupSession::New)).await;

    let focused = focused.expect("a fresh session is focused");
    assert_ne!(focused, stored, "--new must not reuse a stored session");
}
