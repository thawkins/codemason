//! `Config::edit_source_config` / `set_source_value` (deep-refactor audit car
//! 1, CF-04): a toggle edits only its own key in the file the config was
//! loaded from. It must never serialise the merged config into that file
//! (which copied user-global secrets into a repository file), must write the
//! loaded project file rather than one relative to the process cwd, and must
//! refuse to overwrite a source file it cannot parse.

#![allow(unsafe_code)]

use codemason_config::Config;
use serial_test::serial;

/// Isolates `XDG_CONFIG_HOME`, the config env overrides and the cwd, and
/// restores them on drop.
struct Isolated {
    tmp: tempfile::TempDir,
    prev_cwd: std::path::PathBuf,
    prev: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl Isolated {
    fn new() -> Self {
        let keys = [
            "XDG_CONFIG_HOME",
            "CODEMASON_CONFIG",
            "CODEMASON_CONFIG_CONTENT",
        ];
        let prev = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
        let tmp = tempfile::tempdir().expect("tempdir");
        // SAFETY: `#[serial]` keeps env mutation single-threaded.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path().join(".config"));
            std::env::remove_var("CODEMASON_CONFIG");
            std::env::remove_var("CODEMASON_CONFIG_CONTENT");
        }
        Config::invalidate_load_cache();
        Self {
            tmp,
            prev_cwd: std::env::current_dir().expect("cwd"),
            prev,
        }
    }

    fn global_file(&self) -> std::path::PathBuf {
        self.tmp.path().join(".config/codemason/codemason.json")
    }

    fn write_global(&self, json: &str) {
        let path = self.global_file();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("global dir");
        std::fs::write(path, json).expect("write global");
    }

    /// Create `<tmp>/<name>` with `json` as its project config; returns the dir.
    fn project(&self, name: &str, json: &str) -> std::path::PathBuf {
        let dir = self.tmp.path().join(name);
        std::fs::create_dir_all(dir.join(".codemason")).expect("project dir");
        std::fs::write(dir.join(".codemason/codemason.json"), json).expect("write project");
        dir
    }
}

impl Drop for Isolated {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev_cwd);
        for (key, value) in self.prev.drain(..) {
            // SAFETY: `#[serial]` keeps env mutation single-threaded.
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        Config::invalidate_load_cache();
    }
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse")
}

