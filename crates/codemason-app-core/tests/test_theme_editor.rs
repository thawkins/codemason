//! Theme editor tests: opening, editing with a live preview, cancelling,
//! saving to the right file, save failures and resetting a built-in theme.

use codemason_app_core::app::App;
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::theme_tokens::{
    Rgb, ThemeCatalog, ThemeSelection, save_theme, set_user_themes_dir_override,
};

mod support;

/// Points the user themes directory at a scratch directory under
/// `target/temp` while alive, holding the binary-wide env lock.
struct ScratchThemes {
    dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl ScratchThemes {
    fn scrub_themes() -> Self {
        let lock = crate::global_env_lock();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
        std::fs::create_dir_all(&root).expect("scratch root");
        let dir = tempfile::Builder::new()
            .prefix("theme-editor-")
            .tempdir_in(&root)
            .expect("scratch dir");
        set_user_themes_dir_override(Some(dir.path().join("themes")));
        Self { dir, _lock: lock }
    }

    fn themes(&self) -> std::path::PathBuf {
        self.dir.path().join("themes")
    }
}

impl Drop for ScratchThemes {
    fn drop(&mut self) {
        set_user_themes_dir_override(None);
    }
}

/// An App on the built-in catalog, showing `nord`, whose cwd is the scratch
/// directory so no project theme directory is read.
fn make_app(scratch: &ScratchThemes) -> App {
    let mut app = support::make_app();
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::Named("nord".to_string());
    app.cwd_path = scratch.dir.path().to_path_buf();
    app
}

const RED: Rgb = Rgb(200, 10, 10);

#[test]
fn test_theme_editor_open_ignores_system_and_unknown() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("system");
    assert!(app.theme_editor.is_none());
    app.open_theme_editor("no-such-theme");
    assert!(app.theme_editor.is_none());
    app.open_theme_editor("nord");
    let editor = app.theme_editor.as_ref().expect("editor open");
    assert_eq!(editor.name, "nord");
    assert!(editor.builtin);
    assert_eq!(editor.draft, *editor.original);
}

#[test]
fn test_theme_editor_set_colour_updates_draft_and_preview() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("codemason-light");
    app.set_theme_colour("accent", RED);
    let editor = app.theme_editor.as_ref().expect("editor open");
    assert_eq!(editor.draft.accent, RED);
    // The window previews the theme being edited, not the selected one.
    let shown = app.active_theme_tokens(true);
    assert_eq!(shown.name, "codemason-light");
    assert_eq!(shown.accent, RED);
    // The catalog is untouched until the edit is saved.
    assert_ne!(
        app.themes.get("codemason-light").expect("theme").accent,
        RED
    );
}

#[test]
fn test_theme_editor_contrast_warnings_follow_the_draft() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("codemason-light");
    assert_eq!(
        app.theme_editor.as_ref().expect("open").warnings,
        Vec::<String>::new()
    );
    let background = app.theme_editor.as_ref().expect("open").draft.bg.base;
    app.set_theme_colour("text.base", background);
    let warnings = &app.theme_editor.as_ref().expect("open").warnings;
    assert!(
        warnings.iter().any(|w| w.contains("text.base")),
        "{warnings:?}"
    );
}

#[test]
fn test_theme_editor_invalid_key_leaves_draft() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("nord");
    let before = app.theme_editor.as_ref().expect("open").draft.clone();
    app.set_theme_colour("text.nope", RED);
    assert_eq!(app.theme_editor.as_ref().expect("open").draft, before);
}

#[test]
fn test_theme_editor_cancel_restores() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("nord");
    app.set_theme_colour("accent", RED);
    app.cancel_theme_edit();
    assert!(app.theme_editor.is_none());
    assert_ne!(app.themes.get("nord").expect("nord").accent, RED);
    assert!(!scratch.themes().join("nord.json").exists());
}

#[test]
fn test_theme_editor_save_writes_and_reloads() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("nord");
    app.set_theme_colour("accent", RED);
    app.save_theme_edit();
    assert!(app.theme_editor.is_none());
    assert_eq!(app.status, "theme saved: nord");
    let file = scratch.themes().join("nord.json");
    assert!(file.is_file(), "the themes directory is created");
    assert_eq!(app.themes.get("nord").expect("nord").accent, RED);
    assert_eq!(app.themes.source("nord"), Some(file.as_path()));

    let mut fresh = ThemeCatalog::builtin();
    fresh.load_dir(&scratch.themes());
    assert_eq!(fresh.get("nord").expect("nord").accent, RED);
}

#[test]
fn test_theme_editor_save_goes_back_to_the_source_file() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    let project = scratch.dir.path().join("project-themes");
    let mut theme = (*app.themes.get("nord").expect("nord")).clone();
    theme.name = "mine".to_string();
    let source = project.join("my-file.json");
    save_theme(&theme, &source).expect("save");
    app.themes.load_dir(&project);

    app.open_theme_editor("mine");
    assert!(!app.theme_editor.as_ref().expect("open").builtin);
    app.set_theme_colour("accent", RED);
    app.save_theme_edit();

    let mut reread = ThemeCatalog::builtin();
    reread.load_dir(&project);
    assert_eq!(reread.get("mine").expect("mine").accent, RED);
    assert!(!scratch.themes().join("mine.json").exists());
}

#[test]
fn test_theme_editor_save_failure_keeps_editor() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    // The themes directory path is taken by a file, so the save must fail.
    std::fs::write(scratch.themes(), "not a directory").expect("write");
    app.open_theme_editor("nord");
    app.set_theme_colour("accent", RED);
    app.save_theme_edit();
    let editor = app.theme_editor.as_ref().expect("editor stays open");
    assert_eq!(editor.draft.accent, RED);
    assert!(
        app.status.starts_with("theme not saved: "),
        "{}",
        app.status
    );
    assert_ne!(app.themes.get("nord").expect("nord").accent, RED);
}

