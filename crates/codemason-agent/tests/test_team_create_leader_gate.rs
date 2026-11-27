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
    ctx_with_session(dir, role, "sess-gate")
}

fn ctx_with_session(dir: &std::path::Path, role: AgentRole, session: &str) -> ToolContext {
    ToolContext {
        session_id: session.to_string(),
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

async fn create_seeded(ws: &std::path::Path) {
    tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "seeded" }),
            &ctx_in(ws, AgentRole::TeamLeader),
        )
        .await
        .expect("create");
}

#[tokio::test]
async fn test_team_create_leader_gate_seed_only_exempt() {
    let ws = workspace();
    create_seeded(ws.path()).await;
    let standard = ctx_in(ws.path(), AgentRole::Standard);
    tool("team_create")
        .execute(
            json!({
                "blueprint": "code-review", "context": "x", "name": "seeded", "seed_only": true
            }),
            &standard,
        )
        .await
        .expect("seed_only from the lead session is exempt from the role gate");
}

#[tokio::test]
async fn test_team_create_leader_gate_seed_only_refused_for_other_session() {
    let ws = workspace();
    create_seeded(ws.path()).await;
    let other = ctx_with_session(ws.path(), AgentRole::TeamLeader, "someone-else");
    let err = tool("team_create")
        .execute(
            json!({
                "blueprint": "code-review", "context": "x", "name": "seeded", "seed_only": true
            }),
            &other,
        )
        .await
        .expect_err("a non-lead session may not seed the team");
    assert!(err.to_string().contains("not_team_leader"), "{err}");
}

#[tokio::test]
async fn test_team_create_leader_gate_refuses_existing_inactive_name() {
    let ws = workspace();
    create_seeded(ws.path()).await;
    let mut store =
        codemason_agent::team::TeamStore::load_by_name("seeded", ws.path()).expect("load");
    store.config.status = codemason_agent::team::TeamStatus::Completed;
    store.save().expect("save");
    let err = tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "seeded" }),
            &ctx_in(ws.path(), AgentRole::TeamLeader),
        )
        .await
        .expect_err("an inactive team's name is still taken");
    assert!(err.to_string().contains("seeded"), "{err}");
}

#[tokio::test]
async fn test_team_create_leader_gate_refuses_configless_dir_name() {
    let ws = workspace();
    std::fs::create_dir_all(ws.path().join(".codemason/teams/halfmade")).expect("dir");
    let err = tool("team_create")
        .execute(
            json!({ "blueprint": "code-review", "context": "x", "name": "halfmade" }),
            &ctx_in(ws.path(), AgentRole::TeamLeader),
        )
        .await
        .expect_err("a config-less directory name is refused");
    assert!(err.to_string().contains("halfmade"), "{err}");
    assert!(
        !ws.path()
            .join(".codemason/teams/halfmade/config.json")
            .exists()
    );
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

/// Eight real parallel creates of one blueprint. Rounds repeat until the
/// creates share a timestamp base (so the `-N` suffix path runs under
/// contention); every round must still produce distinct names.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_team_create_leader_gate_concurrent_generated_names_distinct() {
    let mut saw_suffix = false;
    for _ in 0..20 {
        let ws = workspace();
        let barrier = Arc::new(tokio::sync::Barrier::new(8));
        let mut handles = Vec::new();
        for _ in 0..8 {
            let dir = ws.path().to_path_buf();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                let ctx = ctx_in(&dir, AgentRole::TeamLeader);
                barrier.wait().await;
                tool("team_create")
                    .execute(json!({ "blueprint": "code-review", "context": "x" }), &ctx)
                    .await
            }));
        }
        let mut names = Vec::new();
        for h in handles {
            let out = h.await.expect("join").expect("create");
            names.push(
                out.metadata.expect("metadata")["team_name"]
                    .as_str()
                    .expect("name")
                    .to_string(),
            );
        }
        let distinct: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(distinct.len(), names.len(), "names collided: {names:?}");
        // A real suffixed name is `<other>-<k>` (k >= 2) where `<other>` is
        // itself one of the created names.
        let set: std::collections::HashSet<&str> = names.iter().map(String::as_str).collect();
        if names.iter().any(|n| {
            n.rsplit_once('-').is_some_and(|(base, k)| {
                k.parse::<u32>().is_ok_and(|k| k >= 2) && set.contains(base)
            })
        }) {
            saw_suffix = true;
            break;
        }
    }
    assert!(saw_suffix, "no round exercised the -N suffix path");
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
