//! `/team create` goes through a team leader: the primary session is never
//! the team's lead; a `team-leader` background task is, one per team. The
//! blueprint is seeded before the leader starts, an uninstalled blueprint is
//! refused before anything is created, and closing a team cancels its leader.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::Utc;
use codemason_agent::task::{AgentManager, ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{TeamStatus, TeamStore};
use codemason_app_core::app::App;
use support::make_app;

/// A fresh workspace under `target/temp/test_slash_team_create_leader/<test>`
/// with the `bp1` (seeds nothing) and `pair` (seeds `alice`) blueprints.
fn workspace(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_slash_team_create_leader")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".codemason")).expect("workspace");
    let dir = dir.canonicalize().expect("canonical workspace");
    support::install_test_blueprint(&dir, "bp1", &[]);
    support::install_test_blueprint(&dir, "pair", &["alice"]);
    dir
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

    app.execute_slash_command("/team create bp1 alpha x").await;

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

    app.execute_slash_command("/team create bp1 alpha x").await;
    app.execute_slash_command("/team create bp1 beta y").await;

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

/// A blueprint that is not installed is refused before any leader or team
/// exists.
#[tokio::test]
async fn test_slash_team_create_leader_uninstalled_blueprint_creates_nothing() {
    let ws = workspace("uninstalled");
    let (mut app, manager) = app_in(&ws);
    let primary = app.session.session_id.clone().expect("primary");

    app.execute_slash_command("/team create no-such-bp alpha x")
        .await;

    assert!(
        app.status
            .contains("blueprint 'no-such-bp' is not installed"),
        "status: {}",
        app.status
    );
    assert!(manager.list_agents(&primary).await.is_empty());
    assert!(TeamStore::load_by_name("alpha", &ws).is_err());
}

/// The blueprint's teammates are recorded by the time the command returns:
/// the seed ran inside the launch, before the leader's first step.
#[tokio::test]
async fn test_slash_team_create_leader_seeds_before_leader_starts() {
    let ws = workspace("seeded");
    let (mut app, manager) = app_in(&ws);
    let primary = app.session.session_id.clone().expect("primary");
    // A blueprint seed only runs tools the session already allows.
    *app.session_processor.permission_checker.write() =
        codemason_agent::permission::PermissionChecker::new(vec![
            codemason_config::permission::PermissionRule {
                permission: codemason_config::permission::Permission::Custom(
                    "team:manage".to_string(),
                ),
                pattern: Some("**".to_string()),
                action: codemason_config::permission::PermissionAction::Allow,
            },
        ]);

    app.execute_slash_command("/team create pair alpha x").await;

    let store = TeamStore::load_by_name("alpha", &ws).expect("team alpha");
    assert!(
        store.config.member_by_name("alice").is_some(),
        "the blueprint teammate is seeded: {:?}",
        store.config.members
    );
    assert!(
        app.session.team_members.iter().any(|m| m.name == "alice"),
        "the pane shows the seeded teammate"
    );
    assert_eq!(manager.list_agents(&primary).await.len(), 1);
}

/// `/team close` cancels the running leader task of the team it closes.
#[tokio::test]
async fn test_slash_team_create_leader_close_cancels_leader() {
    let ws = workspace("close");
    let (mut app, manager) = app_in(&ws);
    let primary = app.session.session_id.clone().expect("primary");
    let store = TeamStore::create("alpha", "leader-sess", &ws, true).expect("team");
    app.session.active_team = Some(store.config.clone());
    // A leader that is still running (a real one fails at once here: its
    // model is not registered).
    let flag = manager.insert_task_for_test(TaskEntry {
        id: "team-leader-1".to_string(),
        parent_session_id: primary,
        child_session_id: "leader-sess".to_string(),
        agent_name: "team-leader".to_string(),
        task_prompt: "lead".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    });

    app.execute_slash_command("/team close").await;

    assert_eq!(app.status, "team closed");
    assert!(
        flag.load(Ordering::Relaxed),
        "closing the team cancels its leader"
    );
}
