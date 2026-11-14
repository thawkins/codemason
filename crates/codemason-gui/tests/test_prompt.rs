//! Tests for the GUI prompt editor (`codemason_gui::panels::prompt`) and the
//! input queue view (`codemason_gui::panels::queue`), driven through the
//! whole [`Shell`] frame so key routing and the App's own input buffer are
//! exercised together.
//!
//! GUI assertions use egui_kittest accessibility queries only (no wgpu
//! snapshots), so no GPU or display is needed. Each test owns a tokio
//! runtime that it does not enter while the harness runs, because the shell
//! drives `App::handle_key` with `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use codemason_app_core::app::{App, ConfiguredProvider, ProviderSource, QueuedInput};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::prompt::{MENTION_LABEL, PROMPT_LABEL};
use codemason_gui::panels::queue::{QUEUE_LABEL, QUEUE_MENU_LABEL};
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A shell over `app` with sources attached, and the runtime driving it.
///
/// The runtime is returned so it outlives the harness; it is entered only
/// while `configure` builds the app.
fn rig(configure: impl FnOnce(&mut App)) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    rig_with(configure, Harness::builder())
}

/// [`rig`] with the harness built from `builder`.
fn rig_with(
    configure: impl FnOnce(&mut App),
    builder: egui_kittest::HarnessBuilder<Shell>,
) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        configure(&mut app);
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = builder
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

/// The prompt's text input node.
fn prompt<'h>(harness: &'h Harness<'static, Shell>) -> egui_kittest::Node<'h> {
    harness.get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
}

/// The prompt's text, as the accessibility tree reports it.
fn prompt_text(harness: &Harness<'static, Shell>) -> String {
    prompt(harness).value().unwrap_or_default()
}

/// A press (or release) of `key` with no modifiers.
fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Focus the prompt and let the focus settle.
fn focus_prompt(harness: &mut Harness<'static, Shell>) {
    prompt(harness).focus();
    harness.run();
}

/// A scratch project directory under `target/temp` holding `files`.
fn scratch_project(name: &str, files: &[&str]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/codemason-gui-test-prompt")
        .join(name);
    std::fs::create_dir_all(&dir).expect("create scratch project");
    for file in files {
        std::fs::write(dir.join(file), "fn main() {}\n").expect("write scratch file");
    }
    dir
}

/// Configure a provider and a running turn, so Enter queues the prompt.
fn busy_with_provider(app: &mut App) {
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::Database,
    });
    app.session.selected_model = Some("ollama/qwen2.5".to_string());
    app.session.is_processing = true;
}

#[test]
fn test_prompt_typed_text_lands_in_app_buffer_once() {
    let (_rt, mut harness) = rig(|_| {});
    focus_prompt(&mut harness);

    prompt(&harness).type_text("hi");
    harness.run();

    assert_eq!(harness.state().app().session.input, "hi");
    assert_eq!(harness.state().app().session.input_cursor, 2);
    assert_eq!(prompt_text(&harness), "hi");
}

#[test]
fn test_prompt_shows_keys_typed_while_unfocused() {
    let (_rt, mut harness) = rig(|_| {});

    harness.key_press(egui::Key::A);
    harness.event(egui::Event::Text("a".to_string()));
    harness.run();

    assert_eq!(
        harness.state().app().session.input,
        "a",
        "the App inserts the forwarded key once; its Text is not typed again"
    );
    assert_eq!(prompt_text(&harness), "a");
}

#[test]
fn test_prompt_enter_submits_and_clears() {
    let (_rt, mut harness) = rig(busy_with_provider);
    focus_prompt(&mut harness);
    prompt(&harness).type_text("fix the parser");
    harness.run();

    harness.key_press(egui::Key::Enter);
    harness.run();

    let app = harness.state().app();
    assert_eq!(app.session.input, "", "the App's submit path cleared it");
    assert_eq!(app.session.input_queue.len(), 1);
    assert_eq!(app.session.input_queue[0].text, "fix the parser");
    assert_eq!(prompt_text(&harness), "");
    assert!(prompt(&harness).is_focused(), "Enter keeps the prompt");
}

#[test]
fn test_prompt_text_and_enter_in_one_frame_submit_in_order() {
    let (_rt, mut harness) = rig(busy_with_provider);
    focus_prompt(&mut harness);

    // Kittest sends each queued event in a frame of its own; put all four
    // into one frame, as a fast typist or an input tool can.
    harness.input_mut().events.extend([
        egui::Event::Text("abc".to_string()),
        key_event(egui::Key::Enter, true),
        key_event(egui::Key::Enter, false),
        egui::Event::Text("d".to_string()),
    ]);
    harness.run();

    let app = harness.state().app();
    assert_eq!(app.session.input_queue.len(), 1);
    assert_eq!(app.session.input_queue[0].text, "abc");
    assert_eq!(
        app.session.input, "d",
        "text after Enter starts the next prompt"
    );
}

