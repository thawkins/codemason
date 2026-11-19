//! Tests for the GUI message transcript view
//! (`codemason_gui::panels::messages`).
//!
//! GUI assertions use egui_kittest accessibility queries only (no wgpu
//! snapshots), so no GPU or display is needed.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{
    ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus,
};
use codemason_app_core::app::App;
use codemason_app_core::app::ContextAction;
use codemason_app_core::message_lines::MESSAGE_STREAM_MIN_INTERVAL;
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};
use codemason_gui::Shell;
use codemason_gui::dialogs::image_view::{
    ACTUAL_SIZE, CLOSE_IMAGE, FIT_TO_WINDOW, IMAGE_VIEW_TITLE,
};
use codemason_gui::palette::{Palette, palette_for};
use codemason_gui::panels::code_block::{COPY, INSERT, NO_LANGUAGE, WRAP, code_block_ui, code_job};
use codemason_gui::panels::messages::{MESSAGES_LABEL, MessagesView, USER_CHIP};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// The app plus the view under test, as one kittest state.
struct State {
    app: App,
    view: MessagesView,
}

/// A harness 800 x 600 points that draws only the transcript. Scrolling
/// animates over a few frames, so `run` may take up to 64 steps to settle.
fn harness(app: App) -> Harness<'static, State> {
    Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .with_max_steps(64)
        .build_ui_state(
            |ui, state: &mut State| {
                let palette = palette_for(&state.app, ui.ctx());
                state.view.show(ui, &state.app, &palette);
            },
            State {
                app,
                view: MessagesView::default(),
            },
        )
}

