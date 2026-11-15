//! `/codeindex on|off` persistence safety (deep-refactor audit car 1, AG-02
//! and CF-04).
//!
//! - A config that fails to load must be reported, never replaced: the toggle
//!   once loaded with `unwrap_or_default()` and saved that empty default over
//!   the user's global config.
//! - A successful toggle writes only `code_index.enabled` into the loaded
//!   project file; it must not copy user-global secrets into it.

#[path = "support/mod.rs"]
mod support;

/// RAII cwd guard so a temp working directory is restored on drop.
struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

const GLOBAL: &str = r#"{ "tavily_api_key": "USER-SECRET", "username": "me" }"#;

/// Enter a temp project whose `.codemason/codemason.json` holds `project`, with
/// [`GLOBAL`] as the user-global config.
fn enter(project: &str) -> (CwdGuard, support::GlobalConfigGuard, tempfile::TempDir) {
    let temp = tempfile::tempdir().expect("tempdir");
    let global = support::GlobalConfigGuard::install(temp.path(), GLOBAL);
    let original = std::env::current_dir().expect("cwd");
    std::fs::create_dir_all(temp.path().join(".codemason")).expect(".codemason");
    std::fs::write(temp.path().join(".codemason/codemason.json"), project).expect("project");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    (CwdGuard(original), global, temp)
}

fn global_text(temp: &tempfile::TempDir) -> String {
    std::fs::read_to_string(temp.path().join(".config/codemason/codemason.json"))
        .expect("read global")
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_codeindex_toggle_never_overwrites_global_when_project_config_is_broken() {
    let _lock = crate::global_env_lock();
    let (_cwd, _global, temp) = enter("{ \"bad\": ");
    let mut app = support::make_app();
    app.cwd_path = temp.path().to_path_buf();

    for command in ["/codeindex off", "/codeindex on"] {
        app.execute_slash_command(command).await;
        assert!(
            app.status.ends_with("(unsaved)"),
            "{command}: a failed load must be reported, got status {:?}",
            app.status
        );
        assert_eq!(
            global_text(&temp),
            GLOBAL,
            "{command}: the user-global config must be left byte-identical"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_codeindex_toggle_writes_only_its_key_into_the_project_file() {
    let _lock = crate::global_env_lock();
    let (_cwd, _global, temp) = enter(r#"{ "username": "project" }"#);
    let mut app = support::make_app();
    app.cwd_path = temp.path().to_path_buf();

    app.execute_slash_command("/codeindex off").await;
    assert_eq!(app.status, "codeindex: off");

    let project: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(temp.path().join(".codemason/codemason.json")).expect("read"),
    )
    .expect("parse project");
    assert_eq!(
        project,
        serde_json::json!({ "username": "project", "code_index": { "enabled": false } })
    );
    assert_eq!(global_text(&temp), GLOBAL, "the global config is untouched");
}
