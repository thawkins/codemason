//! Tests for the window clipboard (`codemason_gui::platform::clipboard`):
//! an image pasted with Ctrl+V/Cmd+V (egui's `PasteImage`, which egui-winit
//! sends when the clipboard holds an image and no text) is staged through
//! app-core's attachment path, and app-core's copy actions reach the
//! window's clipboard (`Context::copy_text`).
//!
//! GUI assertions use egui_kittest accessibility queries and the frame's
//! platform output. The pasted PNG lands in the process `target/temp/`
//! (app-core's paste directory) and is removed by the test.

#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;

use codemason_app_core::app::{App, HistoryPickerState};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui::{Color32, ColorImage, OutputCommand};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The session the tests run in.
const SESSION: &str = "session-clipboard";

/// A shell over `app` with sources attached, and the runtime driving it.
fn rig(configure: impl FnOnce(&mut App)) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        configure(&mut app);
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

/// A 2 x 2 red image, as egui-winit hands a pasted clipboard image over.
fn pasted_image() -> egui::Event {
    egui::Event::PasteImage(Arc::new(ColorImage::new([2, 2], vec![Color32::RED; 4])))
}

#[test]
fn test_pasted_image_is_staged_as_an_attachment() {
    let (_runtime, mut harness) = rig(|_| {});
    harness.run();

    harness.input_mut().events.push(pasted_image());
    harness.run();

    let attachments = harness.state().app().session.pending_attachments.clone();
    assert_eq!(attachments.len(), 1, "one image staged");
    let path = &attachments[0];
    let saved = image_dims(path);
    for path in &attachments {
        std::fs::remove_file(path).expect("remove pasted image");
    }
    assert_eq!(saved, (2, 2), "the pasted pixels were saved as a PNG");
    let name = path
        .file_name()
        .expect("file name")
        .to_string_lossy()
        .into_owned();
    assert!(name.starts_with("codemason_paste_"), "{name}");
    // The prompt lists the staged image.
    harness.get_by_label(&name);
}

#[test]
fn test_pasted_image_while_a_dialog_owns_the_window_is_left_alone() {
    let (_runtime, mut harness) = rig(|app| {
        app.history_picker = Some(HistoryPickerState {
            entries: vec!["newest".to_string()],
            selected: 0,
            scroll_offset: 0,
        });
    });
    harness.run();

    harness.input_mut().events.push(pasted_image());
    harness.run();

    assert!(harness.state().app().session.pending_attachments.is_empty());
}

#[test]
fn test_app_core_copy_reaches_the_window_clipboard() {
    let (_runtime, mut harness) = rig(|app| {
        app.history_picker = Some(HistoryPickerState {
            entries: vec!["newest".to_string(), "older".to_string()],
            selected: 0,
            scroll_offset: 0,
        });
    });
    harness.run();

    // `c` copies the highlighted history entry (app-core's history picker).
    let input = harness.input_mut();
    input.events.push(key(egui::Key::C, true));
    input.events.push(egui::Event::Text("c".to_string()));
    input.events.push(key(egui::Key::C, false));
    harness.step();

    let copied: Vec<&OutputCommand> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter(|c| matches!(c, OutputCommand::CopyText(_)))
        .collect();
    assert_eq!(copied, vec![&OutputCommand::CopyText("newest".to_string())]);
    assert_eq!(harness.state().app().status, "history: copied to clipboard");
}

/// A key press or release with no modifiers.
fn key(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// The width and height of the PNG at `path`, read from its header.
fn image_dims(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("read pasted image");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG file");
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    (be(16), be(20))
}
