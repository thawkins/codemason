//! Tests for `codemason_app_core::message_blocks`: a message split into
//! transcript blocks.

use std::collections::HashMap;

use codemason_agent::message::{
    ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus,
};
use codemason_app_core::message_blocks::{TranscriptBlock, message_to_blocks};
use serde_json::json;

fn blocks(msg: &Message) -> Vec<TranscriptBlock> {
    message_to_blocks(msg, &HashMap::<String, (String, u32, u32)>::new(), "/proj")
}

fn tool(status: ToolCallStatus) -> MessagePart {
    MessagePart::ToolCall {
        tool: "bash".to_string(),
        call_id: "call-1".to_string(),
        state: Box::new(ToolCallState {
            status,
            input: json!({"command": "ls"}),
            output: None,
            error: None,
            duration_ms: None,
        }),
    }
}

#[test]
fn test_message_blocks_assistant_text_is_markdown_and_user_text_is_plain() {
    let assistant = Message::assistant_text("s", "# Title\n\nSome *text*.");
    assert_eq!(
        blocks(&assistant),
        [TranscriptBlock::Markdown(
            "# Title\n\nSome *text*.".to_string()
        )]
    );
    let user = Message::user_text("s", "fix **this**");
    assert_eq!(
        blocks(&user),
        [TranscriptBlock::Plain("fix **this**".to_string())]
    );
    let summary = Message::new(
        "s",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "## Objective".to_string(),
        }],
    );
    assert_eq!(
        blocks(&summary),
        [TranscriptBlock::Markdown("## Objective".to_string())]
    );
}

#[test]
fn test_message_blocks_keep_part_order_and_skip_empty_text() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "Looking.".to_string(),
            },
            tool(ToolCallStatus::Running),
            MessagePart::Text {
                text: "  \n".to_string(),
            },
            MessagePart::Image(Box::new(ImageData {
                mime_type: "image/png".to_string(),
                path: "/tmp/shot.png".into(),
            })),
        ],
    );
    let got = blocks(&msg);
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[0], TranscriptBlock::Markdown("Looking.".to_string()));
    let TranscriptBlock::Tool {
        lines,
        status_label,
    } = &got[1]
    else {
        panic!("tool block expected: {got:?}");
    };
    assert_eq!(status_label, "Bash: running");
    assert!(lines[0].text().contains("Bash"), "{lines:?}");
    assert!(
        lines.last().is_some_and(|l| !l.text().trim().is_empty()),
        "the trailing separator line is dropped"
    );
    assert_eq!(
        got[2],
        TranscriptBlock::Image {
            name: "shot.png".to_string(),
            path: "/tmp/shot.png".into(),
        }
    );
    assert_eq!(got[2].plain_text(), "[image: shot.png]");
}

#[test]
fn test_message_blocks_reasoning_and_notices() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "first\nsecond".to_string(),
            signature: None,
        }],
    );
    let got = blocks(&msg);
    let [TranscriptBlock::Thinking(lines)] = got.as_slice() else {
        panic!("thinking block expected: {got:?}");
    };
    assert_eq!(got[0].plain_text(), "  first\n  second");
    assert_eq!(lines.len(), 2);

    let notice = Message::assistant_text("s", "Agent Notice: build finished");
    let got = blocks(&notice);
    assert!(
        matches!(got.as_slice(), [TranscriptBlock::Notice(_)]),
        "{got:?}"
    );
}

#[test]
fn test_message_blocks_neutralise_terminal_escapes() {
    let msg = Message::assistant_text("s", "red \u{1b}[31mtext");
    let got = blocks(&msg);
    let [TranscriptBlock::Markdown(text)] = got.as_slice() else {
        panic!("markdown expected: {got:?}");
    };
    assert!(!text.contains('\u{1b}'), "{text:?}");
}
