//! Tests for `codemason_app_core::permission_preview`: finding the call a
//! permission request is about and previewing it.

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_app_core::permission_preview::{PermissionPreview, permission_preview};
use serde_json::{Value, json};

fn call(tool: &str, status: ToolCallStatus, input: Value) -> MessagePart {
    MessagePart::ToolCall {
        tool: tool.to_string(),
        call_id: format!("c-{tool}"),
        state: Box::new(ToolCallState {
            status,
            input,
            output: None,
            error: None,
            duration_ms: None,
        }),
    }
}

fn messages(parts: Vec<MessagePart>) -> Vec<Message> {
    vec![Message::new("s", Role::Assistant, parts)]
}

#[test]
fn test_permission_preview_edit_shows_its_diff() {
    let msgs = messages(vec![call(
        "Edit",
        ToolCallStatus::Running,
        json!({"file_path": "a.rs", "old_string": "x\n", "new_string": "y\n"}),
    )]);
    // `Edit` has no `path`, so its resource is `tool:Edit`.
    let Some(PermissionPreview::Diffs(diffs)) = permission_preview(&msgs, "Edit: tool:Edit") else {
        panic!("diff preview expected");
    };
    assert_eq!(diffs[0].path, "a.rs");
    assert_eq!((diffs[0].added, diffs[0].removed), (1, 1));
}

#[test]
fn test_permission_preview_bash_shows_the_full_command() {
    let command = "cargo build && \\\n  cargo test --workspace";
    let msgs = messages(vec![call(
        "Bash",
        ToolCallStatus::Pending,
        json!({ "command": command }),
    )]);
    assert_eq!(
        permission_preview(&msgs, &format!("Bash: {command}")),
        Some(PermissionPreview::Command(command.to_string()))
    );
    // A different command is a different request.
    assert_eq!(permission_preview(&msgs, "Bash: ls"), None);
}

#[test]
fn test_permission_preview_needs_one_unfinished_match() {
    let input = json!({"command": "ls"});
    // A finished call is not what is being asked about.
    let done = messages(vec![call("Bash", ToolCallStatus::Completed, input.clone())]);
    assert_eq!(permission_preview(&done, "Bash: ls"), None);
    // Two unfinished calls that fit: ambiguous, so no preview.
    let mut both = messages(vec![call("Bash", ToolCallStatus::Running, input.clone())]);
    both.extend(messages(vec![call("Bash", ToolCallStatus::Running, input)]));
    assert_eq!(permission_preview(&both, "Bash: ls"), None);
    // Other tools, and a description without a resource, have none.
    let read = messages(vec![call(
        "read",
        ToolCallStatus::Running,
        json!({"path": "a"}),
    )]);
    assert_eq!(permission_preview(&read, "read: a"), None);
    assert_eq!(permission_preview(&read, "read"), None);
}
