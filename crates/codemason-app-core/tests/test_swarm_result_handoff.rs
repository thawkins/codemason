//! A blocked `/swarm` teammate, once its dependencies complete, is spawned
//! with a prompt that carries their stored results (`App::poll_swarm_unblock`).
//!
//! Every file is written under the workspace `target/temp/`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use codemason_agent::team::task::Task;
use codemason_agent::team::{
    MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, TaskStatus, TaskStore, TeamMember,
    TeamStore,
};
use codemason_app_core::app::App;

#[path = "support/mod.rs"]
mod support;

const TEAM: &str = "swarm-handoff";

/// A fresh scratch project (with a `.codemason/` directory) under the
/// workspace `target/temp/`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/tui-swarm-handoff")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch dir");
    }
    std::fs::create_dir_all(dir.join(".codemason")).expect("create scratch project");
    dir.canonicalize().expect("canonical scratch dir")
}

fn subtask(id: &str, depends_on: &[&str]) -> SwarmSubtask {
    SwarmSubtask {
        id: id.to_string(),
        title: format!("Title {id}"),
        description: format!("Do the {id} work"),
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        agent_type: None,
        model: None,
    }
}

/// A swarm where `s1` is completed with a result and `s2` (depending on it)
/// is still blocked.
async fn swarm_with_blocked_dependent(dir: &Path) -> (App, PathBuf) {
    let store = TeamStore::create(TEAM, "lead-session", dir, true).expect("create team");
    let tasks = TaskStore::open(&store.dir).expect("task store");
    let mut done = Task::new("s1", "Title s1");
    done.status = TaskStatus::Completed;
    done.assigned_to = Some("tm-001".to_string());
    done.result = Some("s1 found the parser in src/parse.rs".to_string());
    tasks.add_task(done).await.expect("seed s1");
    let mut pending = Task::new("s2", "Title s2");
    pending.depends_on = vec!["s1".to_string()];
    tasks.add_task(pending).await.expect("seed s2");

    let mut first = TeamMember::new("swarm-s1", "tm-001", "general");
    first.status = MemberStatus::Idle;
    let mut second = TeamMember::new("swarm-s2", "tm-002", "general");
    second.status = MemberStatus::Blocked;
    second.spawn_prompt = Some("stale prompt".to_string());
    let members = vec![first, second];
    let persisted = members.clone();
    TeamStore::update(&store.dir, |config| {
        config.members = persisted;
        Ok(())
    })
    .expect("persist members");

    let mut app = support::make_app();
    app.session.session_id = Some("lead-session".to_string());
    app.cwd_path = dir.to_path_buf();
    app.session.directory = dir.to_path_buf();
    app.session.active_team = Some(store.config.clone());
    app.session.team_members = members;
    app.session.swarm_state = Some(SwarmState {
        team_name: TEAM.to_string(),
        prompt: String::new(),
        decomposition: SwarmDecomposition {
            tasks: vec![subtask("s1", &[]), subtask("s2", &["s1"])],
        },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.session.swarm_unblock_last_poll = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("clock");
    (app, store.dir)
}

#[tokio::test]
async fn test_swarm_unblock_rewrites_prompt_with_predecessor_result() {
    let dir = scratch("unblock");
    let (mut app, team_dir) = swarm_with_blocked_dependent(&dir).await;

    app.poll_swarm_unblock();

    let config = TeamStore::load(&team_dir).expect("load team").config;
    let member = config.member_by_name("swarm-s2").expect("s2 member");
    assert_eq!(member.status, MemberStatus::Spawning);
    let prompt = member.spawn_prompt.as_deref().expect("spawn prompt");
    assert!(prompt.contains("## Results from dependencies"), "{prompt}");
    assert!(
        prompt.contains("s1 found the parser in src/parse.rs"),
        "{prompt}"
    );
    assert!(prompt.contains("Do the s2 work"), "{prompt}");

    let local = app
        .session
        .team_members
        .iter()
        .find(|m| m.name == "swarm-s2")
        .expect("local s2");
    assert_eq!(local.spawn_prompt.as_deref(), Some(prompt));

    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}
