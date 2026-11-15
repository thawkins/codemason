//! Background services started at launch (`App::start_background_services`,
//! `App::run_startup`), as the removed terminal UI's `run_tui` started them:
//!
//! - the spec manager is installed on the App and the session processor;
//! - the cron scheduler is running;
//! - the automation scheduler runs only when the config enables automation;
//! - the background code-index startup delivers its result to the App;
//! - startup removes stale pasted-image temp files from `target/temp`.
//!
//! Each test runs in its own scratch directory under `target/temp`, made the
//! process cwd and the user config/home dirs while it holds the binary-wide
//! env lock (taken by `scratch_env`): every one of these services resolves its paths from the cwd.

// Rust 2024 makes `std::env::set_var`/`remove_var` unsafe; this is the one
// sanctioned test use (AGENTS-LANGUAGE.md, Core Code Rules).
#![allow(unsafe_code)]

#[path = "support/mod.rs"]
mod support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use codemason_app_core::app::{App, StartupSession};

/// Restores the cwd and the environment a test changed, even if it panics.
struct ScratchEnv {
    cwd: PathBuf,
    vars: Vec<(&'static str, Option<OsString>)>,
    dir: PathBuf,
    /// Held for the guard's lifetime; declared last so it is released only
    /// after the cwd and variables are restored.
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for ScratchEnv {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.cwd); // INTENTIONAL: best-effort restore
        for (name, value) in &self.vars {
            match value {
                // SAFETY: `self._lock` serialises every env mutation.
                Some(v) => unsafe { std::env::set_var(name, v) },
                // SAFETY: `self._lock` serialises every env mutation.
                None => unsafe { std::env::remove_var(name) },
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir); // INTENTIONAL: best-effort scratch cleanup
    }
}

/// Take the binary-wide env lock and make a fresh scratch dir the cwd,
/// `HOME` and `XDG_CONFIG_HOME`; the returned guard holds the lock, so a test
/// must not take it again.
fn scratch_env(tag: &str) -> ScratchEnv {
    let lock = crate::global_env_lock();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/temp")
        .join(format!("startup-services-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: start from an empty dir
    std::fs::create_dir_all(dir.join("project")).expect("scratch project");
    let names = ["HOME", "XDG_CONFIG_HOME"];
    let guard = ScratchEnv {
        cwd: std::env::current_dir().expect("cwd"),
        vars: names.iter().map(|n| (*n, std::env::var_os(n))).collect(),
        dir: dir.clone(),
        _lock: lock,
    };
    std::env::set_current_dir(dir.join("project")).expect("enter scratch project");
    // SAFETY: `guard` holds the env lock.
    unsafe {
        std::env::set_var("HOME", dir.join("home"));
        std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
    }
    guard
}

/// Make the processor's cached config enable or disable automation.
fn set_automation(app: &App, enabled: bool) {
    let mut config = codemason_config::Config::default();
    config.automation = Some(codemason_config::automation::AutomationConfig {
        enabled,
        ..Default::default()
    });
    *app.session_processor.cached_config.lock() =
        Some(codemason_agent::session::processor::CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        });
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_startup_services_install_the_spec_manager() {
    let _env = scratch_env("spec");
    let mut app = support::make_app();

    app.start_background_services();

    let root = std::env::current_dir().expect("cwd").join("specs");
    let on_app = app.spec_manager.as_ref().expect("spec manager on the App");
    assert_eq!(on_app.root(), root.as_path());
    let on_processor = app
        .session_processor
        .spec_manager
        .get()
        .expect("spec manager on the processor");
    assert_eq!(on_processor.root(), root.as_path());
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_startup_services_start_the_cron_scheduler() {
    let _env = scratch_env("cron");
    let mut app = support::make_app();

    app.start_background_services();

    assert!(app.cron_scheduler.is_some(), "the cron scheduler runs");
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_startup_services_skip_automation_when_disabled() {
    let _env = scratch_env("auto-off");
    let mut app = support::make_app();
    set_automation(&app, false);

    app.start_background_services();

    assert!(
        app.automation_scheduler.is_none(),
        "a project that did not opt in runs no automation scheduler"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_startup_services_start_automation_when_enabled() {
    let _env = scratch_env("auto-on");
    let mut app = support::make_app();
    set_automation(&app, true);

    app.start_background_services();

    assert!(
        app.automation_scheduler.is_some(),
        "the automation scheduler runs"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_startup_services_deliver_the_code_index_result() {
    let _env = scratch_env("codeindex");
    let mut app = support::make_app();
    // As at launch, the first session exists before the services start, so
    // the outcome has a transcript to land in.
    app.open_startup_session(&StartupSession::New);

    app.start_background_services();
    assert!(
        app.code_index_startup_pending(),
        "the code index is starting"
    );
    app.await_code_index_startup().await;

    assert!(!app.code_index_startup_pending(), "the result was applied");
    assert!(
        app.code_index.is_some(),
        "the default config enables the index"
    );
    let transcript: String = app
        .session
        .messages
        .iter()
        .map(codemason_agent::message::Message::text_content)
        .collect();
    assert!(
        transcript.contains("Code index: enabled"),
        "the outcome is reported: {transcript}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the lock pins the cwd for the whole test
async fn test_run_startup_removes_stale_pasted_image_files() {
    let _env = scratch_env("clipboard");
    let temp = std::env::current_dir().expect("cwd").join("target/temp");
    std::fs::create_dir_all(&temp).expect("temp dir");
    let stale = temp.join("codemason_paste_stale.png");
    let fresh = temp.join("codemason_paste_fresh.png");
    std::fs::write(&stale, b"old").expect("stale file");
    std::fs::write(&fresh, b"new").expect("fresh file");
    let two_days_ago = SystemTime::now() - Duration::from_hours(48);
    filetime::set_file_mtime(&stale, filetime::FileTime::from_system_time(two_days_ago))
        .expect("age the stale file");
    let mut app = support::make_app();

    app.run_startup(
        &StartupSession::New,
        Vec::new(),
        codemason_agent::StartupTimings::new(),
    )
    .await;

    assert!(
        !stale.exists(),
        "a pasted image older than a day is removed"
    );
    assert!(fresh.exists(), "a recent pasted image is kept");
}
