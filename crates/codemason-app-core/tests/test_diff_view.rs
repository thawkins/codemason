//! Tests for `codemason_app_core::diff_view`: line diffs of edit tool calls.

use std::collections::HashMap;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::activity::group_activity;
use codemason_app_core::diff_view::{
    DIFF_MAX_LINES, DiffLineKind, diffs_from_tool_input, file_diff,
};
use codemason_app_core::message_blocks::{TranscriptBlock, message_to_blocks};
use codemason_app_core::transcript_prefs::Verbosity;
use serde_json::json;

fn numbered(n: usize) -> String {
    (1..=n).map(|i| format!("line {i}\n")).collect()
}

#[test]
fn test_diff_view_one_line_change() {
    let old = numbered(10);
    let new = old.replace("line 6\n", "line six\n");
    let diff = file_diff("src/a.rs", &old, &new);
    assert_eq!(diff.path, "src/a.rs");
    assert_eq!((diff.added, diff.removed), (1, 1));
    assert!(!diff.truncated);
    assert_eq!(diff.hunks.len(), 1);
    let hunk = &diff.hunks[0];
    assert_eq!(hunk.header, "@@ -3,7 +3,7 @@");
    let removed = hunk
        .lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Removed)
        .expect("removed line");
    assert_eq!((removed.old_no, removed.new_no), (Some(6), None));
    assert_eq!(removed.text, "line 6");
    let added = hunk
        .lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Added)
        .expect("added line");
    assert_eq!((added.old_no, added.new_no), (None, Some(6)));
    assert_eq!(added.text, "line six");
    let first = &hunk.lines[0];
    assert_eq!(
        (first.kind, first.old_no, first.new_no),
        (DiffLineKind::Context, Some(3), Some(3))
    );
}

#[test]
fn test_diff_view_from_tool_inputs() {
    let edit = diffs_from_tool_input(
        "Edit",
        &json!({"file_path": "a.rs", "old_string": "x\n", "new_string": "y\n"}),
    );
    assert_eq!(edit.len(), 1);
    assert_eq!((edit[0].added, edit[0].removed), (1, 1));
    // The alternate keys and a retired spelling work too.
    let alt = diffs_from_tool_input(
        "edit",
        &json!({"path": "b.rs", "old_str": "", "new_str": "z"}),
    );
    assert_eq!(alt[0].path, "b.rs");

    let write = diffs_from_tool_input("Write", &json!({"path": "c.txt", "content": "1\n2\n3\n"}));
    assert_eq!(write.len(), 1);
    let lines = &write[0].hunks[0].lines;
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|l| l.kind == DiffLineKind::Added));

    let multi = diffs_from_tool_input(
        "multi_edit",
        &json!({"edits": [
            {"file_path": "d.rs", "old_string": "a", "new_string": "b"},
            {"file_path": "e.rs", "old_string": "c", "new_string": "d"},
            {"old_string": "no path"}
        ]}),
    );
    assert_eq!(
        multi.iter().map(|d| d.path.as_str()).collect::<Vec<_>>(),
        ["d.rs", "e.rs"]
    );

    assert_eq!(
        diffs_from_tool_input("read", &json!({"path": "a"})),
        [] as [codemason_app_core::diff_view::FileDiff; 0]
    );
    assert_eq!(
        diffs_from_tool_input("Edit", &json!({})),
        [] as [codemason_app_core::diff_view::FileDiff; 0]
    );
}

#[test]
fn test_diff_view_truncates_large_diff() {
    let old = numbered(10_000);
    let new: String = (1..=10_000).map(|i| format!("new {i}\n")).collect();
    let diff = file_diff("big.txt", &old, &new);
    let lines: usize = diff.hunks.iter().map(|h| h.lines.len()).sum();
    assert!(lines <= DIFF_MAX_LINES, "{lines}");
    assert!(diff.truncated);
    assert_eq!((diff.added, diff.removed), (10_000, 10_000));
}

fn edit_message(status: ToolCallStatus) -> Message {
    Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Edit".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status,
                input: json!({"file_path": "a.rs", "old_string": "x", "new_string": "y"}),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    )
}

#[test]
fn test_diff_view_blocks_follow_finished_edits() {
    let map = HashMap::<String, (String, u32, u32)>::new();
    let done = message_to_blocks(&edit_message(ToolCallStatus::Completed), &map, "/p");
    assert!(matches!(
        done.as_slice(),
        [TranscriptBlock::Tool { .. }, TranscriptBlock::Diff(_)]
    ));
    let running = message_to_blocks(&edit_message(ToolCallStatus::Running), &map, "/p");
    assert!(matches!(running.as_slice(), [TranscriptBlock::Tool { .. }]));
    let failed = message_to_blocks(&edit_message(ToolCallStatus::Error), &map, "/p");
    assert!(matches!(failed.as_slice(), [TranscriptBlock::Tool { .. }]));

    // Folded with its call at low, standalone at medium and high.
    let low = group_activity(done.clone(), Verbosity::Low);
    assert!(matches!(low.as_slice(), [TranscriptBlock::Activity(g)] if g.items.len() == 2));
    let medium = group_activity(done.clone(), Verbosity::Medium);
    assert!(matches!(
        medium.as_slice(),
        [TranscriptBlock::Tool { .. }, TranscriptBlock::Diff(_)]
    ));
    assert!(
        done[1].plain_text().contains("\n-x\n+y"),
        "{}",
        done[1].plain_text()
    );
}