/// An assistant message with one tool call in `status`.
fn tool_message(status: ToolCallStatus) -> Message {
    Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(ToolCallState {
                status,
                input: serde_json::json!({ "command": "ls" }),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    )
}

#[tokio::test]
async fn test_message_view_lays_out_only_visible_rows() {
    let mut app = support::make_app();
    app.session.messages = (0..20_000)
        .map(|i| Message::assistant_text("s1", format!("message {i}")))
        .collect();
    let mut harness = harness(app);
    harness.run();

    let laid_out = harness.state().view.layout_count();
    assert!(laid_out > 0, "the visible messages are laid out");
    assert!(laid_out <= 200, "laid out {laid_out} messages in one frame");
    // The view starts at the bottom of the transcript.
    harness.get_by_label_contains("message 19999");
    assert!(harness.query_by_label("message 0").is_none());
}

#[tokio::test]
async fn test_message_view_scroll_wheel_reveals_earlier_messages() {
    let mut app = support::make_app();
    app.session.messages = (0..500)
        .map(|i| Message::assistant_text("s1", format!("message {i}")))
        .collect();
    let mut harness = harness(app);
    harness.run();
    assert!(harness.query_by_label_contains("message 499").is_some());

    harness.hover_at(egui::pos2(400.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Page,
        delta: egui::vec2(0.0, 1000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();

    harness.get_by_label("message 0");
    assert!(harness.query_by_label_contains("message 499").is_none());
    assert!(harness.state().view.layout_count() <= 200);
}

#[tokio::test]
async fn test_message_view_tool_call_shows_status_label() {
    let mut app = support::make_app();
    // Each call is its own block at high verbosity; low folds them into an
    // activity row (`test_activity_rows`).
    app.transcript_prefs.verbosity = codemason_app_core::transcript_prefs::Verbosity::High;
    app.session.messages = vec![
        tool_message(ToolCallStatus::Running),
        tool_message(ToolCallStatus::Error),
    ];
    let mut harness = harness(app);
    harness.run();

    harness.get_by_label(MESSAGES_LABEL);
    harness.get_by_label_contains("Bash: running");
    harness.get_by_label_contains("Bash: error");

    // The status follows the message as it changes.
    let msg = &mut harness.state_mut().app.session.messages[0];
    if let MessagePart::ToolCall { state, .. } = &mut msg.parts[0] {
        state.status = ToolCallStatus::Completed;
    }
    msg.touch();
    harness.run();
    harness.get_by_label_contains("Bash: completed");
    assert!(harness.query_by_label_contains("Bash: running").is_none());
}

#[tokio::test]
async fn test_shell_shows_messages_above_prompt() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::user_text("s1", "hello there")];
    let shell = Shell::new(app);
    let harness = Harness::new_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);

    harness.get_by_label(MESSAGES_LABEL);
    harness.get_by_label("hello there");
    harness.get_by_label(USER_CHIP);
    harness.get_by_label("Prompt");
}

/// `count` assistant messages `"message 0"`, `"message 1"`, ...
fn numbered_messages(count: usize) -> Vec<Message> {
    (0..count)
        .map(|i| Message::assistant_text("s1", format!("message {i}")))
        .collect()
}

/// Evict the oldest message and append `"message <n>"`, the way the core
/// trims a session at its message cap.
fn evict_and_push(app: &mut App, n: usize) {
    app.session.messages.remove(0);
    app.session
        .messages
        .push(Message::assistant_text("s1", format!("message {n}")));
}

#[tokio::test]
async fn test_message_view_eviction_renders_only_the_new_message() {
    let mut app = support::make_app();
    app.session.messages = numbered_messages(500);
    let mut harness = harness(app);
    harness.run();
    let renders_before = harness.state().view.render_count();
    let heights_before = harness.state().view.message_heights();

    evict_and_push(&mut harness.state_mut().app, 500);
    harness.run();

    let view = &harness.state().view;
    assert_eq!(view.render_count() - renders_before, 1);
    let heights_after = view.message_heights();
    assert_eq!(heights_after.len(), 500);
    assert_eq!(heights_after[..499], heights_before[1..]);
    harness.get_by_label_contains("message 500");
}

#[tokio::test]
async fn test_message_view_eviction_keeps_scrolled_up_position() {
    let mut app = support::make_app();
    app.session.messages = numbered_messages(500);
    let mut harness = harness(app);
    harness.run();

    harness.hover_at(egui::pos2(400.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 3000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let anchor = harness
        .state()
        .view
        .first_visible_id()
        .map(str::to_owned)
        .expect("a visible message");
    assert!(harness.query_by_label_contains("message 499").is_none());

    evict_and_push(&mut harness.state_mut().app, 500);
    harness.run();

    assert_eq!(
        harness.state().view.first_visible_id(),
        Some(anchor.as_str())
    );
    assert!(harness.query_by_label_contains("message 500").is_none());
}

#[tokio::test]
async fn test_message_view_throttles_streaming_rerenders() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "chunk")];
    app.session.is_processing = true;
    let mut harness = harness(app);
    let start = std::time::Instant::now();
    harness.state_mut().view.set_clock(start);
    harness.run();
    let renders_before = harness.state().view.render_count();

    // Many deltas inside one throttle interval (the clock is frozen).
    for _ in 0..20 {
        let msg = &mut harness.state_mut().app.session.messages[0];
        if let MessagePart::Text { text } = &mut msg.parts[0] {
            text.push_str(" chunk");
        }
        msg.touch();
        harness.step();
    }
    assert!(harness.state().view.render_count() - renders_before <= 1);

    // Once the interval has passed, the pending delta is drawn.
    let after_interval = renders_before;
    harness
        .state_mut()
        .view
        .set_clock(start + MESSAGE_STREAM_MIN_INTERVAL);
    harness.step();
    assert!(harness.state().view.render_count() > after_interval);

    // A finished message re-renders at once, even inside the interval.
    let before_finish = harness.state().view.render_count();
    {
        let state = harness.state_mut();
        let msg = &mut state.app.session.messages[0];
        if let MessagePart::Text { text } = &mut msg.parts[0] {
            text.push_str(" done");
        }
        msg.touch();
        state.app.session.is_processing = false;
    }
    harness.step();
    assert_eq!(harness.state().view.render_count() - before_finish, 1);
    harness.get_by_label_contains("chunk done");
}

