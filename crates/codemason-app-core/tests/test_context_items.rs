//! Tests for the Context panel's per-item breakdown
//! (`codemason_app_core::context_items`).

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::app::ContextItem;
use codemason_app_core::context_items::{
    TOOL_CALLS_ITEM, bytes_to_tokens, history_items, memory_items, message_bytes, skill_items,
    tool_items,
};
use codemason_types::llm::ToolDefinition;
use serde_json::json;

/// A tool definition named `name` with a description of `len` bytes.
fn def(name: &str, len: usize) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: "x".repeat(len),
        parameters: json!({}),
    }
}

#[test]
fn test_context_items_tools_split_into_built_in_and_mcp() {
    let defs = [
        def("bash", 400),
        def("mcp_github_issue", 800),
        def("read", 40),
    ];
    let (built_in, mcp) = tool_items(&defs, |name| {
        name.strip_prefix("mcp_github_")
            .map(|tool| format!("github: {tool}"))
    });
    let names: Vec<&str> = built_in.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["bash", "read"], "largest first");
    assert_eq!(mcp.len(), 1);
    assert_eq!(mcp[0].name, "github: issue");
    let sum: u64 = built_in.iter().chain(&mcp).map(|i| i.tokens).sum();
    let expected: u64 = defs
        .iter()
        .map(|d| bytes_to_tokens(d.name.len() + d.description.len() + 2 + 60))
        .sum();
    assert_eq!(sum, expected, "items cost what the catalog costs");
}

#[test]
fn test_context_items_skills_are_parsed_from_the_section() {
    let section = "## Available Skills\n\nYou can invoke:\n\n\
                   - `/review [pr]`  - Review a pull request in depth\n\
                   - `/fmt`  - Format\n";
    let items = skill_items(section);
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["review", "fmt"]);
    assert!(items[0].tokens > items[1].tokens);
}

#[test]
fn test_context_items_memories_take_their_tag_lines() {
    let long = "a".repeat(60);
    let section = format!(
        "## Relevant Memories\n- [pref] short (confidence: 0.90)\n  tags: one, two\n- [fact] {long}\n\n"
    );
    let items = memory_items(&section);
    assert_eq!(items.len(), 2);
    let short = items
        .iter()
        .find(|i| i.name.starts_with("[pref]"))
        .map(|i| i.tokens);
    let alone = bytes_to_tokens("- [pref] short (confidence: 0.90)".len() + 1);
    assert!(short > Some(alone), "the tags line counts toward its entry");
    assert!(
        items.iter().any(|i| i.name.ends_with("...")),
        "long names are cut: {items:?}"
    );
}

#[test]
fn test_context_items_history_splits_roles_and_tool_calls() {
    let call = MessagePart::ToolCall {
        tool: "bash".to_string(),
        call_id: "c1".to_string(),
        state: Box::new(ToolCallState {
            status: ToolCallStatus::Completed,
            input: json!({"command": "ls"}),
            output: Some(json!("a b c")),
            error: None,
            duration_ms: None,
        }),
    };
    let messages = vec![
        Message::user_text("s", "hello there"),
        Message::new(
            "s",
            Role::Assistant,
            vec![
                MessagePart::Text {
                    text: "running it".to_string(),
                },
                call,
            ],
        ),
    ];
    let items = history_items(&messages, true);
    let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
    for expected in ["User messages", "Assistant messages", TOOL_CALLS_ITEM] {
        assert!(names.contains(&expected), "{names:?}");
    }
    let bytes: usize = messages.iter().map(|m| message_bytes(m, true)).sum();
    let sum: u64 = items.iter().map(|i| i.tokens).sum();
    assert!(sum <= bytes_to_tokens(bytes) && sum + 3 >= bytes_to_tokens(bytes));
}

#[test]
fn test_context_items_empty_inputs_give_no_items() {
    assert_eq!(skill_items(""), Vec::<ContextItem>::new());
    assert_eq!(memory_items(""), Vec::<ContextItem>::new());
    assert_eq!(history_items(&[], true), Vec::<ContextItem>::new());
}

#[test]
fn test_context_items_history_counts_tool_results_as_the_request_sends_them() {
    // A tool call whose output carries a huge result plus metadata repeating
    // it, as `wait_agents` and file reads do. The request sends only the
    // result text, cut to the agent's tool-result budget, so the estimate
    // must not count the raw JSON output.
    let big = "y".repeat(1_000_000);
    let message = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"command": "cat big.log"}),
                output: Some(json!({"content": big, "metadata": {"full": big}})),
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let bytes = message_bytes(&message, true);
    assert!(
        bytes < 20_000,
        "a 1 MB result is sent cut to the tool-result budget, got {bytes} bytes"
    );
    assert!(bytes > 14_000, "the cut result is still counted: {bytes}");
}

#[test]
fn test_context_items_reasoning_counts_only_when_the_provider_replays_it() {
    let message = Message::new(
        "s",
        Role::Assistant,
        vec![
            MessagePart::Reasoning {
                text: "z".repeat(40_000),
                signature: None,
            },
            MessagePart::Text {
                text: "done".to_string(),
            },
        ],
    );
    let sent = message_bytes(&message, true);
    let dropped = message_bytes(&message, false);
    assert_eq!(sent - dropped, 40_000);
    let items = history_items(std::slice::from_ref(&message), false);
    assert!(items.iter().all(|i| i.tokens < 100), "{items:?}");
}
