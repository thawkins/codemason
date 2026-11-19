//! Tests for `/editor` in the window (`codemason_gui::external_edit`): the
//! leader key then `e` runs the editor off the UI thread, and the prompt
//! takes what it saved.
#![cfg(unix)]

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui_kittest::Harness;

/// A window whose editor overwrites the prompt file with `edited`.
fn rig() -> (
    tokio::runtime::Runtime,
    tempfile::TempDir,
    Harness<'static, Shell>,
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp");
    std::fs::create_dir_all(&root).expect("mkdir target/temp");
    let scratch = tempfile::Builder::new()
        .prefix("gui-editor-")
        .tempdir_in(root)
        .expect("tempdir");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.input = "draft".to_string();
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone())
        .with_external_editor(
            r#"sh -c 'printf edited > "$0"'"#.to_string(),
            scratch.path().to_path_buf(),
        );
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, scratch, harness)
}

/// Press and release `key` with `modifiers`, then draw a few frames.
///
/// A fixed number of frames, not `Harness::run`: once the editor thread is
/// running the window keeps polling it (and the App's housekeeping timer
/// may be due), so it need not settle within `run`'s step limit.
fn key(harness: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    let input = harness.input_mut();
    for pressed in [true, false] {
        input.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        });
    }
    harness.run_steps(4);
}

#[test]
fn test_leader_e_edits_the_prompt_in_the_external_editor() {
    let (_rt, _scratch, mut harness) = rig();
    harness.run();
    key(&mut harness, egui::Key::G, egui::Modifiers::CTRL);
    key(&mut harness, egui::Key::E, egui::Modifiers::NONE);
    // The editor runs on its own thread; frames keep coming meanwhile.
    for _ in 0..200 {
        if !harness.state().app().external_editor_open {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        harness.step();
    }
    let app = harness.state().app();
    assert!(!app.external_editor_open, "the editor finished");
    assert_eq!(app.session.input, "edited");
}
