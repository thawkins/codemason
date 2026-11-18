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

#[test]
fn test_message_blocks_split_top_level_code_blocks() {
    use codemason_app_core::message_blocks::split_code_blocks;

    let md = |t: &str| TranscriptBlock::Markdown(t.to_string());
    let code = |lang: Option<&str>, t: &str| TranscriptBlock::Code {
        lang: lang.map(str::to_string),
        text: t.to_string(),
    };
    assert_eq!(
        split_code_blocks(
            "Intro.\n\n```rust title\nfn main() {}\n```\n\nBetween.\n\n```\nplain\n```"
        ),
        [
            md("Intro."),
            code(Some("rust"), "fn main() {}"),
            md("Between."),
            code(None, "plain"),
        ]
    );
    // A code block inside a list item stays in the list's Markdown.
    let listed = "1. Run:\n\n   ```bash\n   ls\n   ```\n2. Done";
    assert_eq!(split_code_blocks(listed), [md(listed)]);
    // An unclosed fence (still streaming) runs to the end.
    assert_eq!(
        split_code_blocks("Now:\n```py\nprint(1)\n"),
        [md("Now:"), code(Some("py"), "print(1)")]
    );
    // Only a code block.
    assert_eq!(split_code_blocks("```\nx\n```"), [code(None, "x")]);
    assert_eq!(split_code_blocks("  \n"), []);
}

#[test]
fn test_message_blocks_assistant_code_blocks_become_code_blocks() {
    let msg = Message::assistant_text("s", "Try:\n\n```sh\nmake\n```");
    assert_eq!(
        blocks(&msg),
        [
            TranscriptBlock::Markdown("Try:".to_string()),
            TranscriptBlock::Code {
                lang: Some("sh".to_string()),
                text: "make".to_string(),
            },
        ]
    );
    // The user's fences are left as typed.
    let user = Message::user_text("s", "```\nx\n```");
    assert_eq!(
        blocks(&user),
        [TranscriptBlock::Plain("```\nx\n```".to_string())]
    );
}

#[test]
fn test_message_blocks_reference_links_resolve_across_code_blocks() {
    use codemason_app_core::message_blocks::split_code_blocks;

    let md =
        "See [the docs][d].\n\n```\ncode\n```\n\nAlso [d].\n\n[d]: https://example.com \"Docs\"\n";
    let blocks = split_code_blocks(md);
    let [
        TranscriptBlock::Markdown(before),
        TranscriptBlock::Code { .. },
        TranscriptBlock::Markdown(after),
    ] = blocks.as_slice()
    else {
        panic!("markdown, code, markdown expected: {blocks:?}");
    };
    // The fragment before the code carries the definition from after it.
    assert!(before.starts_with("See [the docs][d]."), "{before:?}");
    assert!(
        before.contains("[d]: https://example.com \"Docs\""),
        "{before:?}"
    );
    assert!(after.contains("[d]: https://example.com"), "{after:?}");
    // Without a code block the Markdown is left as written.
    let plain = "[a][x]\n\n[x]: https://x.test";
    assert_eq!(
        split_code_blocks(plain),
        [TranscriptBlock::Markdown(plain.to_string())]
    );
}
