//! The `/swarm` synthesis step: once every task is done, one LLM call
//! combines the task results, and `App::poll_pending_swarm` shows the answer
//! (or the failure) in the chat.
//!
//! Every file is written under the workspace `target/temp/`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use codemason_agent::message::MessagePart;
use codemason_agent::team::task::Task;
use codemason_agent::team::{
    MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, TaskStatus, TaskStore, TeamMember,
    TeamStore,
};
use codemason_app_core::app::App;

#[path = "support/mod.rs"]
mod support;

const TEAM: &str = "swarm-synthesis";

/// A fresh scratch project (with a `.codemason/` directory) under the
/// workspace `target/temp/`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/tui-swarm-synthesis")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch dir");
    }
    std::fs::create_dir_all(dir.join(".codemason")).expect("create scratch project");
    dir.canonicalize().expect("canonical scratch dir")
}

/// Whether any chat message holds text containing `needle`.
fn chat_contains(app: &App, needle: &str) -> bool {
    app.session.messages.iter().any(|m| {
        m.parts
            .iter()
            .any(|p| matches!(p, MessagePart::Text { text } if text.contains(needle)))
    })
}

/// A swarm whose one task is completed with a result and whose teammate is
/// idle, so the next completion pass finalises it.
async fn finished_swarm(dir: &Path) -> App {
    let store = TeamStore::create(TEAM, "lead-session", dir, true).expect("create team");
    let mut task = Task::new("s1", "Title s1");
    task.status = TaskStatus::Completed;
    task.assigned_to = Some("tm-001".to_string());
    task.result = Some("s1 result".to_string());
    TaskStore::open(&store.dir)
        .expect("task store")
        .add_task(task)
        .await
        .expect("seed task");

    let mut app = support::make_app();
    app.session.session_id = Some("lead-session".to_string());
    app.cwd_path = dir.to_path_buf();
    app.session.directory = dir.to_path_buf();
    app.session.active_team = Some(store.config.clone());
    let mut member = TeamMember::new("swarm-s1", "tm-001", "general");
    member.status = MemberStatus::Idle;
    app.session.team_members = vec![member];
    app.session.swarm_state = Some(SwarmState {
        team_name: TEAM.to_string(),
        prompt: "Harden auth".to_string(),
        decomposition: SwarmDecomposition {
            tasks: vec![SwarmSubtask {
                id: "s1".to_string(),
                title: "Title s1".to_string(),
                description: String::new(),
                depends_on: Vec::new(),
                agent_type: None,
                model: None,
            }],
        },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.session.swarm_completion_last_poll = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("clock");
    app
}

/// Run completion passes until the swarm is finalised. The first pass hands
/// the leftover-task auto-completion to the runtime; a later pass, once that
/// job has ended, reads the tally and finalises.
async fn finalise(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !app
        .session
        .swarm_state
        .as_ref()
        .is_some_and(|s| s.completed)
    {
        assert!(Instant::now() < deadline, "swarm never finalised");
        app.session.swarm_completion_last_poll = Instant::now()
            .checked_sub(Duration::from_secs(3))
            .expect("clock");
        app.poll_swarm_completion().await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn test_swarm_synthesis_answer_is_shown() {
    let mut app = support::make_app();
    app.session.session_id = Some("lead-session".to_string());
    *app.session.swarm_synthesis_result.lock().expect("slot") =
        Some(Ok("All auth handlers now rate-limited.".to_string()));
    app.poll_pending_swarm().await;
    assert!(chat_contains(&app, "## Swarm Result"));
    assert!(chat_contains(&app, "All auth handlers now rate-limited."));
    assert!(
        app.session
            .swarm_synthesis_result
            .lock()
            .expect("slot")
            .is_none()
    );
}

#[tokio::test]
async fn test_swarm_synthesis_failure_is_reported() {
    let mut app = support::make_app();
    app.session.session_id = Some("lead-session".to_string());
    *app.session.swarm_synthesis_result.lock().expect("slot") =
        Some(Err("provider down".to_string()));
    app.poll_pending_swarm().await;
    assert!(chat_contains(&app, "Synthesis Failed"));
    assert!(chat_contains(&app, "provider down"));
}

#[tokio::test]
async fn test_swarm_completion_without_model_skips_synthesis() {
    let dir = scratch("no-model");
    let mut app = finished_swarm(&dir).await;
    app.session.selected_model = None;
    finalise(&mut app).await;
    assert!(chat_contains(&app, "Swarm Complete: swarm-synthesis"));
    assert!(!chat_contains(&app, "Combining the results"));
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}

/// With a model selected, completion starts the synthesis call. The provider
/// does not exist, so the call fails fast without any network access, and
/// that failure is what reaches the chat - proving the call ran and its
/// outcome was adopted.
#[tokio::test]
async fn test_swarm_completion_starts_synthesis_call() {
    let dir = scratch("with-model");
    let mut app = finished_swarm(&dir).await;
    app.session.selected_model = Some("no-such-provider/model".to_string());
    finalise(&mut app).await;
    assert!(chat_contains(&app, "Combining the results"));

    let deadline = Instant::now() + Duration::from_secs(20);
    while app
        .session
        .swarm_synthesis_result
        .lock()
        .expect("slot")
        .is_none()
    {
        assert!(Instant::now() < deadline, "synthesis call never returned");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    app.poll_pending_swarm().await;
    assert!(chat_contains(&app, "Synthesis Failed"));
    std::fs::remove_dir_all(&dir).expect("clean scratch dir");
}