#[tokio::test]
async fn test_empty_session_shows_the_empty_state() {
    use codemason_gui::panels::empty_state::{EMPTY_STATE_LABEL, STARTERS, TIP, WORDMARK};

    let app = support::make_app();
    assert!(app.session.messages.is_empty());
    let shell = Shell::new(app);
    let mut harness = Harness::new_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    harness.run();

    harness.get_by_label(EMPTY_STATE_LABEL);
    harness.get_by_label(WORDMARK);
    harness.get_by_label(&format!("Version {}", env!("CARGO_PKG_VERSION")));
    harness.get_by_label(TIP);
    for (label, _) in STARTERS {
        harness.get_by_label(label);
    }

    // A starter action fills the prompt and leaves sending to the user.
    harness.get_by_label("List the commands").click();
    harness.run();
    assert_eq!(harness.state().app().session.input, "/help");
    assert!(harness.state().app().session.messages.is_empty());
}

#[tokio::test]
async fn test_session_with_messages_has_no_empty_state() {
    use codemason_gui::panels::empty_state::EMPTY_STATE_LABEL;

    let mut app = support::make_app();
    app.session.messages = vec![Message::user_text("s1", "hello there")];
    let shell = Shell::new(app);
    let mut harness = Harness::new_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    harness.run();

    assert!(harness.query_by_label(EMPTY_STATE_LABEL).is_none());
}

#[tokio::test]
async fn test_starter_action_replaces_a_draft_and_its_completion_menu() {
    let mut app = support::make_app();
    // The user had started a slash command: its completion menu is open.
    app.session.input = "/mod".to_string();
    app.input_edited(4);
    assert!(app.slash_menu.is_some(), "the slash menu is open for /mod");
    let shell = Shell::new(app);
    // Tall enough that the completion popup above the prompt does not cover
    // the starter buttons at the top of the Messages pane.
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 1400.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    harness.run();

    harness.get_by_label("Explain this project").click();
    harness.run();

    let app = harness.state().app();
    assert_eq!(
        app.session.input,
        "Explain what this project does and how it is organised."
    );
    assert_eq!(app.session.input_cursor, app.session.input.chars().count());
    assert!(
        app.slash_menu.is_none(),
        "the old draft's menu must not survive to act on Enter"
    );
}

/// A response with a heading, a table, a link and two fenced code blocks.
const RICH_REPLY: &str = "# Plan\n\nRun the **tests** first.\n\n\
| Step | Command |\n|---|---|\n| build | cargo build |\n\n\
See [the docs](https://example.com/docs).\n\n\
```rust\nfn main() {}\n```\n\n```\nplain block\n```\n";

#[tokio::test]
async fn test_messages_render_markdown_blocks() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", RICH_REPLY)];
    let mut harness = harness(app);
    harness.run();

    harness.get_by_label("Plan");
    harness.get_by_label_contains("tests");
    harness.get_by_label("build");
    harness.get_by_label("cargo build");
    harness.get_by_role_and_label(egui::accesskit::Role::Link, "the docs");
    // The raw Markdown is not shown.
    assert!(harness.query_by_label_contains("# Plan").is_none());
    assert!(harness.query_by_label_contains("**tests**").is_none());
    assert!(harness.query_by_label_contains("```").is_none());
    // Both code blocks are drawn with their language and buttons.
    harness.get_by_label("fn main() {}");
    harness.get_by_label("plain block");
    harness.get_by_label("rust");
    harness.get_by_label(NO_LANGUAGE);
    for button in [COPY, WRAP, INSERT] {
        assert_eq!(harness.query_all_by_label(button).count(), 2, "{button}");
    }
}

#[test]
fn test_code_block_job_uses_the_theme_syntax_colours() {
    let themes = ThemeCatalog::builtin();
    let palette = Palette::from_tokens(&themes.resolve(&ThemeSelection::System, true));
    let job = code_job("fn main() {} // hi", Some("rust"), &palette);
    let color_of = |needle: &str| {
        job.sections
            .iter()
            .find(|section| {
                job.text[section.byte_range.start.0..section.byte_range.end.0].contains(needle)
            })
            .map(|section| section.format.color)
    };
    assert_eq!(color_of("fn"), Some(palette.syntax.keyword));
    assert_eq!(color_of("// hi"), Some(palette.syntax.comment));
    assert_eq!(job.text, "fn main() {} // hi");
    // An unknown language is drawn in the body text colour.
    let plain = code_job("x = 1", Some("cobol"), &palette);
    assert!(
        plain
            .sections
            .iter()
            .all(|s| s.format.color == palette.text)
    );
}

