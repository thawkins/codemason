//! Tests for `codemason_app_core::permission_preview`: a permission request
//! that carries its tool call is previewed from exactly that call's input.

use std::path::PathBuf;

use codemason_agent::event::Event;
use codemason_app_core::permission_preview::{PermissionPreview, permission_preview};
use codemason_types::event::PermissionToolCall;
use serde_json::{Value, json};

mod support;

/// A fresh, empty scratch directory under the git-ignored `target/temp`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_permission_preview")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The request the App queues for a `PermissionRequested` carrying `call`.
async fn request(
    description: &str,
    call: Option<(&str, Value)>,
) -> codemason_config::permission::PermissionRequest {
    let mut app = support::make_app();
    let session = app
        .session
        .session_id
        .clone()
        .unwrap_or_else(|| "s".to_string());
    app.session.session_id = Some(session.clone());
    app.handle_event(Event::PermissionRequested {
        session_id: session,
        request_id: "req".to_string(),
        permission: "edit".to_string(),
        description: description.to_string(),
        options: Vec::new(),
        tool_call: call.map(|(name, input)| PermissionToolCall {
            id: "call-1".to_string(),
            name: name.to_string(),
            input,
        }),
    })
    .await;
    let queued = app
        .session
        .permission_queue
        .front()
        .cloned()
        .expect("request queued");
    assert_eq!(
        queued.tool_call_id.is_some(),
        queued
            .metadata
            .get("tool_call")
            .is_some_and(|v| !v.is_null())
    );
    queued
}

#[tokio::test]
async fn test_permission_preview_edit_shows_the_effective_diff() {
    // The request carries the input the call will run with (after hooks),
    // whatever the transcript says.
    let req = request(
        "Edit: tool:Edit",
        Some((
            "Edit",
            json!({"file_path": "a.rs", "old_string": "x\n", "new_string": "y\n"}),
        )),
    )
    .await;
    assert_eq!(req.tool_call_id.as_deref(), Some("call-1"));
    let Some(PermissionPreview::Diffs(diffs)) = permission_preview(&req, &scratch("edit")) else {
        panic!("diff preview expected");
    };
    assert_eq!(diffs[0].path, "a.rs");
    assert_eq!((diffs[0].added, diffs[0].removed), (1, 1));
}

#[tokio::test]
async fn test_permission_preview_bash_shows_the_full_command() {
    // The resource of a Bash prompt is only the command name.
    let command = "cargo build --release && cargo nextest run --workspace";
    let req = request("Bash: cargo", Some(("Bash", json!({ "command": command })))).await;
    assert_eq!(
        permission_preview(&req, &scratch("bash")),
        Some(PermissionPreview::Command(command.to_string()))
    );
}

#[tokio::test]
async fn test_permission_preview_write_is_diffed_against_the_current_file() {
    let dir = scratch("write");
    std::fs::write(dir.join("old.txt"), "keep\ndrop\n").expect("write");
    let req = request(
        "Write: old.txt",
        Some(("Write", json!({"path": "old.txt", "content": "keep\n"}))),
    )
    .await;
    let Some(PermissionPreview::Diffs(diffs)) = permission_preview(&req, &dir) else {
        panic!("diff preview expected");
    };
    assert_eq!((diffs[0].added, diffs[0].removed), (0, 1));
    assert!(!diffs[0].whole_file);

    // A new file: every line is added, and that is the whole change.
    let req = request(
        "Write: new.txt",
        Some(("Write", json!({"path": "new.txt", "content": "a\nb\n"}))),
    )
    .await;
    let Some(PermissionPreview::Diffs(diffs)) = permission_preview(&req, &dir) else {
        panic!("diff preview expected");
    };
    assert_eq!((diffs[0].added, diffs[0].removed), (2, 0));
    assert!(!diffs[0].whole_file);
}

#[tokio::test]
async fn test_permission_preview_needs_the_call() {
    let dir = scratch("none");
    assert_eq!(
        permission_preview(&request("Bash: ls", None).await, &dir),
        None
    );
    let read = request("read: a", Some(("read", json!({"path": "a"})))).await;
    assert_eq!(permission_preview(&read, &dir), None);
}