#[test]
#[serial]
fn test_source_edit_writes_only_the_key_and_no_global_secret() {
    let env = Isolated::new();
    env.write_global(r#"{ "tavily_api_key": "USER-SECRET", "username": "me" }"#);
    let project = env.project("p", r#"{ "username": "project" }"#);
    std::env::set_current_dir(&project).expect("cwd");

    let config = Config::load().expect("load");
    config
        .set_source_value(&["code_index", "enabled"], serde_json::json!(false))
        .expect("edit");

    let written = read_json(&project.join(".codemason/codemason.json"));
    assert_eq!(
        written,
        serde_json::json!({ "username": "project", "code_index": { "enabled": false } }),
        "only the toggled key may be added to the project file"
    );
    assert!(
        !std::fs::read_to_string(project.join(".codemason/codemason.json"))
            .expect("read")
            .contains("USER-SECRET"),
        "a user-global secret must never be copied into the project file"
    );
    assert!(!Config::load().expect("reload").code_index.enabled);
}

#[test]
#[serial]
fn test_source_edit_targets_the_loaded_project_not_the_process_cwd() {
    let env = Isolated::new();
    let a = env.project("a", r#"{ "username": "a" }"#);
    let b = env.project("b", r#"{ "username": "b" }"#);
    std::env::set_current_dir(&a).expect("cwd a");

    let config = Config::load_for(&b).expect("load b");
    config
        .set_source_value(&["code_index", "enabled"], serde_json::json!(false))
        .expect("edit");

    assert_eq!(
        read_json(&a.join(".codemason/codemason.json")),
        serde_json::json!({ "username": "a" }),
        "the process-cwd project must be untouched"
    );
    assert_eq!(
        read_json(&b.join(".codemason/codemason.json")),
        serde_json::json!({ "username": "b", "code_index": { "enabled": false } })
    );
}

#[test]
#[serial]
fn test_source_edit_goes_to_global_when_no_project_file_was_loaded() {
    let env = Isolated::new();
    env.write_global(r#"{ "tavily_api_key": "USER-SECRET", "unknown_future_key": 7 }"#);
    let dir = env.tmp.path().join("plain");
    std::fs::create_dir_all(&dir).expect("dir");
    std::env::set_current_dir(&dir).expect("cwd");

    let config = Config::load().expect("load");
    config
        .set_source_value(&["security_analyzer", "enabled"], serde_json::json!(true))
        .expect("edit");

    assert_eq!(
        read_json(&env.global_file()),
        serde_json::json!({
            "tavily_api_key": "USER-SECRET",
            "unknown_future_key": 7,
            "security_analyzer": { "enabled": true }
        }),
        "every other global key, even an unmodelled one, must survive"
    );
}

#[test]
#[serial]
fn test_source_edit_refuses_to_overwrite_an_unparseable_source() {
    let env = Isolated::new();
    let project = env.project("p", r#"{ "username": "project" }"#);
    std::env::set_current_dir(&project).expect("cwd");
    let config = Config::load().expect("load");

    let broken = "{ \"username\": ";
    std::fs::write(project.join(".codemason/codemason.json"), broken).expect("corrupt");
    let err = config
        .set_source_value(&["code_index", "enabled"], serde_json::json!(true))
        .expect_err("a corrupt source must not be overwritten");
    assert!(err.to_string().contains("parse"), "got: {err}");
    assert_eq!(
        std::fs::read_to_string(project.join(".codemason/codemason.json")).expect("read"),
        broken
    );
}

/// A project config that is a symlink to the user-global file must not be
/// edited: the atomic rename would replace the link with a regular project
/// file holding the global secrets.
#[cfg(unix)]
#[test]
#[serial]
fn test_source_edit_refuses_a_project_file_symlinked_to_the_global_config() {
    let env = Isolated::new();
    env.write_global(r#"{ "tavily_api_key": "USER-SECRET" }"#);
    let project = env.tmp.path().join("linked");
    std::fs::create_dir_all(project.join(".codemason")).expect("dir");
    let link = project.join(".codemason/codemason.json");
    std::os::unix::fs::symlink(env.global_file(), &link).expect("symlink");
    std::env::set_current_dir(&project).expect("cwd");

    let config = Config::load().expect("load");
    let err = config
        .set_source_value(&["code_index", "enabled"], serde_json::json!(false))
        .expect_err("a symlinked project config must not be written");
    assert!(err.to_string().contains("symbolic link"), "got: {err}");
    assert!(
        std::fs::symlink_metadata(&link)
            .expect("link")
            .file_type()
            .is_symlink(),
        "the link must be left in place"
    );
    assert_eq!(
        read_json(&env.global_file()),
        serde_json::json!({ "tavily_api_key": "USER-SECRET" })
    );
}

/// `"section": null` loads as an absent optional section, so a toggle must
/// treat it like a missing one instead of failing with "not an object".
#[test]
#[serial]
fn test_source_edit_replaces_a_null_section() {
    let env = Isolated::new();
    let project = env.project("p", r#"{ "security_analyzer": null }"#);
    std::env::set_current_dir(&project).expect("cwd");

    Config::load()
        .expect("load")
        .set_source_value(&["security_analyzer", "enabled"], serde_json::json!(true))
        .expect("a null section is replaced");
    assert_eq!(
        read_json(&project.join(".codemason/codemason.json")),
        serde_json::json!({ "security_analyzer": { "enabled": true } })
    );
}

/// The project path is recorded absolute at load time, so changing the cwd
/// between the load and the edit still writes the loaded project.
#[test]
#[serial]
fn test_source_edit_after_a_cwd_change_targets_the_loaded_project() {
    let env = Isolated::new();
    let a = env.project("a", r#"{ "username": "a" }"#);
    let b = env.project("b", r#"{ "username": "b" }"#);
    std::env::set_current_dir(&a).expect("cwd a");
    let config = Config::load().expect("load a");
    std::env::set_current_dir(&b).expect("cwd b");

    config
        .set_source_value(&["code_index", "enabled"], serde_json::json!(false))
        .expect("edit");
    assert_eq!(
        read_json(&a.join(".codemason/codemason.json")),
        serde_json::json!({ "username": "a", "code_index": { "enabled": false } })
    );
    assert_eq!(
        read_json(&b.join(".codemason/codemason.json")),
        serde_json::json!({ "username": "b" }),
        "the new cwd's project must be untouched"
    );
}

/// `i18n` merges as a whole section, so `/i18n on` must keep an inherited
/// locale rather than write `enabled` alone and reset `locale` to "en".
#[test]
#[serial]
fn test_i18n_toggle_keeps_the_inherited_locale() {
    let env = Isolated::new();
    env.write_global(r#"{ "i18n": { "enabled": false, "locale": "fr" } }"#);
    let project = env.project("p", r#"{ "username": "project" }"#);
    std::env::set_current_dir(&project).expect("cwd");

    codemason_config::i18n::persist(Some(true), None).expect("persist");
    let reloaded = Config::load().expect("reload");
    let i18n = reloaded.i18n.expect("i18n section");
    assert!(i18n.enabled);
    assert_eq!(i18n.locale, "fr");
}