#[test]
fn test_prompt_shift_enter_inserts_newline() {
    let (_rt, mut harness) = rig(busy_with_provider);
    focus_prompt(&mut harness);
    prompt(&harness).type_text("a");
    harness.run();

    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    harness.run();
    prompt(&harness).type_text("b");
    harness.run();

    assert_eq!(harness.state().app().session.input, "a\nb");
    assert_eq!(prompt_text(&harness), "a\nb");
    assert!(harness.state().app().session.input_queue.is_empty());
}

#[test]
fn test_prompt_alt_enter_inserts_newline() {
    let (_rt, mut harness) = rig(busy_with_provider);
    focus_prompt(&mut harness);
    prompt(&harness).type_text("a");
    harness.run();

    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::Enter);
    harness.run();

    assert_eq!(harness.state().app().session.input, "a\n");
    assert_eq!(prompt_text(&harness), "a\n", "the App's newline shows");
    assert!(harness.state().app().session.input_queue.is_empty());
}

#[test]
fn test_prompt_at_mention_opens_completion() {
    let project = scratch_project("mention", &["hello_world.rs"]);
    let (_rt, mut harness) = rig(|app| app.cwd_path = project);
    focus_prompt(&mut harness);

    prompt(&harness).type_text("see @hello");
    harness.run();

    harness.get_by_label(MENTION_LABEL);
    harness.get_by_label("hello_world.rs");

    harness.key_press(egui::Key::Tab);
    harness.run();

    assert_eq!(harness.state().app().session.input, "see @hello_world.rs");
    assert_eq!(prompt_text(&harness), "see @hello_world.rs");
    assert!(
        harness.query_by_label(MENTION_LABEL).is_none(),
        "Tab accepted"
    );
    assert!(prompt(&harness).is_focused(), "Tab stays in the prompt");
}

#[test]
fn test_prompt_editor_ime_commit_inserts_text() {
    // The bundled test font has no CJK glyphs; the assertions read the text,
    // not the pixels, so tofu is fine here.
    let (_rt, mut harness) = rig_with(|_| {}, Harness::builder().allow_missing_glyphs());
    focus_prompt(&mut harness);

    harness.event(egui::Event::Ime(egui::ImeEvent::Commit(
        "\u{4f60}\u{597d}".to_string(),
    )));
    harness.run();

    assert_eq!(prompt_text(&harness), "\u{4f60}\u{597d}");
    let app = harness.state().app();
    assert_eq!(
        app.session.input, "\u{4f60}\u{597d}",
        "inserted exactly once"
    );
    assert_eq!(app.session.input_cursor, 2, "the cursor follows the commit");
}

#[test]
fn test_prompt_ime_enter_confirming_a_word_does_not_submit() {
    let (_rt, mut harness) = rig(busy_with_provider);
    focus_prompt(&mut harness);

    harness.input_mut().events.extend([
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".to_string(),
            active_range_chars: None,
        }),
        key_event(egui::Key::Enter, true),
        key_event(egui::Key::Enter, false),
    ]);
    harness.run();

    assert!(
        harness.state().app().session.input_queue.is_empty(),
        "the IME owns the Enter that confirms its word"
    );
}

#[test]
fn test_prompt_paste_while_unfocused_focuses_and_pastes() {
    let (_rt, mut harness) = rig(|_| {});

    harness.event(egui::Event::Paste("one\r\ntwo".to_string()));
    harness.run();

    assert!(prompt(&harness).is_focused());
    assert_eq!(harness.state().app().session.input, "one\ntwo");
    assert_eq!(prompt_text(&harness), "one\ntwo");
}

#[test]
fn test_queue_menu_shows_queued_items() {
    let (_rt, mut harness) = rig(|app| {
        busy_with_provider(app);
        for text in ["first queued", "second queued"] {
            app.session.input_queue.push_back(QueuedInput {
                text: text.to_string(),
                image_paths: Vec::new(),
            });
        }
    });
    harness.run();
    harness.get_by_label(QUEUE_LABEL);
    harness.get_by_label_contains("first queued");
    harness.get_by_label_contains("second queued");

    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::Q);
    harness.run();

    assert!(harness.state().app().queue_menu_open);
    harness.get_by_label(QUEUE_MENU_LABEL);
    harness.get_by_label_contains("2 pending");
    for row in ["Next", "Stop", "Clear", "Show"] {
        harness.get_by_label_contains(row);
    }

    harness.key_press(egui::Key::Escape);
    harness.run();

    assert!(
        !harness.state().app().queue_menu_open,
        "Esc closes the menu"
    );
    assert!(harness.query_by_label(QUEUE_MENU_LABEL).is_none());
}
