//! Tests for `append_language_reminder`: the per-request reply-language
//! reminder added to the newest user message so long contexts do not drift
//! out of the configured language.

use codemason_agent::llm::{ChatContent, ChatMessage, ContentPart};
use codemason_agent::session::prompt_builders::append_language_reminder;

fn user_text(text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn text_of(message: &ChatMessage) -> String {
    match &message.content {
        ChatContent::Text(t) => t.clone(),
        ChatContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[test]
fn test_language_reminder_appends_to_last_user_text() {
    let mut msgs = vec![user_text("first"), user_text("continue")];
    append_language_reminder(&mut msgs, Some("English"));
    assert_eq!(text_of(&msgs[0]), "first", "older messages are untouched");
    let last = text_of(&msgs[1]);
    assert!(last.starts_with("continue"), "got: {last}");
    assert!(last.contains("Reply in English"), "got: {last}");
}

#[test]
fn test_language_reminder_adds_text_part_after_tool_results() {
    let mut msgs = vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: "ok".into(),
            is_error: false,
        }]),
    }];
    append_language_reminder(&mut msgs, Some("English"));
    let ChatContent::Parts(parts) = &msgs[0].content else {
        panic!("tool-result message must stay structured");
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[0], ContentPart::ToolResult { .. }));
    assert!(matches!(&parts[1], ContentPart::Text { text } if text.contains("Reply in English")));
}

#[test]
fn test_language_reminder_skips_blank_language_and_non_user_tail() {
    let mut msgs = vec![user_text("hi")];
    append_language_reminder(&mut msgs, Some("  "));
    append_language_reminder(&mut msgs, None);
    assert_eq!(text_of(&msgs[0]), "hi");

    let mut msgs = vec![ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Text("answer".to_string()),
    }];
    append_language_reminder(&mut msgs, Some("English"));
    assert_eq!(text_of(&msgs[0]), "answer");
}
