//! `/team create` goes through a team leader: the primary session is never
//! the team's lead; a `team-leader` background task is, one per team.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::task::AgentManager;
use codemason_agent::team::{TeamStatus, TeamStore};
use codemason_app_core::app::App;
use support::make_app;

/// A fresh workspace under `target/temp/test_slash_team_create_leader/<test>`.
fn workspace(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_slash_team_create_leader")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".codemason")).expect("workspace");
    dir.canonicalize().expect("canonical workspace")
}

/// An app rooted at `ws` with a real primary session and an agent manager.
fn app_in(ws: &std::path::Path) -> (App, Arc<AgentManager>) {
    let mut app = make_app();
    app.cwd_path = ws.to_path_buf();
    app.cwd = ws.display().to_string();
    let manager = support::attach_leader_runtime(&mut app);
    (app, manager)
}

#[tokio::test]
async fn test_slash_team_create_leader_spawns_leader() {
    let ws = workspace("spawns_leader");
    let (mut app, manager) = app_in(&ws);
    let primary = app.session.session_id.clone().expect("primary");

    app.execute_slash_command("/team create code-review alpha x")
        .await;

    let tasks = manager.list_agents(&primary).await;
    assert_eq!(tasks.len(), 1, "one leader task: {tasks:?}");
    assert_eq!(tasks[0].agent_name, "team-leader");
    let store = TeamStore::load_by_name("alpha", &ws).expect("team alpha");
    assert_eq!(store.config.lead_session_id, tasks[0].child_session_id);
    assert_ne!(store.config.lead_session_id, primary);
}

#[tokio::test]
async fn test_slash_team_create_leader_two_teams() {
    let ws = workspace("two_teams");
    let (mut app, manager) = app_in(&ws);
    let primary = app.session.session_id.clone().expect("primary");

    app.execute_slash_command("/team create code-review alpha x")
        .await;
    app.execute_slash_command("/team create code-review beta y")
        .await;

    let tasks = manager.list_agents(&primary).await;
    assert_eq!(tasks.len(), 2, "two leader tasks: {tasks:?}");
    assert!(tasks.iter().all(|t| t.agent_name == "team-leader"));
    for name in ["alpha", "beta"] {
        let store = TeamStore::load_by_name(name, &ws).expect("team exists");
        assert_eq!(store.config.status, TeamStatus::Active);
        assert!(
            tasks
                .iter()
                .any(|t| t.child_session_id == store.config.lead_session_id),
            "{name}'s lead is one of the leader tasks"
        );
        assert!(
            app.session_processor.team_managers.get(name).is_some(),
            "{name}'s manager is bound"
        );
    }
}
