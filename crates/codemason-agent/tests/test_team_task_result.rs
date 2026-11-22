//! A team task's `result`: stored by `team_task_complete`, kept across a
//! completion without one, omitted from `tasks.json` when absent, and cut at
//! `MAX_TASK_RESULT_BYTES`.
//!
//! Every fixture lives in its own `tempfile::tempdir()`, so nothing is
//! written into the repository.

use std::sync::Arc;

use codemason_agent::team::{Task, TaskList, TaskStatus, TaskStore, TeamStore};
use codemason_agent::tool::team_task_complete::{
    MAX_TASK_RESULT_BYTES, TeamTaskCompleteTool, cap_task_result,
};
use codemason_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::json;

const TEAM: &str = "result-team";
const OWNER: &str = "tm-001";

/// A workspace with one team holding task `t1`, in progress and owned by
/// [`OWNER`]. Returns the tempdir (keep it alive), the workspace and the store.
fn team_with_claimed_task() -> (tempfile::TempDir, std::path::PathBuf, TaskStore) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();
    std::fs::create_dir_all(workspace.join(".codemason")).expect("create .codemason");
    let store = TeamStore::create(TEAM, "lead", &workspace, true).expect("create team");
    let tasks = TaskStore::open(&store.dir).expect("open task store");
    let mut task = Task::new("t1", "work t1");
    task.status = TaskStatus::InProgress;
    task.assigned_to = Some(OWNER.to_string());
    tasks
        .write_through(|list| list.tasks.push(task))
        .expect("seed task");
    (tmp, workspace, tasks)
}

/// A store-fallback `ToolContext` (no manager) acting as [`OWNER`].
fn tool_context(workspace: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: OWNER.to_string(),
        working_dir: workspace.to_path_buf(),
        event_bus: Arc::new(codemason_agent::event::EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: TEAM.to_string(),
            agent_id: OWNER.to_string(),
            is_lead: false,
        })),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

fn stored_result(store: &TaskStore) -> Option<String> {
    store
        .read()
        .expect("read tasks")
        .tasks
        .into_iter()
        .find(|t| t.id == "t1")
        .and_then(|t| t.result)
}

#[tokio::test]
async fn test_task_result_tool_stores_result() {
    let (_tmp, workspace, store) = team_with_claimed_task();
    let ctx = tool_context(&workspace);
    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": TEAM, "task_id": "t1", "result": "Found 3 handlers"}),
            &ctx,
        )
        .await
        .expect("complete");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["completed"], true);
    assert_eq!(meta["result_recorded"], true);
    assert_eq!(meta["result_truncated"], false);
    assert_eq!(stored_result(&store).as_deref(), Some("Found 3 handlers"));
}

#[tokio::test]
async fn test_task_result_tool_cuts_long_result() {
    let (_tmp, workspace, store) = team_with_claimed_task();
    let ctx = tool_context(&workspace);
    let long = "x".repeat(MAX_TASK_RESULT_BYTES + 100);
    let out = TeamTaskCompleteTool
        .execute(
            json!({"team_name": TEAM, "task_id": "t1", "result": long}),
            &ctx,
        )
        .await
        .expect("complete");
    assert!(out.content.contains("was cut"));
    assert_eq!(out.metadata.expect("metadata")["result_truncated"], true);
    assert_eq!(
        stored_result(&store).map(|r| r.len()),
        Some(MAX_TASK_RESULT_BYTES)
    );
}

#[tokio::test]
async fn test_task_result_completion_without_result_keeps_earlier_one() {
    let (_tmp, _workspace, store) = team_with_claimed_task();
    store
        .complete_with_result("t1", OWNER, Some("first"))
        .await
        .expect("complete with result");
    // An idempotent re-completion without a result must not erase it.
    store.complete("t1", OWNER).await.expect("re-complete");
    assert_eq!(stored_result(&store).as_deref(), Some("first"));
}

#[test]
fn test_task_result_absent_is_not_serialized_and_old_lists_load() {
    let task = Task::new("t1", "work");
    let json = serde_json::to_value(&task).expect("serialize");
    assert!(json.get("result").is_none());

    // A task written before the field existed still loads.
    let list: TaskList = serde_json::from_value(json!({
        "tasks": [{
            "id": "t1", "title": "old", "status": "completed", "assigned_to": null,
            "created_at": "2026-01-01T00:00:00Z"
        }]
    }))
    .expect("old task list loads");
    assert_eq!(list.tasks[0].result, None);
}

#[test]
fn test_task_result_cap_keeps_char_boundary() {
    let (short, cut) = cap_task_result("short");
    assert_eq!((short, cut), ("short", false));
    let long = "\u{e9}".repeat(MAX_TASK_RESULT_BYTES);
    let (kept, cut) = cap_task_result(&long);
    assert!(cut);
    assert!(kept.len() <= MAX_TASK_RESULT_BYTES);
    assert!(long.is_char_boundary(kept.len()));
}
