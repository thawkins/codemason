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
    // A dependent swarm task is reserved for its teammate at seeding.
    pending.assigned_to = Some("tm-002".to_string());
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

    // The dependent task is given to its own teammate, so no idle teammate
    // can take it with a bare continuation.
    let s2 = TaskStore::open(&team_dir)
        .expect("task store")
        .read()
        .expect("read tasks")
        .tasks
        .into_iter()
        .find(|t| t.id == "s2")
        .expect("s2 task");
    assert_eq!(s2.status, TaskStatus::InProgress);
    assert_eq!(s2.assigned_to.as_deref(), Some("tm-002"));

    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

/// A decomposition landing creates a swarm team with auto-claim off, names
/// each teammate `<agent-type>-<adjective>-<surname>`, gives each ready task
/// to its own teammate, reserves a dependent task (still pending) for its
/// teammate until unblock, and keeps the goal for the synthesis step.
#[tokio::test]
async fn test_swarm_decomposition_assigns_tasks_and_disables_auto_claim() {
    let dir = scratch("decompose");
    let mut app = support::make_app();
    app.cwd_path = dir.clone();
    app.session.directory = dir.clone();
    let manager = support::attach_leader_runtime(&mut app);
    app.session.swarm_state = Some(SwarmState {
        team_name: String::new(),
        prompt: "Harden auth".to_string(),
        decomposition: SwarmDecomposition { tasks: Vec::new() },
        spawned: false,
        completed: false,
        default_agent_type: None,
    });
    *app.session.swarm_result.lock().expect("slot") = Some(Ok(r#"{"tasks":[
        {"id":"s1","title":"Find","description":"Find auth","depends_on":[]},
        {"id":"s2","title":"Fix","description":"Fix auth","depends_on":["s1"]}
    ]}"#
    .to_string()));

    app.poll_pending_swarm().await;

    let state = app.session.swarm_state.clone().expect("swarm state");
    assert_eq!(
        state.prompt, "Harden auth",
        "the goal must survive decomposition"
    );
    let team_dir =
        codemason_agent::team::find_team_dir(&dir, &state.team_name).expect("swarm team dir");
    let config = TeamStore::load(&team_dir).expect("load team").config;
    assert!(!config.settings.auto_claim_tasks);
    let leaders = manager
        .list_agents(app.session.session_id.as_deref().expect("primary"))
        .await;
    assert_eq!(leaders.len(), 1, "one team leader: {leaders:?}");
    assert_eq!(leaders[0].agent_name, "team-leader");
    assert_eq!(
        config.lead_session_id, leaders[0].child_session_id,
        "the swarm team is led by its team leader, not the primary"
    );
    assert_eq!(config.members.len(), 2);
    for member in &config.members {
        let parts: Vec<&str> = member.name.split('-').collect();
        assert_eq!(parts.len(), 3, "readable name: {}", member.name);
        assert_eq!(
            parts[0], member.agent_type,
            "prefixed by agent type: {}",
            member.name
        );
        assert!(!member.name.starts_with("swarm-"), "{}", member.name);
    }

    let tasks = TaskStore::open(&team_dir)
        .expect("task store")
        .read()
        .expect("read tasks")
        .tasks;
    let s1 = tasks.iter().find(|t| t.id == "s1").expect("s1");
    assert_eq!(s1.status, TaskStatus::InProgress);
    let s2 = tasks.iter().find(|t| t.id == "s2").expect("s2");
    assert_eq!(s2.status, TaskStatus::Pending);
    let (Some(s1_agent), Some(s2_agent)) = (s1.assigned_to.as_deref(), s2.assigned_to.as_deref())
    else {
        panic!("both tasks have their teammate: {s1:?} {s2:?}");
    };
    assert_ne!(s1_agent, s2_agent, "one teammate per subtask");
    assert!(config.member_by_id(s1_agent).is_some());
    assert!(config.member_by_id(s2_agent).is_some());

    app.stop_swarm_autocomplete(&state.team_name).await;
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

