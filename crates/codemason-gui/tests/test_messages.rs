//! Tests for the GUI message transcript view
//! (`codemason_gui::panels::messages`).
//!
//! GUI assertions use egui_kittest accessibility queries only (no wgpu
//! snapshots), so no GPU or display is needed.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::app::App;
use codemason_app_core::message_lines::MESSAGE_STREAM_MIN_INTERVAL;
use codemason_gui::Shell;
use codemason_gui::palette::palette_for;
use codemason_gui::panels::messages::{MESSAGES_LABEL, MessagesView};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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
                let palette = palette_for(state.app.theme_mode);
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
    assert!(harness.query_by_label_contains("message 0\n").is_none());
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

    harness.get_by_label_contains("message 0\n");
    assert!(harness.query_by_label_contains("message 499").is_none());
    assert!(harness.state().view.layout_count() <= 200);
}

#[tokio::test]
async fn test_message_view_tool_call_shows_status_label() {
    let mut app = support::make_app();
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
    harness.get_by_label_contains("You: hello there");
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
