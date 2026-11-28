//! `launch_leader_for_team`: the team is created with the leader's child
//! session as lead, and the parent (primary) session is never the lead.

use std::path::PathBuf;

use codemason_agent::session::history::resolve_team_context_for_session;
use codemason_agent::team::TeamStore;
use codemason_agent::team::leader::{
    cancel_leader_for_session, launch_leader_for_team, launch_leader_for_team_with_seed,
    leader_task_for_existing_team, leader_task_for_swarm,
};

#[path = "task_support/mod.rs"]
mod task_support;

/// A fresh scratch directory under `target/temp/` for one test.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/team_leader_launch")
        .join(format!("{name}-{}", uuid::Uuid::new_v4()));
    // A `.codemason` marker makes the scratch dir the project root, so the
    // project-local team is created here and not in an ancestor directory.
    std::fs::create_dir_all(dir.join(".codemason")).expect("create scratch dir");
    dir.canonicalize().expect("canonical scratch dir")
}

/// The team's lead is the leader session, and that session is a child of the
/// parent that launched it.
#[tokio::test]
async fn test_team_leader_launch_leader_is_lead() {
    let dir = scratch_dir("lead");
    let manager = task_support::manager(4);
    let parent = manager
        .processor()
        .session_manager
        .create_session(dir.clone())
        .expect("parent session");

    let launch = launch_leader_for_team(&manager, &parent.id, &dir, "alpha", "do x", None)
        .await
        .expect("launch leader");

    assert_eq!(launch.team_name, "alpha");
    let store = TeamStore::load_by_name("alpha", &dir).expect("team exists");
    assert_eq!(store.config.lead_session_id, launch.leader_session_id);
    let leader = manager
        .processor()
        .session_manager
        .get_session(&launch.leader_session_id)
        .expect("get session")
        .expect("leader session exists");
    assert_eq!(leader.parent_id.as_deref(), Some(parent.id.as_str()));

    let task = manager.get_task(&launch.task_id).await.expect("task entry");
    assert_eq!(task.agent_name, "team-leader");
    assert_eq!(task.child_session_id, launch.leader_session_id);
    assert!(task.background && !task.detached);
    assert!(
        manager.processor().team_managers.get("alpha").is_some(),
        "the team's manager is bound so teammates can be spawned at once"
    );
}

/// The parent session resolves no team context: it is not the lead.
#[tokio::test]
async fn test_team_leader_launch_parent_has_no_team_context() {
    let dir = scratch_dir("parent");
    let manager = task_support::manager(4);
    let parent = manager
        .processor()
        .session_manager
        .create_session(dir.clone())
        .expect("parent session");

    launch_leader_for_team(&manager, &parent.id, &dir, "beta", "do y", None)
        .await
        .expect("launch leader");

    assert!(resolve_team_context_for_session(&parent.id, &dir).is_none());
}

/// A launch whose team name is taken creates no leader task.
#[tokio::test]
async fn test_team_leader_launch_taken_name_spawns_nothing() {
    let dir = scratch_dir("taken");
    let manager = task_support::manager(4);
    let parent = manager
        .processor()
        .session_manager
        .create_session(dir.clone())
        .expect("parent session");
    TeamStore::create("gamma", "someone", &dir, true).expect("pre-existing team");

    let result = launch_leader_for_team(&manager, &parent.id, &dir, "gamma", "z", None).await;

    assert!(result.is_err(), "a taken name is refused");
    assert!(manager.list_agents(&parent.id).await.is_empty());
}

/// The existing-team task names the team and forbids re-creating it.
#[test]
fn test_team_leader_launch_existing_team_task_text() {
    let text = leader_task_for_existing_team("alpha", "review src/");
    assert!(text.contains("'alpha'"), "{text}");
    assert!(text.contains("team_create"), "{text}");
    assert!(text.contains("team_wait"), "{text}");
    assert!(text.contains("review src/"), "{text}");
    assert!(
        text.contains("team_cleanup"),
        "the leader cleans up: {text}"
    );
    assert!(!text.contains("Do not call `team_cleanup`"), "{text}");
}

