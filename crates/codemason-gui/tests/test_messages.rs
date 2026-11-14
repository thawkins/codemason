//! Tests for the GUI message transcript view
//! (`codemason_gui::panels::messages`).
//!
//! GUI assertions use egui_kittest accessibility queries only (no wgpu
//! snapshots), so no GPU or display is needed.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::app::App;
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