#[tokio::test]
async fn test_code_block_insert_puts_the_code_in_the_prompt() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text(
        "s1",
        "Run:\n\n```sh\ncargo test\n```",
    )];
    app.session.input = "please".to_string();
    let shell = Shell::new(app);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), shell);
    harness.run();
    // The code block's button, not the ribbon's Insert tab.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, INSERT)
        .click();
    harness.run();
    assert_eq!(harness.state().app().session.input, "please\ncargo test");
}

#[tokio::test]
async fn test_code_block_wrap_toggles() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "```\nx\n```")];
    let mut harness = harness(app);
    harness.run();
    let selected = |harness: &Harness<'_, State>| {
        harness.get_by_label(WRAP).accesskit_node().toggled()
            == Some(egui::accesskit::Toggled::True)
    };
    assert!(!selected(&harness));
    harness.get_by_label(WRAP).click();
    harness.run();
    assert!(selected(&harness));
}

#[tokio::test]
async fn test_messages_user_turn_is_plain_text_with_a_chip() {
    let mut app = support::make_app();
    app.session.messages = vec![
        Message::user_text("s1", "fix **this** please"),
        Message::assistant_text("s1", "done"),
    ];
    let mut harness = harness(app);
    harness.run();
    // The user's text is shown as typed, with a `You` chip and no prefix.
    harness.get_by_label("fix **this** please");
    harness.get_by_label(USER_CHIP);
    assert!(harness.query_by_label_contains("You:").is_none());
    harness.get_by_label("done");
}

#[tokio::test]
async fn test_messages_widgets_sit_under_the_transcript_node() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "inside the log")];
    let mut harness = harness(app);
    harness.run();
    let node = harness.get_by_label("inside the log");
    let mut ancestor = node.accesskit_node().parent();
    let mut found = false;
    while let Some(parent) = ancestor {
        if parent.label().as_deref() == Some(MESSAGES_LABEL) {
            found = true;
            break;
        }
        ancestor = parent.parent();
    }
    assert!(found, "the text is not under the {MESSAGES_LABEL} node");
}

/// A window on `app` with the whole shell.
fn shell_harness(app: App) -> Harness<'static, Shell> {
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .with_max_steps(64)
        .build_ui_state(|ui, shell: &mut Shell| shell.show(ui), Shell::new(app))
}

#[tokio::test]
async fn test_transcript_menu_quotes_the_message_in_reply() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "Use **cargo**.")];
    let mut harness = shell_harness(app);
    harness.run();

    harness.get_by_label_contains("cargo").click_secondary();
    harness.run();
    let menu = harness
        .state()
        .app()
        .context_menu
        .clone()
        .expect("menu open");
    // A press on selectable text places an egui selection there, so Copy is
    // enabled too (it copies nothing while that selection is empty).
    assert_eq!(
        menu.items,
        [
            (ContextAction::Copy, true),
            (ContextAction::CopyMarkdown, true),
            (ContextAction::QuoteReply, true),
        ]
    );
    harness.get_by_label("Quote in reply").click();
    harness.run();
    let app = harness.state().app();
    assert!(app.context_menu.is_none());
    assert_eq!(app.session.input, "> Use **cargo**.\n\n");
}

#[tokio::test]
async fn test_transcript_menu_off_a_message_has_nothing_to_quote() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "short")];
    let mut harness = shell_harness(app);
    harness.run();
    // Below the only message, still inside the transcript.
    let log = harness.get_by_label(MESSAGES_LABEL);
    let rect = log.rect();
    harness.hover_at(egui::pos2(rect.center().x, rect.bottom() - 20.0));
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: egui::pos2(rect.center().x, rect.bottom() - 20.0),
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: egui::pos2(rect.center().x, rect.bottom() - 20.0),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let menu = harness
        .state()
        .app()
        .context_menu
        .clone()
        .expect("menu open");
    assert!(menu.items.iter().all(|(_, enabled)| !enabled), "{menu:?}");
}