/// The seed runs after the team exists (led by the leader session) and
/// before the leader task is registered, i.e. before its first step.
#[tokio::test]
async fn test_team_leader_launch_seed_runs_before_leader_starts() {
    let dir = scratch_dir("seed-order");
    let manager = task_support::manager(4);
    let parent = manager
        .processor()
        .session_manager
        .create_session(dir.clone())
        .expect("parent session");
    let manager_ref = &manager;
    let parent_id = parent.id.clone();
    let dir_ref = &dir;

    let launch = launch_leader_for_team_with_seed(
        &manager,
        &parent.id,
        &dir,
        "delta",
        "lead",
        None,
        |seed| async move {
            let store = TeamStore::load_by_name("delta", dir_ref)?;
            assert_eq!(store.config.lead_session_id, seed.leader_session_id);
            assert!(
                manager_ref.list_agents(&parent_id).await.is_empty(),
                "no leader task exists while seeding"
            );
            let mut store = store;
            store.add_member(codemason_agent::team::TeamMember::new(
                "alice", "tm-001", "general",
            ))?;
            Ok(())
        },
    )
    .await
    .expect("launch leader");

    let store = TeamStore::load_by_name("delta", &dir).expect("team exists");
    assert!(store.config.member_by_name("alice").is_some());
    assert!(manager.get_task(&launch.task_id).await.is_some());
}

/// A seeding error fails the launch: no task, no team, no leader session.
#[tokio::test]
async fn test_team_leader_launch_seed_error_creates_nothing() {
    let dir = scratch_dir("seed-error");
    let manager = task_support::manager(4);
    let parent = manager
        .processor()
        .session_manager
        .create_session(dir.clone())
        .expect("parent session");

    let result = launch_leader_for_team_with_seed(
        &manager,
        &parent.id,
        &dir,
        "epsilon",
        "lead",
        None,
        |_| std::future::ready(Err(anyhow::anyhow!("seed broke"))),
    )
    .await;

    let err = result.expect_err("seeding error fails the launch");
    assert!(format!("{err:#}").contains("seed broke"), "{err:#}");
    assert!(manager.list_agents(&parent.id).await.is_empty());
    assert!(TeamStore::load_by_name("epsilon", &dir).is_err());
    assert!(manager.processor().team_managers.get("epsilon").is_none());
    let has_child = manager
        .processor()
        .session_manager
        .list_sessions()
        .expect("list sessions")
        .into_iter()
        .any(|s| s.parent_id.as_deref() == Some(parent.id.as_str()));
    assert!(!has_child, "the leader session is deleted");
}

/// The swarm leader is passive, synthesises the answer, and cleans up.
#[test]
fn test_team_leader_launch_swarm_task_text() {
    let text = leader_task_for_swarm("swarm-1", "Harden auth");
    assert!(text.contains("'swarm-1'"), "{text}");
    assert!(text.contains("Harden auth"), "{text}");
    assert!(
        text.contains("do NOT call `team_shutdown_teammate`"),
        "{text}"
    );
    assert!(text.contains("Blocked"), "{text}");
    assert!(text.contains("team_task_list"), "{text}");
    assert!(
        text.contains(codemason_agent::team::SYNTHESIS_SYSTEM_PROMPT),
        "{text}"
    );
    assert!(text.contains("team_cleanup"), "{text}");
}

/// `cancel_leader_for_session` cancels the running task owning the session.
#[tokio::test]
async fn test_team_leader_launch_cancel_by_lead_session() {
    let manager = task_support::manager(4);
    let mut entry = task_support::entry_full(
        "lead-1",
        codemason_agent::task::TaskStatus::Running,
        "parent",
        true,
        false,
    );
    entry.child_session_id = "leader-sess".to_string();
    let flag = manager.insert_task_for_test(entry);

    assert!(!cancel_leader_for_session(&manager, "other-sess").await);
    assert!(cancel_leader_for_session(&manager, "leader-sess").await);
    assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
}
