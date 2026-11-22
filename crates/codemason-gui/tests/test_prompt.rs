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

use codemason_gui::dialogs::shortcuts::SHORTCUTS_TITLE;
use codemason_gui::material::thumbnail::REMOVE_LABEL;
use codemason_gui::panels::prompt::{ATTACHMENTS_LABEL, SLASH_LABEL};

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

    // egui-winit sends a printable press and its Text in one frame.
    harness.input_mut().events.extend([
        key_event(egui::Key::A, true),
        egui::Event::Text("a".to_string()),
        key_event(egui::Key::A, false),
    ]);
    harness.run();

    assert_eq!(
        harness.state().app().session.input,
        "a",
        "the prompt takes the focus and types the key once"
    );
    assert_eq!(prompt_text(&harness), "a");
    assert!(prompt(&harness).is_focused());
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
    let menu = harness.get_by_label(QUEUE_MENU_LABEL);
    harness.get_by_label_contains("2 pending");
    // Within the menu: the ribbon has a Stop button of its own.
    for row in ["Next", "Stop", "Clear", "Show"] {
        menu.get_by_label_contains(row);
    }

    harness.key_press(egui::Key::Escape);
    harness.run();

    assert!(
        !harness.state().app().queue_menu_open,
        "Esc closes the menu"
    );
    assert!(harness.query_by_label(QUEUE_MENU_LABEL).is_none());
}

#[test]
fn test_prompt_unfocused_text_without_key_inserts_once() {
    let (_rt, mut harness) = rig(|_| {});

    // A dead key or a layout character arrives as Text with no Key.
    harness.event(egui::Event::Text("\u{fc}".to_string()));
    harness.run();

    assert_eq!(harness.state().app().session.input, "\u{fc}");
    assert_eq!(prompt_text(&harness), "\u{fc}");
    assert!(prompt(&harness).is_focused());
}

#[test]
fn test_prompt_unfocused_ime_commit_inserts_once() {
    let (_rt, mut harness) = rig(|_| {});

    harness.event(egui::Event::Ime(egui::ImeEvent::Commit(
        "\u{e9}t\u{e9}".to_string(),
    )));
    harness.run();

    assert_eq!(harness.state().app().session.input, "\u{e9}t\u{e9}");
    assert_eq!(harness.state().app().session.input_cursor, 3);
}

#[test]
fn test_prompt_backspace_deletes_whole_grapheme() {
    // "a", then a thumbs-up emoji with a skin-tone modifier: one grapheme of
    // two code points.
    let text = "a\u{1f44d}\u{1f3fd}";
    let (_rt, mut harness) = rig_with(
        |app| {
            app.session.input = text.to_string();
            app.session.input_cursor = text.chars().count();
        },
        Harness::builder().allow_missing_glyphs(),
    );
    harness.run();

    // The prompt is not focused: Backspace must still edit by grapheme.
    harness.key_press(egui::Key::Backspace);
    harness.run();

    assert_eq!(harness.state().app().session.input, "a");
    assert_eq!(harness.state().app().session.input_cursor, 1);
    assert!(prompt(&harness).is_focused());
}

#[test]
fn test_prompt_left_moves_over_a_combining_sequence() {
    // "x", then "e" with a combining acute accent: one grapheme.
    let text = "xe\u{301}";
    let (_rt, mut harness) = rig(|app| {
        app.session.input = text.to_string();
        app.session.input_cursor = 3;
    });
    harness.run();

    harness.key_press(egui::Key::ArrowLeft);
    harness.run();

    assert_eq!(
        harness.state().app().session.input_cursor,
        1,
        "the caret never stops between e and its accent"
    );
}

#[test]
fn test_prompt_question_mark_on_empty_prompt_opens_shortcuts() {
    let (_rt, mut harness) = rig(|_| {});

    harness.input_mut().events.extend([
        key_event(egui::Key::Questionmark, true),
        egui::Event::Text("?".to_string()),
    ]);
    harness.run();

    let app = harness.state().app();
    assert_eq!(
        app.session.input, "",
        "the `?` that opens the panel never types"
    );
    assert!(
        app.show_shortcuts,
        "`?` on an empty prompt opens the panel, as in the TUI"
    );
    assert!(
        app.is_input_locked(),
        "the open panel holds the keyboard, as in the TUI"
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, SHORTCUTS_TITLE)
            .is_some(),
        "the panel is drawn"
    );
}