/// A message from the user with the image at `path` attached.
fn image_message(path: &std::path::Path) -> Message {
    Message::new(
        "s1",
        Role::User,
        vec![MessagePart::Image(Box::new(ImageData {
            mime_type: "image/png".to_string(),
            path: path.to_path_buf(),
        }))],
    )
}

#[tokio::test]
async fn test_image_thumbnail_opens_the_image_view() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_messages_image");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("shot.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 40, 40, 255]))
        .save(&path)
        .expect("write png");
    let mut app = support::make_app();
    app.session.messages = vec![image_message(&path)];
    let mut harness = shell_harness(app);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    harness.get_by_label("Image: shot.png").click();
    harness.run();
    let dialog = |harness: &Harness<'_, Shell>| {
        harness
            .query_by_role_and_label(egui::accesskit::Role::Dialog, IMAGE_VIEW_TITLE)
            .is_some()
    };
    assert!(dialog(&harness));
    // Actual size and back: the toggle names what it will do next.
    harness.get_by_label(ACTUAL_SIZE).click();
    harness.run();
    harness.get_by_label(FIT_TO_WINDOW).click();
    harness.run();
    harness.get_by_label(ACTUAL_SIZE);
    harness.get_by_label(CLOSE_IMAGE).click();
    harness.run();
    assert!(!dialog(&harness));
}

#[tokio::test]
async fn test_missing_image_is_named_not_drawn() {
    let mut app = support::make_app();
    app.session.messages = vec![image_message(std::path::Path::new(
        "target/temp/no-such-shot.png",
    ))];
    let mut harness = shell_harness(app);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    harness.get_by_label("[image: no-such-shot.png]");
    assert!(harness.query_by_label("Image: no-such-shot.png").is_none());
}

#[tokio::test]
async fn test_streaming_reply_ends_with_a_caret() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "partial")];
    let mut harness = harness(app);
    harness.run();
    let settled = harness.state().view.message_heights()[0];

    harness.state_mut().app.session.is_processing = true;
    harness.state_mut().app.session.messages[0].edit_seq += 1;
    harness.run();
    let streaming = harness.state().view.message_heights()[0];
    assert!(
        streaming > settled,
        "{streaming} <= {settled}: no caret row"
    );
}

#[test]
fn test_code_block_layout_is_reused_until_the_code_or_theme_changes() {
    let themes = ThemeCatalog::builtin();
    let dark = Palette::from_tokens(&themes.resolve(&ThemeSelection::System, true));
    let light = Palette::from_tokens(&themes.resolve(&ThemeSelection::System, false));
    let id = egui::Id::unique("code-cache-test");
    let slot = id.with("job");
    let mut harness = Harness::new_ui_state(
        |ui, (code, palette): &mut (String, Palette)| {
            let _ = code_block_ui(ui, id, Some("rust"), code, palette);
        },
        ("fn a() {}".to_string(), dark),
    );
    let cached = |harness: &Harness<'_, (String, Palette)>| {
        harness
            .ctx
            .data(|d| d.get_temp::<(u64, std::sync::Arc<egui::text::LayoutJob>)>(slot))
            .expect("cached layout")
    };
    harness.run();
    let (key, first) = cached(&harness);
    harness.run();
    let (same_key, again) = cached(&harness);
    assert_eq!(key, same_key);
    assert!(
        std::sync::Arc::ptr_eq(&first, &again),
        "the layout is reused"
    );
    harness.state_mut().0 = "fn b() {}".to_string();
    harness.run();
    let (code_key, _) = cached(&harness);
    assert_ne!(key, code_key, "changed code is laid out again");
    harness.state_mut().1 = light;
    harness.run();
    let (theme_key, _) = cached(&harness);
    assert_ne!(code_key, theme_key, "a theme change is laid out again");
}

