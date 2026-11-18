//! Tests for `codemason_app_core::activity`: folding tool calls and
//! reasoning into activity rows, their labels, and result truncation.

use std::collections::HashMap;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::activity::{
    ActivityGroup, RESULT_PREVIEW_MAX_LINES, group_activity, truncate_result_lines,
};
use codemason_app_core::message_blocks::{ToolState, TranscriptBlock, message_to_blocks};
use codemason_app_core::styled::StyledLine;
use codemason_app_core::transcript_prefs::Verbosity;
use serde_json::json;

/// A finished (or failed, or running) call of `name` that took `ms`.
fn tool(name: &str, state: ToolState, ms: u64) -> TranscriptBlock {
    TranscriptBlock::Tool {
        lines: vec![StyledLine::plain(name)],
        status_label: format!("{name}: done"),
        name: name.to_string(),
        state,
        duration_ms: Some(ms),
    }
}

fn md(text: &str) -> TranscriptBlock {
    TranscriptBlock::Markdown(text.to_string())
}

fn label(items: Vec<TranscriptBlock>) -> String {
    ActivityGroup::new(items).summary.label()
}

#[test]
fn test_activity_label_counts_kinds_and_time() {
    let mut items: Vec<TranscriptBlock> = (0..4)
        .map(|_| tool("read", ToolState::Done, 1_000))
        .collect();
    items.push(tool("Edit", ToolState::Done, 2_000));
    items.push(tool("Write", ToolState::Done, 2_000));
    items.push(tool("Bash", ToolState::Done, 4_300));
    assert_eq!(
        label(items),
        "Read 4 files, edited 2 files, ran 1 command (12 s)"
    );
    assert_eq!(
        label(vec![tool("read", ToolState::Done, 300)]),
        "Read 1 file"
    );
    assert_eq!(
        label(vec![
            tool("Bash", ToolState::Done, 10),
            tool("Bash", ToolState::Failed, 10),
            tool("Bash", ToolState::Done, 10),
        ]),
        "Ran 3 commands, 1 failed"
    );
    assert_eq!(
        label(vec![
            tool("Grep", ToolState::Done, 0),
            tool("codeindex_search", ToolState::Done, 0),
            tool("calculator", ToolState::Done, 0),
            TranscriptBlock::Thinking(vec![StyledLine::plain("hmm")]),
            tool("think", ToolState::Done, 0),
        ]),
        "Ran 2 searches, used 1 tool, thought 2 times"
    );
    // Retired spellings count as their adopted tool.
    assert_eq!(
        label(vec![tool("edit", ToolState::Done, 0)]),
        "Edited 1 file"
    );
    assert_eq!(
        label(vec![tool("read", ToolState::Done, 125_000)]),
        "Read 1 file (2 m 05 s)"
    );
}

#[test]
fn test_activity_running_flag() {
    assert!(ActivityGroup::new(vec![tool("Bash", ToolState::Running, 0)]).running);
    assert!(!ActivityGroup::new(vec![tool("Bash", ToolState::Done, 0)]).running);
}

#[test]
fn test_activity_grouping_follows_verbosity() {
    let thinking = TranscriptBlock::Thinking(vec![StyledLine::plain("hmm")]);
    let blocks = vec![
        md("one"),
        tool("read", ToolState::Done, 0),
        thinking.clone(),
        md("two"),
        tool("Bash", ToolState::Done, 0),
    ];
    let low = group_activity(blocks.clone(), Verbosity::Low);
    assert_eq!(low.len(), 4, "{low:?}");
    assert_eq!(low[0], md("one"));
    let TranscriptBlock::Activity(first) = &low[1] else {
        panic!("activity expected: {low:?}");
    };
    assert_eq!(first.items.len(), 2);
    assert_eq!(low[2], md("two"));
    assert!(matches!(&low[3], TranscriptBlock::Activity(g) if g.items.len() == 1));

    let medium = group_activity(blocks.clone(), Verbosity::Medium);
    assert_eq!(medium.len(), 5, "{medium:?}");
    assert!(matches!(medium[1], TranscriptBlock::Tool { .. }));
    assert!(matches!(&medium[2], TranscriptBlock::Activity(g) if g.summary.thoughts == 1));

    assert_eq!(group_activity(blocks.clone(), Verbosity::High), blocks);
    assert_eq!(group_activity(Vec::new(), Verbosity::Low), Vec::new());
}

#[test]
fn test_activity_large_result_preview_truncated() {
    let lines: Vec<StyledLine> = (0..5_001)
        .map(|i| StyledLine::plain(&format!("line {i}")))
        .collect();
    let kept = truncate_result_lines(lines);
    assert_eq!(kept.len(), 1 + RESULT_PREVIEW_MAX_LINES + 1);
    assert_eq!(kept[0].text(), "line 0", "the header is kept");
    assert_eq!(
        kept.last().map(StyledLine::text).as_deref(),
        Some("  ... 4980 more lines")
    );
    let short: Vec<StyledLine> = (0..5).map(|i| StyledLine::plain(&format!("{i}"))).collect();
    assert_eq!(truncate_result_lines(short.clone()), short);
}

#[test]
fn test_activity_tool_blocks_carry_name_state_and_time() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Error,
                input: json!({"command": "false"}),
                output: None,
                error: Some("exit 1".to_string()),
                duration_ms: Some(40),
            }),
        }],
    );
    let blocks = message_to_blocks(&msg, &HashMap::<String, (String, u32, u32)>::new(), "/p");
    let [
        TranscriptBlock::Tool {
            name,
            state,
            duration_ms,
            ..
        },
    ] = blocks.as_slice()
    else {
        panic!("tool block expected: {blocks:?}");
    };
    assert_eq!(name, "Bash");
    assert_eq!(*state, ToolState::Failed);
    assert_eq!(*duration_ms, Some(40));
}