#[test]
fn test_prompt_question_mark_inside_text_types() {
    let (_rt, mut harness) = rig(|_| {});
    focus_prompt(&mut harness);
    prompt(&harness).type_text("why");
    harness.run();

    harness.input_mut().events.extend([
        key_event(egui::Key::Questionmark, true),
        egui::Event::Text("?".to_string()),
    ]);
    harness.run();

    let app = harness.state().app();
    assert_eq!(app.session.input, "why?");
    assert!(!app.show_shortcuts);
}

#[test]
fn test_prompt_text_then_question_mark_in_one_frame_types() {
    let (_rt, mut harness) = rig(|_| {});

    // "a?" typed fast: the `?` follows text, so the prompt is not empty.
    harness.input_mut().events.extend([
        key_event(egui::Key::A, true),
        egui::Event::Text("a".to_string()),
        key_event(egui::Key::Questionmark, true),
        egui::Event::Text("?".to_string()),
    ]);
    harness.run();
    harness.run();

    let app = harness.state().app();
    assert_eq!(app.session.input, "a?");
    assert!(!app.show_shortcuts);
}

#[test]
fn test_prompt_forwarded_question_mark_matches_tui() {
    // What the TUI does with `?` while a compaction locks the prompt.
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let tui_shows = runtime.block_on(async {
        let mut app = support::make_app();
        app.session.compact_in_progress = true;
        app.handle_key(codemason_app_core::input_event::KeyInput {
            key: codemason_app_core::input_event::Key::Char('?'),
            mods: codemason_app_core::input_event::Modifiers::NONE,
        })
        .await;
        (app.show_shortcuts, app.session.input.clone())
    });
    drop(runtime);
    let (_rt, mut harness) = rig(|app| app.session.compact_in_progress = true);

    // A locked prompt forwards every key to the App.
    harness.input_mut().events.extend([
        key_event(egui::Key::Questionmark, true),
        egui::Event::Text("?".to_string()),
    ]);
    harness.run();

    let app = harness.state().app();
    assert_eq!((app.show_shortcuts, app.session.input.clone()), tui_shows);
}

#[test]
fn test_prompt_typing_keeps_another_focused_widget() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(
            |ui, (shell, other): &mut (Shell, String)| {
                shell.update(ui);
                egui::Area::new(egui::Id::unique("other_field"))
                    .fixed_pos(egui::pos2(20.0, 20.0))
                    .show(ui.ctx(), |ui| {
                        ui.horizontal(|ui| {
                            let label = ui.label("Other field");
                            ui.text_edit_singleline(other).labelled_by(label.id);
                        });
                    });
            },
            (shell, String::new()),
        );
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Other field")
        .focus();
    harness.run();

    harness.input_mut().events.extend([
        key_event(egui::Key::A, true),
        egui::Event::Text("a".to_string()),
        key_event(egui::Key::A, false),
    ]);
    harness.run();

    let (shell, other) = harness.state();
    assert_eq!(other, "a", "the focused field keeps its typing");
    assert_eq!(shell.app().session.input, "", "the prompt takes nothing");
    assert!(
        !harness
            .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
            .is_focused()
    );
}

#[test]
fn test_prompt_slash_menu_shows_commands_and_tab_inserts() {
    let (_rt, mut harness) = rig(|_| {});
    focus_prompt(&mut harness);

    prompt(&harness).type_text("/hel");
    harness.run();

    harness.get_by_label(SLASH_LABEL);
    harness.get_by_label_contains("/help");
    let trigger = {
        let menu = harness
            .state()
            .app()
            .slash_menu
            .as_ref()
            .expect("slash menu");
        menu.matches[menu.selected].trigger.clone()
    };

    harness.key_press(egui::Key::Tab);
    harness.run();

    assert_eq!(harness.state().app().session.input, format!("/{trigger} "));
    assert!(
        harness.query_by_label(SLASH_LABEL).is_none(),
        "Tab accepted"
    );
    assert!(prompt(&harness).is_focused());
}