#[tokio::test]
async fn test_output_view_code_block_insert_reaches_the_prompt() {
    let mut view = MessagesView::default();
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text("s1", "```\nmake\n```")];
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let palette = palette_for(&state.app, ui.ctx());
                state.view.show(ui, &state.app, &palette);
                state.view.apply_actions(ui.ctx(), &mut state.app);
            },
            State {
                app,
                view: std::mem::take(&mut view),
            },
        );
    harness.run();
    // The code block's button, not the ribbon's Insert tab.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, INSERT)
        .click();
    harness.run();
    assert_eq!(harness.state().app.session.input, "make");
}

/// A scratch folder of `name` under the git-ignored `target/temp`.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(name);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[tokio::test]
async fn test_reply_image_is_inline_at_reading_width_and_opens_the_image_view() {
    use codemason_gui::panels::messages::{INLINE_IMAGE_WIDTH, THUMBNAIL_SIZE};
    let root = scratch("test_messages_reply_image");
    std::fs::create_dir_all(root.join("out")).expect("out dir");
    // A wide JPEG: it decodes, and it is drawn wider than a thumbnail.
    image::RgbImage::from_pixel(1200, 300, image::Rgb([30, 90, 200]))
        .save(root.join("out/chart.jpg"))
        .expect("write jpeg");
    let mut app = support::make_app();
    app.cwd_path = root;
    app.session.messages = vec![Message::assistant_text(
        "s1",
        "Here is the chart:\n\n![Sales by month](out/chart.jpg)\n\nAs you can see.",
    )];
    let mut harness = shell_harness(app);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    let image = harness.get_by_label("Image: Sales by month");
    let width = image.rect().width();
    assert!(
        width > THUMBNAIL_SIZE && width <= INLINE_IMAGE_WIDTH + 0.5,
        "inline image is {width} wide"
    );
    harness.get_by_label_contains("As you can see.");
    image.click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Dialog, IMAGE_VIEW_TITLE)
            .is_some()
    );
}

#[tokio::test]
async fn test_reply_web_image_is_named_not_loaded() {
    let mut app = support::make_app();
    app.session.messages = vec![Message::assistant_text(
        "s1",
        "![logo](https://example.com/logo.png)",
    )];
    let mut harness = shell_harness(app);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    harness.get_by_label("[image: logo] (web images are not loaded)");
    assert!(harness.query_by_label("Image: logo").is_none());
}

#[test]
fn test_reply_image_source_resolves_paths_and_refuses_urls() {
    use codemason_gui::panels::messages::{ReplyImageSource, reply_image_source};
    let root = std::path::Path::new("/work/project");
    assert_eq!(
        reply_image_source("out/a.png", root),
        ReplyImageSource::File(root.join("out/a.png"))
    );
    assert_eq!(
        reply_image_source("/tmp/b.png", root),
        ReplyImageSource::File("/tmp/b.png".into())
    );
    assert_eq!(
        reply_image_source("file:///tmp/c.png", root),
        ReplyImageSource::File("/tmp/c.png".into())
    );
    for url in [
        "https://x.test/a.png",
        "http://x.test/a.png",
        "data:image/png;base64,AA",
    ] {
        assert_eq!(
            reply_image_source(url, root),
            ReplyImageSource::NotLoaded,
            "{url}"
        );
    }
}

#[tokio::test]
async fn test_reply_svg_image_is_drawn_and_opens_the_image_view() {
    let root = scratch("test_messages_reply_svg");
    std::fs::write(
        root.join("diagram.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="120"><rect width="300" height="120" fill="#3060c0"/></svg>"##,
    )
    .expect("write svg");
    let mut app = support::make_app();
    app.cwd_path = root;
    app.session.messages = vec![Message::assistant_text(
        "s1",
        "![Architecture](diagram.svg)",
    )];
    let mut harness = shell_harness(app);
    egui_extras::install_image_loaders(&harness.ctx);
    harness.run();
    let image = harness.get_by_label("Image: Architecture");
    let size = image.rect().size();
    assert!(
        (size.x - 300.0).abs() < 1.0 && (size.y - 120.0).abs() < 1.0,
        "drawn at its own size: {size:?}"
    );
    image.click();
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(egui::accesskit::Role::Dialog, IMAGE_VIEW_TITLE)
            .is_some()
    );
}