/// `/swarm cancel` cancels the swarm's running team leader.
#[tokio::test]
async fn test_swarm_cancel_stops_the_leader() {
    let dir = scratch("cancel-leader");
    let mut app = support::make_app();
    app.cwd_path = dir.clone();
    app.session.directory = dir.clone();
    let manager = support::attach_leader_runtime(&mut app);
    let primary = app.session.session_id.clone().expect("primary");
    let store = TeamStore::create(TEAM, "swarm-leader-sess", &dir, true).expect("team");
    app.session.active_team = Some(store.config.clone());
    app.session.swarm_state = Some(SwarmState {
        team_name: TEAM.to_string(),
        prompt: "Harden auth".to_string(),
        decomposition: SwarmDecomposition { tasks: Vec::new() },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    let leader = codemason_agent::task::TaskEntry {
        id: "team-leader-swarm".to_string(),
        parent_session_id: primary,
        child_session_id: "swarm-leader-sess".to_string(),
        agent_name: "team-leader".to_string(),
        task_prompt: "lead the swarm".to_string(),
        background: true,
        detached: false,
        status: codemason_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: codemason_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    };
    let flag = manager.insert_task_for_test(leader);

    app.execute_slash_command("/swarm cancel").await;

    assert_eq!(app.status, "swarm: cancelled");
    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "cancelling the swarm cancels its leader"
    );
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

/// The swarm leader's own `team_cleanup` reports the leader's session; the
/// frontend that launched it still clears the swarm and drops the team from
/// the pane, so a new `/swarm` is accepted.
#[tokio::test]
async fn test_swarm_leader_cleanup_clears_swarm_state() {
    let dir = scratch("leader-cleanup");
    let mut app = support::make_app();
    app.cwd_path = dir.clone();
    app.session.directory = dir.clone();
    support::attach_leader_runtime(&mut app);
    let primary = app.session.session_id.clone().expect("primary");
    let leader = app
        .session_processor
        .session_manager
        .create_child_session(
            dir.clone(),
            &primary,
            codemason_storage::SessionKind::Subagent,
        )
        .expect("leader session");
    let store = TeamStore::create(TEAM, &leader.id, &dir, true).expect("team");
    let manager = codemason_agent::team::TeamManager::new(
        TEAM,
        leader.id.clone(),
        store.dir.clone(),
        std::sync::Arc::clone(&app.session_processor),
        app.event_bus.clone(),
    )
    .into_arc();
    assert!(app.session_processor.team_managers.bind(manager).is_ok());
    app.session.swarm_state = Some(SwarmState {
        team_name: TEAM.to_string(),
        prompt: "Harden auth".to_string(),
        decomposition: SwarmDecomposition { tasks: Vec::new() },
        spawned: true,
        completed: true,
        default_agent_type: None,
    });

    app.handle_event(codemason_agent::event::Event::TeamCleanedUp {
        session_id: leader.id.clone(),
        team_name: TEAM.to_string(),
    })
    .await;

    assert!(app.session.swarm_state.is_none(), "the swarm is cleared");
    assert!(
        app.session_processor.team_managers.get(TEAM).is_none(),
        "the team is dropped from the pane"
    );
    app.execute_slash_command("/swarm Harden auth again").await;
    assert_ne!(
        app.status, "[warn] A swarm is already active - use /swarm cancel first",
        "a new swarm is accepted"
    );

    // A cleanup reported by a session that is not this session's leader is
    // ignored.
    app.session.swarm_state = Some(SwarmState {
        team_name: "swarm-other".to_string(),
        prompt: "x".to_string(),
        decomposition: SwarmDecomposition { tasks: Vec::new() },
        spawned: true,
        completed: true,
        default_agent_type: None,
    });
    app.handle_event(codemason_agent::event::Event::TeamCleanedUp {
        session_id: "stranger".to_string(),
        team_name: "swarm-other".to_string(),
    })
    .await;
    assert!(app.session.swarm_state.is_some());
    // INTENTIONAL: a `/swarm` call may still be writing; leftovers stay in target/temp.
    let _ = std::fs::remove_dir_all(&dir);
}
