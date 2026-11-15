//! `/i18n` TUI dispatch + status-bar translation tests (spec `openhands`
//! T-014; FR-027).
//!
//! Verifies the `handle_i18n_command` dispatch arm through the public
//! `App::execute_slash_command` entry point, and that the status bar renders a
//! translated label from an installed locale catalog while falling back to
//! English for untranslated keys.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so the
//! toggle never touches the developer's real global config. Env mutation is
//! `unsafe` in edition 2024; the workspace denies `unsafe_code`, so this test
//! target opts back in explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::MutexGuard;

use codemason_app_core::app::App;

#[path = "support/mod.rs"]
mod support;

/// The i18n runtime state and the cwd/env mutations below are process-global;
/// one mutex serialises every test in this binary against all of them.
#[allow(clippy::await_holding_lock)]
/// The process-wide cwd/env lock (see `tests/all.rs`).
fn test_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Restores cwd, config env vars, and the i18n runtime on drop.
struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            keys: vec![
                ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
                ("CODEMASON_CONFIG", std::env::var("CODEMASON_CONFIG").ok()),
                (
                    "CODEMASON_CONFIG_CONTENT",
                    std::env::var("CODEMASON_CONFIG_CONTENT").ok(),
                ),
                ("LANG", std::env::var("LANG").ok()),
                ("LC_ALL", std::env::var("LC_ALL").ok()),
                ("LC_MESSAGES", std::env::var("LC_MESSAGES").ok()),
            ],
            // A prior test can momentarily leave the cwd pointing at a
            // just-deleted tempdir; fall back to `.` so setup never panics.
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.keys {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        // NOTE: the i18n runtime is reset at the *start* of
        // `enter_isolated_project` (under `test_lock`), not here - a drop-time
        // reset would run after the lock is released and race the next test.
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter a fresh tempdir project with an isolated global config dir, a scratch
/// project config, and an installed `zz` locale catalog.
fn enter_isolated_project() -> (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
) {
    let guard = test_lock();
    let env_guard = EnvGuard::new();

    let temp = tempfile::tempdir().expect("tempdir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("CODEMASON_CONFIG") };
    unsafe { std::env::remove_var("CODEMASON_CONFIG_CONTENT") };
    // Ensure no ambient locale auto-enables i18n and skews the assertions.
    unsafe { std::env::remove_var("LC_ALL") };
    unsafe { std::env::remove_var("LC_MESSAGES") };
    unsafe { std::env::set_var("LANG", "C") };
    std::env::set_current_dir(temp.path()).expect("set cwd");

    let codemason_dir = temp.path().join(".codemason");
    std::fs::create_dir_all(&codemason_dir).expect("create .codemason");
    std::fs::write(
        codemason_dir.join("codemason.json"),
        r#"{"defaultAgent": "general"}"#,
    )
    .expect("write scratch project config");

    // Partial catalog: `status.project` translated, everything else falls back.
    let locales = codemason_dir.join("locales");
    std::fs::create_dir_all(&locales).expect("create locales dir");
    std::fs::write(
        locales.join("zz.json"),
        r#"{"locale":"zz","messages":{"status.project":"PROJET: "}}"#,
    )
    .expect("write zz catalog");

    codemason_config::i18n::reset();
    (guard, env_guard, temp)
}

/// Return the text of the most recently appended message.
fn last_message_text(app: &App) -> String {
    app.session
        .messages
        .last()
        .map(codemason_types::message::Message::text_content)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Help / status paths
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_help_renders_usage() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n help").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n help") && text.contains("/i18n on"),
        "help must render the usage table, got: {text}"
    );
    assert_eq!(app.status, "i18n: help", "/i18n help status");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_help_aliases() {
    let _guard = test_lock();
    let _env = EnvGuard::new();
    for input in ["/i18n --help", "/i18n -h"] {
        let mut app = support::make_app();
        app.execute_slash_command(input).await;
        let text = last_message_text(&app);
        assert!(
            text.contains("From: /i18n help"),
            "help form '{input}' must render usage, got: {text}"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_status_reports_state() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n status").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n status") && text.contains("UI internationalisation"),
        "status must report the state, got: {text}"
    );
    assert_eq!(app.status, "i18n: status");
}

// ---------------------------------------------------------------------------
// Enable / disable + persistence
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_list_reports_installed_catalogs() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();
    app.execute_slash_command("/i18n list").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /i18n list") && text.contains("zz"),
        "list must name the installed `zz` catalog, got: {text}"
    );
}
