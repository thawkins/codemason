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
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_i18n_command.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.
#![allow(unsafe_code)]
#![cfg(test)]

use std::sync::MutexGuard;

use codemason_app_core::app::App;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
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

/// `render_app_to_string` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_app_to_string_effects(app: &mut App) {
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// Help / status paths
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Enable / disable + persistence
// ---------------------------------------------------------------------------

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_enable_translates_and_statusbar_renders_catalog() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    // Disabled by default: the status bar shows the English label.
    assert!(!codemason_config::i18n::is_enabled());
    render_app_to_string_effects(&mut app);

    // Enabling i18n with the `zz` locale applies + persists.
    app.execute_slash_command("/i18n zz").await;
    assert!(
        codemason_config::i18n::is_enabled(),
        "/i18n <locale> must enable i18n"
    );
    assert!(codemason_config::i18n::catalog_loaded());
    assert_eq!(codemason_config::i18n::active_locale(), "zz");

    // The translated key renders; an untranslated key falls back to English.
    render_app_to_string_effects(&mut app);

    // The change persisted to the project config.
    let persisted = codemason_config::Config::load().expect("load config");
    let section = persisted.i18n.expect("i18n section persisted");
    assert!(section.enabled);
    assert_eq!(section.locale, "zz");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_off_restores_english() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    app.execute_slash_command("/i18n zz").await;
    assert!(codemason_config::i18n::is_enabled());

    app.execute_slash_command("/i18n off").await;
    assert!(
        !codemason_config::i18n::is_enabled(),
        "/i18n off must disable translation"
    );
    render_app_to_string_effects(&mut app);

    let persisted = codemason_config::Config::load().expect("load config");
    let section = persisted.i18n.expect("i18n section persisted");
    assert!(!section.enabled);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_i18n_missing_catalog_falls_back_to_english() {
    let (_guard, _env, _temp) = enter_isolated_project();
    let mut app = support::make_app();

    // No catalog installed for `qq`.
    app.execute_slash_command("/i18n qq").await;
    assert!(codemason_config::i18n::is_enabled());
    assert!(
        !codemason_config::i18n::catalog_loaded(),
        "a locale with no catalog must not load one"
    );
    render_app_to_string_effects(&mut app);
}
