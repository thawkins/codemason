//! Theme editor dialog tests: the theme picker's edit buttons, the editor
//! overlay's Save, Cancel and Reset to default buttons, and its hex fields.

mod support;

use codemason_app_core::app::App;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::theme_tokens::{
    Rgb, ThemeCatalog, ThemeSelection, save_theme, set_user_themes_dir_override,
};
use codemason_gui::Shell;
use codemason_gui::dialogs::pickers::edit_theme_label;
use codemason_gui::dialogs::theme_editor::{
    CANCEL_EDIT, RESET_THEME, SAVE_THEME, editor_title, hex_label,
};
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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
            .prefix("theme-editor-gui-")
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

/// A shell whose App has the built-in catalog, the `system` theme and the
/// theme picker open, after `configure` ran on it.
fn rig(
    scratch: &ScratchThemes,
    configure: impl FnOnce(&mut App),
) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.themes = ThemeCatalog::builtin();
        app.theme = ThemeSelection::System;
        app.cwd_path = scratch.dir.path().to_path_buf();
        configure(&mut app);
        app.open_theme_picker();
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

fn dialog_shown(harness: &Harness<'static, Shell>, title: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Dialog, title)
        .is_some()
}

#[test]
fn test_theme_picker_shows_edit_button_per_theme() {
    let scratch = ScratchThemes::scrub_themes();
    let (_rt, mut harness) = rig(&scratch, |_| {});
    harness.run();
    for name in ThemeCatalog::builtin().names() {
        assert!(
            harness.query_by_label(&edit_theme_label(name)).is_some(),
            "{name}"
        );
    }
    assert!(
        harness
            .query_by_label(&edit_theme_label("system"))
            .is_none()
    );
}

#[test]
fn test_theme_edit_button_opens_editor() {
    let scratch = ScratchThemes::scrub_themes();
    let (_rt, mut harness) = rig(&scratch, |_| {});
    harness.run();
    harness.get_by_label(&edit_theme_label("nord")).click();
    harness.run();
    let app = harness.state().app();
    assert_eq!(
        app.theme_editor.as_ref().map(|e| e.name.as_str()),
        Some("nord")
    );
    assert!(app.theme_picker.is_some(), "the picker stays underneath");
    assert!(dialog_shown(&harness, &editor_title("nord")));
    assert!(harness.query_by_label(&hex_label("accent")).is_some());
    assert!(harness.query_by_label(RESET_THEME).is_some());
}

#[test]
fn test_theme_editor_save_button_persists() {
    let scratch = ScratchThemes::scrub_themes();
    let (_rt, mut harness) = rig(&scratch, |app| {
        app.open_theme_editor("nord");
        app.set_theme_colour("accent", Rgb(200, 10, 10));
    });
    harness.run();
    harness.get_by_label(SAVE_THEME).click();
    harness.run();
    assert!(harness.state().app().theme_editor.is_none());
    let mut reread = ThemeCatalog::builtin();
    reread.load_dir(&scratch.themes());
    assert_eq!(reread.get("nord").expect("nord").accent, Rgb(200, 10, 10));
}

#[test]
fn test_theme_editor_cancel_button_closes() {
    let scratch = ScratchThemes::scrub_themes();
    let (_rt, mut harness) = rig(&scratch, |app| {
        app.open_theme_editor("nord");
        app.set_theme_colour("accent", Rgb(200, 10, 10));
    });
    harness.run();
    harness.get_by_label(CANCEL_EDIT).click();
    harness.run();
    let app = harness.state().app();
    assert!(app.theme_editor.is_none());
    assert!(app.theme_picker.is_some());
    assert!(!scratch.themes().join("nord.json").exists());
}

#[test]
fn test_theme_editor_reset_hidden_for_user_theme() {
    let scratch = ScratchThemes::scrub_themes();
    let mut theme = (*ThemeCatalog::builtin().get("nord").expect("nord")).clone();
    theme.name = "mine".to_string();
    save_theme(&theme, &scratch.themes().join("mine.json")).expect("save");
    let themes_dir = scratch.themes();
    let (_rt, mut harness) = rig(&scratch, |app| {
        app.themes.load_dir(&themes_dir);
        app.open_theme_editor("mine");
    });
    harness.run();
    assert!(dialog_shown(&harness, &editor_title("mine")));
    assert!(harness.query_by_label(RESET_THEME).is_none());
}

/// Press and release `key` with no modifiers.
fn press(harness: &mut Harness<'static, Shell>, key: egui::Key) {
    for pressed in [true, false] {
        harness.input_mut().events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
}

fn draft_accent(harness: &Harness<'static, Shell>) -> Rgb {
    harness
        .state()
        .app()
        .theme_editor
        .as_ref()
        .expect("editor open")
        .draft
        .accent
}

#[test]
fn test_theme_editor_hex_field_sets_valid_colour_only() {
    let scratch = ScratchThemes::scrub_themes();
    let (_rt, mut harness) = rig(&scratch, |app| app.open_theme_editor("nord"));
    harness.run();
    let before = draft_accent(&harness);

    // Typing after the shown value makes an invalid colour: nothing changes.
    harness.get_by_label(&hex_label("accent")).click();
    harness.run();
    press(&mut harness, egui::Key::End);
    harness.get_by_label(&hex_label("accent")).type_text("zz");
    harness.run();
    assert_eq!(draft_accent(&harness), before);

    // Clearing the field and typing a valid colour sets it.
    for _ in 0..9 {
        press(&mut harness, egui::Key::Backspace);
    }
    harness
        .get_by_label(&hex_label("accent"))
        .type_text("#c80a0a");
    harness.run();
    assert_eq!(draft_accent(&harness), Rgb(200, 10, 10));
}