#[test]
fn test_prompt_up_on_first_line_browses_history() {
    let (_rt, mut harness) = rig(|app| app.input_history = vec!["older".to_string()]);
    focus_prompt(&mut harness);
    prompt(&harness).type_text("x");
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    harness.run();
    prompt(&harness).type_text("y");
    harness.run();

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    let app = harness.state().app();
    assert_eq!(app.session.input, "x\ny", "Up first moves to line one");
    assert_eq!(app.session.input_cursor, 1);

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().app().session.input, "older");
    assert_eq!(prompt_text(&harness), "older");

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(
        harness.state().app().session.input,
        "x\ny",
        "Down while browsing returns to the draft"
    );
}

#[test]
fn test_prompt_shows_pending_attachments() {
    let (_rt, mut harness) = rig(|app| {
        app.session.pending_attachments = vec![PathBuf::from("shots/screen.png")];
    });
    harness.run();

    harness.get_by_label(ATTACHMENTS_LABEL);
    // Each attachment is a thumbnail of the image file, so its accessible
    // name is the file's name.
    harness.get_by_label("screen.png");
}

#[test]
fn test_prompt_shows_a_thumbnail_per_attachment() {
    let (_rt, mut harness) = rig(|app| {
        app.session.pending_attachments = vec![
            PathBuf::from("shots/first.png"),
            PathBuf::from("shots/second.png"),
            PathBuf::from("shots/third.png"),
        ];
    });
    harness.run();

    harness.get_by_label(ATTACHMENTS_LABEL);
    for name in ["first.png", "second.png", "third.png"] {
        harness.get_by_label(name);
    }
}

#[test]
fn test_prompt_removes_an_attachment_from_its_close_mark() {
    let (_rt, mut harness) = rig(|app| {
        app.session.pending_attachments = vec![
            PathBuf::from("shots/first.png"),
            PathBuf::from("shots/second.png"),
        ];
    });
    harness.run();
    // One close mark per attachment; each carries the same accessible name,
    // so the second is the one that takes `second.png` off.
    let marks = harness.get_all_by_label(REMOVE_LABEL).count();
    assert_eq!(marks, 2, "one close mark per attachment");

    harness
        .get_all_by_label(REMOVE_LABEL)
        .last()
        .expect("a mark")
        .click();
    harness.run();

    let remaining = &harness.state().app().session.pending_attachments;
    assert_eq!(remaining, &vec![PathBuf::from("shots/first.png")]);
    assert_eq!(
        harness.get_all_by_label(REMOVE_LABEL).count(),
        1,
        "the strip follows the list"
    );
}

#[test]
fn test_prompt_attachment_strip_disappears_with_its_last_image() {
    let (_rt, mut harness) = rig(|app| {
        app.session.pending_attachments = vec![PathBuf::from("shots/only.png")];
    });
    harness.run();
    harness.get_by_label(ATTACHMENTS_LABEL);

    harness.get_by_label(REMOVE_LABEL).click();
    harness.run();

    assert!(
        harness.state().app().session.pending_attachments.is_empty(),
        "the last attachment was removed"
    );
    assert!(
        harness.query_by_label(ATTACHMENTS_LABEL).is_none(),
        "an empty strip draws nothing"
    );
}

/// A 2x1 opaque red PNG, so an attached image has a real picture to load.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0xF4, 0x22, 0x7F,
    0x8A, 0x00, 0x00, 0x00, 0x0E, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x38, 0xA1, 0xA1, 0xF1,
    0x1F, 0x84, 0x01, 0x12, 0x33, 0x04, 0x2F, 0xB4, 0x17, 0x82, 0x1E, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn test_prompt_double_click_on_an_attachment_opens_the_image_view() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/prompt_attachment_view")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("screen.png");
    std::fs::write(&path, PNG).expect("write png");
    let attached = path.clone();
    let (_rt, mut harness) = rig_with(
        move |app| app.session.pending_attachments = vec![attached],
        // The picture decodes on a background thread, which repaints until it
        // is done; four steps are not always enough for the file to arrive.
        Harness::builder().with_max_steps(64),
    );
    // The shell installs these at startup (`main_thread`); the harness is a
    // bare context, so a `file://` image would never load and the picture
    // would sense no click.
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    assert!(
        codemason_gui::dialogs::image_view::shown(&harness.ctx).is_none(),
        "no view before the click"
    );

    // Below the close mark, which covers the box's top-right quarter.
    let pos = harness.get_by_label("screen.png").rect().left_bottom()
        + egui::vec2(codemason_gui::material::thumbnail::SIZE / 2.0, -12.0);
    // Each click is one frame (press and release), so the two are one frame
    // apart, inside egui's double-click delay.
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        harness.step();
    }
    harness.run();

    let shown = codemason_gui::dialogs::image_view::shown(&harness.ctx).expect("the view opened");
    assert_eq!(shown.name, "screen.png");
    assert_eq!(
        harness.state().app().session.pending_attachments,
        vec![path],
        "enlarging keeps the attachment staged"
    );
}

