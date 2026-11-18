//! Tests for activity rows and thinking blocks in the Messages pane: a
//! folded run of tool calls opens and closes, keeps its state while the
//! message streams, shows a spinner while a call runs; thinking follows
//! `/thinking-display`.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::app::App;
use codemason_app_core::transcript_prefs::{ThinkingDisplay, Verbosity};
use codemason_gui::palette::palette_for;
use codemason_gui::panels::messages::{ACTIVITY_RUNNING_LABEL, MessagesView, THINKING_LABEL};
use egui::accesskit::Role as A11yRole;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

struct State {
    app: App,
    view: MessagesView,
}

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

fn call(tool: &str, id: &str, status: ToolCallStatus) -> MessagePart {
    MessagePart::ToolCall {
        tool: tool.to_string(),
        call_id: id.to_string(),
        state: Box::new(ToolCallState {
            status,
            input: serde_json::json!({ "command": "ls", "file_path": "/p/a.rs" }),
            output: None,
            error: None,
            duration_ms: Some(10),
        }),
    }
}

fn reply(parts: Vec<MessagePart>) -> Message {
    Message::new("s1", Role::Assistant, parts)
}

fn expanded(harness: &Harness<'_, State>, label: &str) -> Option<bool> {
    harness
        .get_by_role_and_label(A11yRole::Button, label)
        .accesskit_node()
        .data()
        .is_expanded()
}

#[tokio::test]
async fn test_activity_row_is_a_collapsed_button_that_opens() {
    let mut app = support::make_app();
    app.session.messages = vec![reply(vec![
        call("read", "c1", ToolCallStatus::Completed),
        call("Bash", "c2", ToolCallStatus::Completed),
    ])];
    let mut harness = harness(app);
    harness.run();
    let label = "Read 1 file, ran 1 command";
    assert_eq!(expanded(&harness, label), Some(false));
    assert!(harness.query_by_label_contains("Bash: completed").is_none());

    harness
        .get_by_role_and_label(A11yRole::Button, label)
        .click();
    harness.run();
    assert_eq!(expanded(&harness, label), Some(true));
    harness.get_by_label_contains("Bash: completed");
    harness.get_by_label_contains("Read: completed");
}

#[tokio::test]
async fn test_activity_row_shows_a_spinner_while_a_call_runs() {
    let mut app = support::make_app();
    app.session.messages = vec![reply(vec![call("Bash", "c1", ToolCallStatus::Running)])];
    let mut harness = harness(app);
    harness.run();
    harness.get_by_role_and_label(A11yRole::ProgressIndicator, ACTIVITY_RUNNING_LABEL);
}

#[tokio::test]
async fn test_activity_row_stays_open_while_streaming() {
    let mut app = support::make_app();
    app.session.is_processing = true;
    app.session.messages = vec![reply(vec![call("read", "c1", ToolCallStatus::Completed)])];
    let mut harness = harness(app);
    harness.run();
    harness
        .get_by_role_and_label(A11yRole::Button, "Read 1 file")
        .click();
    harness.run();

    // The same message gains a call, as while it streams.
    {
        let msg = &mut harness.state_mut().app.session.messages[0];
        msg.parts
            .push(call("read", "c2", ToolCallStatus::Completed));
        msg.edit_seq += 1;
    }
    harness.state_mut().app.session.is_processing = false;
    harness.run();
    assert_eq!(expanded(&harness, "Read 2 files"), Some(true));
}

#[tokio::test]
async fn test_verbosity_high_shows_every_call_and_low_folds_them() {
    let mut app = support::make_app();
    app.session.messages = vec![reply(vec![call("Bash", "c1", ToolCallStatus::Completed)])];
    app.transcript_prefs.verbosity = Verbosity::High;
    let mut harness = harness(app);
    harness.run();
    harness.get_by_label_contains("Bash: completed");
    assert!(
        harness
            .query_by_role_and_label(A11yRole::Button, "Ran 1 command")
            .is_none()
    );

    harness.state_mut().app.transcript_prefs.verbosity = Verbosity::Low;
    harness.run();
    harness.get_by_role_and_label(A11yRole::Button, "Ran 1 command");
}

fn thinking_reply() -> Message {
    reply(vec![
        MessagePart::Reasoning {
            text: "weighing options".to_string(),
            signature: None,
        },
        MessagePart::Text {
            text: "Answer.".to_string(),
        },
    ])
}

#[tokio::test]
async fn test_thinking_block_follows_the_display_setting() {
    let mut app = support::make_app();
    // At high verbosity reasoning is its own block, not folded.
    app.transcript_prefs.verbosity = Verbosity::High;
    app.session.is_processing = true;
    app.session.messages = vec![thinking_reply()];
    let mut harness = harness(app);
    harness.run();
    // Open while the reply streams.
    assert_eq!(expanded(&harness, THINKING_LABEL), Some(true));
    harness.get_by_label_contains("weighing options");

    // Closed once it completes.
    harness.state_mut().app.session.is_processing = false;
    harness.run();
    assert_eq!(expanded(&harness, THINKING_LABEL), Some(false));
    assert!(
        harness
            .query_by_label_contains("weighing options")
            .is_none()
    );

    harness.state_mut().app.transcript_prefs.thinking = ThinkingDisplay::Expanded;
    harness.run();
    harness.get_by_label_contains("weighing options");
    assert!(harness.query_by_label(THINKING_LABEL).is_none());

    harness.state_mut().app.transcript_prefs.thinking = ThinkingDisplay::Hidden;
    harness.run();
    assert!(
        harness
            .query_by_label_contains("weighing options")
            .is_none()
    );
    assert!(harness.query_by_label(THINKING_LABEL).is_none());
    harness.get_by_label("Answer.");
}
