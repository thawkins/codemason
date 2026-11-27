//! `team_create` is leader-only (except `seed_only`), team names are unique,
//! and a team leader cannot call `new_agent`.

use std::sync::Arc;

use codemason_agent::agent::AgentRole;
use codemason_agent::event::EventBus;
use codemason_agent::tool::{Tool, ToolContext, create_default_registry};
use serde_json::json;

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".codemason")).expect("create .codemason");
    dir
}

fn ctx_in(dir: &std::path::Path, role: AgentRole) -> ToolContext {
    ToolContext {
        session_id: "sess-gate".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: vec![dir.to_path_buf()],
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        agent_role: role,
        permission_checker: None,
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

fn tool(name: &str) -> Arc<dyn Tool> {
    create_default_registry()
        .get(name)
        .expect("tool registered")
}

#[tokio::test]
async fn test_team_create_leader_gate_refuses_standard_role() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), AgentRole::Standard);
    let err = tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "gated" }),
            &ctx,
        )
        .await
        .expect_err("a standard agent may not create a team");
    assert!(err.to_string().contains("not_team_leader"), "{err}");
    assert!(!ws.path().join(".codemason/teams/gated").exists());
}

#[tokio::test]
async fn test_team_create_leader_gate_allows_leader() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), AgentRole::TeamLeader);
    tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "led" }),
            &ctx,
        )
        .await
        .expect("a leader may create a team");
    assert!(ws.path().join(".codemason/teams/led").is_dir());
}

#[tokio::test]
async fn test_team_create_leader_gate_seed_only_exempt() {
    let ws = workspace();
    let leader = ctx_in(ws.path(), AgentRole::TeamLeader);
    tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "seeded" }),
            &leader,
        )
        .await
        .expect("create");
    let standard = ctx_in(ws.path(), AgentRole::Standard);
    tool("team_create")
        .execute(
            json!({
                "blueprint": "code-review", "context": "x", "name": "seeded", "seed_only": true
            }),
            &standard,
        )
        .await
        .expect("seed_only is exempt from the leader gate");
}

#[tokio::test]
async fn test_team_create_leader_gate_refuses_existing_active_name() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), AgentRole::TeamLeader);
    let input = json!({ "blueprint": "code-review", "context": "x", "name": "alpha" });
    tool("team_create")
        .execute(input.clone(), &ctx)
        .await
        .expect("first create");
    let err = tool("team_create")
        .execute(input, &ctx)
        .await
        .expect_err("a second create of an active team name is refused");
    assert!(err.to_string().contains("alpha"), "{err}");
}

#[tokio::test]
async fn test_team_create_leader_gate_concurrent_generated_names_distinct() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), AgentRole::TeamLeader);
    let input = json!({ "blueprint": "code-review", "context": "x" });
    let t = tool("team_create");
    let (a, b) = tokio::join!(
        t.execute(input.clone(), &ctx),
        t.execute(input.clone(), &ctx)
    );
    let name = |o: codemason_agent::tool::ToolOutput| {
        o.metadata.expect("metadata")["team_name"]
            .as_str()
            .expect("name")
            .to_string()
    };
    let (a, b) = (name(a.expect("first")), name(b.expect("second")));
    assert_ne!(a, b);
}

#[tokio::test]
async fn test_team_create_leader_gate_leader_cannot_call_new_agent() {
    let ws = workspace();
    let ctx = ctx_in(ws.path(), AgentRole::TeamLeader);
    let err = tool("new_agent")
        .execute(json!({ "agent": "general", "task": "do it" }), &ctx)
        .await
        .expect_err("a team leader may not spawn sub-agents");
    assert!(err.to_string().contains("leader_cannot_spawn"), "{err}");
}
