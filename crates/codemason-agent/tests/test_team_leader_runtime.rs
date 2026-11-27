//! Leader runtime: a team leader's background run is not killed by the
//! configured background cap, and a user message interrupts a blocked
//! `team_wait`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use codemason_agent::agent::AgentRole;
use codemason_agent::event::EventBus;
use codemason_agent::task::await_background_inner;
use codemason_agent::team::{MemberStatus, TeamMember, TeamStore};
use codemason_agent::tool::{Tool, ToolContext};
use serde_json::json;

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_workspace;

#[path = "task_support/mod.rs"]
mod task_support;

/// A background inner future that finishes after `secs` seconds.
fn sleeping_inner(secs: u64) -> tokio::task::JoinHandle<anyhow::Result<String>> {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        Ok("done".to_string())
    })
}

/// A `ToolContext` for a lead session, with an optional input-pending signal.
fn make_ctx(dir: &std::path::Path, notify: Option<Arc<tokio::sync::Notify>>) -> ToolContext {
    ToolContext {
        session_id: "lead-sess".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        agent_role: AgentRole::TeamLeader,
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        user_input_pending: notify,
    }
}

/// Seed a team with one busy teammate.
fn seed_busy_team(dir: &std::path::Path, team: &str) {
    let mut store = TeamStore::create(team, "lead-sess", dir, true).expect("create team");
    let mut member = TeamMember::new("runner", "tm-001", "general");
    member.status = MemberStatus::Working;
    store.add_member(member).expect("add_member");
}

/// A leader outlives a 1 s background cap; a standard agent does not.
#[tokio::test]
async fn test_team_leader_runtime_no_background_cap() {
    let flag = AtomicBool::new(false);
    let (leader, _) =
        await_background_inner(sleeping_inner(2), AgentRole::TeamLeader, None, 1, &flag).await;
    assert!(leader.is_ok(), "leader must not hit the cap: {leader:?}");

    let (standard, _) =
        await_background_inner(sleeping_inner(2), AgentRole::Standard, None, 1, &flag).await;
    assert!(standard.is_err(), "standard agent must hit the cap");
}

/// A per-call bound still applies to a leader.
#[tokio::test]
async fn test_team_leader_runtime_per_call_bound_applies() {
    let flag = AtomicBool::new(false);
    let (res, hit) = await_background_inner(
        sleeping_inner(3),
        AgentRole::TeamLeader,
        Some(1),
        100,
        &flag,
    )
    .await;
    assert!(res.is_err());
    assert!(hit, "per-call bound expiry must be reported");
}

/// A manager whose inbox for `lead-sess` is open, bound into a context.
fn make_ctx_with_inbox(
    dir: &std::path::Path,
) -> (ToolContext, Arc<codemason_agent::task::AgentManager>) {
    let mgr = Arc::new(codemason_agent::task::AgentManager::new(
        Arc::new(EventBus::new(16)),
        task_support::test_processor(),
        2,
        300,
    ));
    mgr.open_inbox_for_test("lead-sess");
    let notify = mgr.inbox_notify("lead-sess").expect("inbox notify");
    let mut ctx = make_ctx(dir, Some(notify));
    ctx.agent_manager = Some(Arc::clone(&mgr));
    (ctx, mgr)
}

/// `notify_one()` wakes a blocked `team_wait`, which reports `interrupted`.
#[tokio::test]
async fn test_team_leader_runtime_user_input_interrupts_wait() {
    let (_tmp, dir) = setup_workspace();
    seed_busy_team(&dir, "leader-team");
    let (ctx, mgr) = make_ctx_with_inbox(&dir);

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        mgr.send_to_session("lead-sess", "steer").expect("queue");
    });

    let start = Instant::now();
    let out = codemason_agent::tool::team_wait::TeamWaitTool
        .execute(
            json!({ "team_name": "leader-team", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .expect("team_wait");
    assert!(start.elapsed() < Duration::from_secs(1));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["interrupted"], json!(true));
    assert_eq!(meta["success"], json!(false));
    assert!(
        out.content.starts_with(
            "Wait interrupted: a user message is pending. Handle it, then call team_wait again."
        ),
        "{}",
        out.content
    );
}

/// A stale permit (message already delivered, inbox empty) must not
/// interrupt: the wait runs to its timeout.
#[tokio::test]
async fn test_team_leader_runtime_stale_notify_does_not_interrupt() {
    let (_tmp, dir) = setup_workspace();
    seed_busy_team(&dir, "stale-team");
    let (ctx, mgr) = make_ctx_with_inbox(&dir);
    mgr.send_to_session("lead-sess", "handled").expect("queue");
    assert_eq!(mgr.take_inbox("lead-sess").len(), 1);

    let out = codemason_agent::tool::team_wait::TeamWaitTool
        .execute(
            json!({ "team_name": "stale-team", "timeout_secs": 1 }),
            &ctx,
        )
        .await
        .expect("team_wait");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["timed_out"], json!(true));
    assert_ne!(meta.get("interrupted"), Some(&json!(true)));
}

/// Without input the wait runs to its timeout as before.
#[tokio::test]
async fn test_team_leader_runtime_wait_unaffected_without_input() {
    let (_tmp, dir) = setup_workspace();
    seed_busy_team(&dir, "quiet-team");
    let ctx = make_ctx(&dir, Some(Arc::new(tokio::sync::Notify::new())));
    let out = codemason_agent::tool::team_wait::TeamWaitTool
        .execute(
            json!({ "team_name": "quiet-team", "timeout_secs": 1 }),
            &ctx,
        )
        .await
        .expect("team_wait");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["timed_out"], json!(true));
    assert_ne!(meta.get("interrupted"), Some(&json!(true)));
}
