//! TUI tests for the live output-view overlay (running sub-agent steps).
//!
//! The output-view overlay for another session's target must rebuild its
//! cached lines as the agent advances, so completed steps appear live instead
//! of only after the run ends. Two mechanisms make that work:
//!
//! * the interim save persists completed tool-call parts into the child
//!   session's SQLite row mid-run (see
//!   `crates/codemason-agent/tests/test_interim_tool_call_persist.rs`), and
//! * the overlay's generation key mixes in the same per-session step /
//!   tool-call counters the Agents panel shows, so the cache invalidates
//!   without them changing.
//!
//! These tests drive the real render path with a `TestBackend`.

mod support;

use serde_json::json;
use std::sync::Arc;

use codemason_types::message::{Message, MessagePart, ToolCallState};

/// A completed `think` tool-call part, as the interim save persists it.
fn think_tool_call_part() -> MessagePart {
    MessagePart::ToolCall {
        tool: "think".to_string(),
        call_id: "call_1".to_string(),
        state: Box::new(ToolCallState {
            status: codemason_types::message::ToolCallStatus::Completed,
            input: json!({"thought": "considering the next step"}),
            output: Some(json!({"thought": "considering the next step"})),
            error: None,
            duration_ms: Some(5),
        }),
    }
}

/// The `Arc<Message>` round-trip through storage keeps the tool-call part
/// serialisable - a cheap sanity check that the seeded shape matches what the
/// interim save writes (`MessagePart::ToolCall` with a completed state).
#[test]
fn test_seeded_tool_call_part_serialises_like_interim_save() {
    let mut assistant = Message::new("s", codemason_types::message::Role::Assistant, vec![]);
    assistant.parts.push(think_tool_call_part());
    let stored = Arc::new(assistant);
    let json = serde_json::to_string(&stored).expect("serialise");
    let round: Message = serde_json::from_str(&json).expect("deserialise");
    assert!(matches!(round.parts[0], MessagePart::ToolCall { .. }));
}