#[test]
fn test_prompt_typing_is_not_swallowed_by_a_focused_button() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(
            |ui, shell: &mut Shell| {
                shell.update(ui);
                egui::Area::new(egui::Id::unique("other_button"))
                    .fixed_pos(egui::pos2(20.0, 20.0))
                    .show(ui.ctx(), |ui| {
                        let _ = ui.button("Other button"); // INTENTIONAL: only its focus matters
                    });
            },
            shell,
        );
    harness.run();
    // As if Tab had moved egui's focus onto the button.
    harness.get_by_label("Other button").focus();
    harness.run();
    assert!(harness.get_by_label("Other button").is_focused());

    harness.input_mut().events.extend([
        key_event(egui::Key::A, true),
        egui::Event::Text("a".to_string()),
        key_event(egui::Key::A, false),
    ]);
    harness.run();

    assert_eq!(
        harness.state().app().session.input,
        "a",
        "a button keeps no typing: the prompt takes it"
    );
    assert!(prompt(&harness).is_focused());
}

#[test]
fn test_prompt_has_no_visible_label() {
    let (_rt, harness) = rig(|_| {});

    // The name is on the editor itself; no separate "Prompt" label is drawn.
    assert_eq!(harness.query_all_by_label(PROMPT_LABEL).count(), 1);
    prompt(&harness);
}

#[test]
fn test_prompt_grows_with_its_text_up_to_eight_rows() {
    let height = |lines: usize| {
        let text = vec!["line"; lines].join("\n");
        let (_rt, mut harness) = rig(move |app| app.session.input = text);
        harness.run();
        codemason_gui::panels::prompt::prompt_visible_height(&harness.ctx)
            .expect("the prompt was drawn")
    };
    let one = height(1);
    let three = height(3);
    let eight = height(8);
    let thirty = height(30);

    assert!(
        three > one * 2.0,
        "three rows are taller than one: {one} -> {three}"
    );
    assert!(
        eight > three,
        "the editor keeps growing: {three} -> {eight}"
    );
    assert!(
        thirty <= eight + 1.0,
        "beyond eight rows the editor scrolls instead of growing: {eight} -> {thirty}"
    );
}

#[test]
fn test_collapse_button_sits_left_of_the_prompt_and_collapses_the_transcript() {
    use codemason_agent::message::{
        Message, MessagePart, Role as MessageRole, ToolCallState, ToolCallStatus,
    };
    use codemason_app_core::transcript_prefs::Verbosity;
    use codemason_gui::panels::messages::COLLAPSE_ALL_LABEL;

    let tool_call = MessagePart::ToolCall {
        tool: "read".to_string(),
        call_id: "c1".to_string(),
        state: Box::new(ToolCallState {
            status: ToolCallStatus::Completed,
            input: serde_json::json!({ "file_path": "/p/a.rs" }),
            output: None,
            error: None,
            duration_ms: Some(1),
        }),
    };
    let (_rt, mut harness) = rig(move |app| {
        app.transcript_prefs.verbosity = Verbosity::Low;
        app.session.messages = vec![Message::new("s1", MessageRole::Assistant, vec![tool_call])];
    });
    harness.run();

    // The button is left of the editor on the same row.
    let button = harness.get_by_label(COLLAPSE_ALL_LABEL).rect();
    let editor = prompt(&harness).rect();
    assert!(
        button.right() <= editor.left() + 0.5,
        "{button:?} {editor:?}"
    );
    assert!((button.center().y - editor.center().y).abs() <= editor.height() / 2.0);
}

