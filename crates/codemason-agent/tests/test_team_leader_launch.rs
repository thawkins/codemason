//! `launch_leader_for_team`: the team is created with the leader's child
//! session as lead, and the parent (primary) session is never the lead.

use std::path::PathBuf;

use codemason_agent::session::history::resolve_team_context_for_session;
use codemason_agent::team::TeamStore;
use codemason_agent::team::leader::{launch_leader_for_team, leader_task_for_existing_team};

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
}