#[test]
fn test_theme_editor_reset_restores_builtin_and_deletes_file() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("nord");
    app.set_theme_colour("accent", RED);
    app.save_theme_edit();
    let file = scratch.themes().join("nord.json");
    assert!(file.is_file());

    app.open_theme_editor("nord");
    app.reset_theme_to_default();
    assert!(app.theme_editor.is_none());
    assert!(!file.exists());
    assert_eq!(app.status, "theme reset: nord");
    assert_eq!(
        *app.themes.get("nord").expect("nord"),
        *ThemeCatalog::builtin().get("nord").expect("nord")
    );
    assert!(app.themes.source("nord").is_none());
}

#[test]
fn test_theme_editor_reset_ignores_user_theme() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    let mut theme = (*app.themes.get("nord").expect("nord")).clone();
    theme.name = "mine".to_string();
    let source = scratch.themes().join("mine.json");
    save_theme(&theme, &source).expect("save");
    app.themes.load_dir(&scratch.themes());

    app.open_theme_editor("mine");
    app.reset_theme_to_default();
    assert!(source.is_file(), "a user theme has no default to reset to");
    assert!(app.theme_editor.is_some());
}

#[tokio::test]
async fn test_theme_editor_esc_cancels() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_picker();
    app.open_theme_editor("nord");
    let _ = app
        .handle_key(KeyInput {
            key: Key::Esc,
            mods: Modifiers::default(),
        })
        .await;
    assert!(app.theme_editor.is_none());
    assert!(app.theme_picker.is_some(), "Esc closes only the editor");
}

#[test]
fn test_theme_editor_close_returns_preview_to_selection() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("codemason-light");
    assert_eq!(app.active_theme_tokens(true).name, "codemason-light");
    app.cancel_theme_edit();
    assert_eq!(app.active_theme_tokens(true).name, "nord");
}

#[test]
fn test_theme_editor_reset_never_deletes_a_project_theme_file() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    let project = scratch.dir.path().join(".codemason").join("themes");
    let mut theme = (*app.themes.get("nord").expect("nord")).clone();
    theme.accent = RED;
    let source = project.join("nord.json");
    save_theme(&theme, &source).expect("save");
    app.themes.load_dir(&project);

    app.open_theme_editor("nord");
    app.reset_theme_to_default();
    assert!(source.is_file(), "a project theme file is never deleted");
    assert!(app.theme_editor.is_some());
    assert!(
        app.status.starts_with("theme not reset: "),
        "{}",
        app.status
    );
}

#[test]
fn test_theme_editor_reset_reports_a_remaining_override() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    // Two user files define `nord`; resetting deletes the one in use and
    // the other still replaces the built-in theme.
    let mut theme = (*app.themes.get("nord").expect("nord")).clone();
    theme.accent = RED;
    save_theme(&theme, &scratch.themes().join("a-nord.json")).expect("save");
    save_theme(&theme, &scratch.themes().join("b-nord.json")).expect("save");
    app.themes.load_dir(&scratch.themes());

    app.open_theme_editor("nord");
    app.reset_theme_to_default();
    assert!(!scratch.themes().join("b-nord.json").exists());
    assert!(
        app.status.contains("still replaced by") && app.status.contains("a-nord.json"),
        "{}",
        app.status
    );
}

#[tokio::test]
async fn test_theme_editor_esc_with_unsaved_edits_asks_first() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    app.open_theme_editor("nord");
    app.set_theme_colour("accent", RED);
    let esc = KeyInput {
        key: Key::Esc,
        mods: Modifiers::default(),
    };
    let _ = app.handle_key(esc).await;
    let editor = app.theme_editor.as_ref().expect("first Esc keeps the edit");
    assert_eq!(editor.draft.accent, RED);
    assert!(app.status.contains("Esc again"), "{}", app.status);
    let _ = app.handle_key(esc).await;
    assert!(app.theme_editor.is_none(), "second Esc discards");
}

#[test]
fn test_theme_editor_locks_the_input_without_the_picker() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    assert!(app.theme_picker.is_none());
    app.open_theme_editor("nord");
    assert!(app.is_input_locked(), "the editor is a modal on its own");
    app.cancel_theme_edit();
    assert!(!app.is_input_locked());
}

#[cfg(unix)]
#[test]
fn test_theme_editor_reset_keeps_a_project_symlink_into_the_user_dir() {
    let scratch = ScratchThemes::scrub_themes();
    let mut app = make_app(&scratch);
    let mut theme = (*app.themes.get("nord").expect("nord")).clone();
    theme.accent = RED;
    let user_copy = scratch.themes().join("nord.json");
    save_theme(&theme, &user_copy).expect("save");
    let project = scratch.dir.path().join(".codemason").join("themes");
    std::fs::create_dir_all(&project).expect("project dir");
    let link = project.join("nord.json");
    std::os::unix::fs::symlink(&user_copy, &link).expect("symlink");
    app.themes.load_dir(&project);
    assert_eq!(app.themes.source("nord"), Some(link.as_path()));

    app.open_theme_editor("nord");
    app.reset_theme_to_default();
    assert!(
        link.symlink_metadata().is_ok(),
        "the project's symlink is not deleted"
    );
    assert!(user_copy.is_file());
    assert!(app.theme_editor.is_some());
    assert!(
        app.status.starts_with("theme not reset: "),
        "{}",
        app.status
    );
}