#[test]
fn test_scroll_to_bottom_button_sits_left_of_the_collapse_button() {
    use codemason_gui::panels::messages::{COLLAPSE_ALL_LABEL, SCROLL_TO_BOTTOM_LABEL};

    let (_rt, mut harness) = rig(|_| {});
    harness.run();

    let jump = harness.get_by_label(SCROLL_TO_BOTTOM_LABEL).rect();
    let collapse = harness.get_by_label(COLLAPSE_ALL_LABEL).rect();
    let editor = prompt(&harness).rect();
    // Left of the collapse button, which is itself left of the field.
    assert!(
        jump.right() <= collapse.left() + 0.5,
        "jump {jump:?} collapse {collapse:?}"
    );
    assert!(
        collapse.right() <= editor.left() + 0.5,
        "collapse {collapse:?} editor {editor:?}"
    );
    assert!((jump.center().y - editor.center().y).abs() <= editor.height() / 2.0);
}

#[test]
fn test_scroll_to_bottom_button_jumps_the_transcript_to_its_end() {
    use codemason_agent::message::Message;
    use codemason_gui::panels::messages::SCROLL_TO_BOTTOM_LABEL;

    // Scrolling animates over a few frames, so let the harness settle it.
    let (_rt, mut harness) = rig_with(
        |app| {
            app.session.messages = (0..300)
                .map(|i| Message::assistant_text("s1", format!("message {i}")))
                .collect();
        },
        Harness::builder().with_max_steps(64),
    );
    harness.run();
    harness.get_by_label_contains("message 299");

    // Scroll the transcript up to its start.
    harness.hover_at(egui::pos2(400.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Page,
        delta: egui::vec2(0.0, 100_000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    harness.get_by_label("message 0");
    assert!(
        harness.query_by_label_contains("message 299").is_none(),
        "the end of the transcript is out of view"
    );

    harness.get_by_label(SCROLL_TO_BOTTOM_LABEL).click();
    harness.run();

    harness.get_by_label_contains("message 299");
    assert!(
        harness.query_by_label("message 0").is_none(),
        "the start of the transcript is out of view again"
    );
}

#[test]
fn test_prompt_row_height_is_not_inflated_by_the_collapse_button() {
    let (_rt, mut harness) = rig(|_| {});
    harness.run();
    let height = codemason_gui::panels::prompt::prompt_visible_height(&harness.ctx)
        .expect("the prompt was drawn");
    // A one-row prompt stays a text row tall: an 18-point dense button does
    // not push it to a Material 40-point target.
    assert!(height < 30.0, "one-row prompt is {height} points tall");
}

#[test]
fn test_prompt_field_grows_upwards_only() {
    let (_rt, mut harness) = rig(|_| {});
    harness.run();
    let one_line = prompt(&harness).rect();
    let field = codemason_gui::page::field_height(&harness.ctx).expect("field drawn");
    // Padding and outline on both sides, plus the inline button's overhang.
    let chrome = 2.0f32.mul_add(codemason_gui::page::FIELD_PADDING + 1.0, 8.0);
    assert!(
        field <= one_line.height() + chrome,
        "the field hugs its one-line editor: field {field}, editor {one_line:?}"
    );

    focus_prompt(&mut harness);
    prompt(&harness).type_text("one\ntwo\nthree\nfour");
    harness.run();
    let four_lines = prompt(&harness).rect();

    assert!(
        four_lines.top() < one_line.top() - 1.0,
        "a taller text moves the top up: {one_line:?} -> {four_lines:?}"
    );
    assert!(
        (four_lines.bottom() - one_line.bottom()).abs() <= 0.5,
        "the bottom stays put: {one_line:?} -> {four_lines:?}"
    );
}

#[test]
fn test_prompt_leaves_a_gap_below_the_attachment_thumbnails() {
    let (_rt, mut harness) = rig(|app| {
        app.session.pending_attachments = vec![PathBuf::from("shots/screen.png")];
    });
    harness.run();
    let strip = harness.get_by_label(ATTACHMENTS_LABEL).rect();
    let editor = prompt(&harness).rect();
    assert!(
        editor.top() - strip.bottom() >= codemason_gui::metrics::space::SM,
        "8pt between thumbnails and editor: strip {strip:?}, editor {editor:?}"
    );
}
