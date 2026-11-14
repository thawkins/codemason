//! Tests that a finalising `/swarm` runs its task store writes and their
//! `TaskCompleted` hooks off the frontend's thread (`App::poll_swarm_completion`).
//!
//! The team's `TaskCompleted` hook is a script that blocks on a FIFO until
//! the test writes one byte, so the hook's duration is controlled exactly:
//! no sleeps and no timeouts. A pass is polled once with a no-op waker; it
//! must be ready (it never waits on the hook), the job must be in flight,
//! and a second pass while it is must start nothing and finalise nothing.
//! Once the gate opens and the job ends, the next pass applies the tally.
//! Every file is written under the workspace `target/temp/`.

#![cfg(unix)]

use std::future::Future;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use codemason_agent::message::MessagePart;
use codemason_agent::team::task::Task;
use codemason_agent::team::{
    HookEntry, HookEvent, MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, TaskStatus,
    TaskStore, TeamMember, TeamStore,
};
use codemason_tui::App;

#[path = "support/mod.rs"]
mod support;

/// The swarm team the test finalises.
const TEAM: &str = "swarm-offload";

/// A fresh scratch project (with a `.codemason/` directory) under the
/// workspace `target/temp/`.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/tui-swarm-offload")
        .join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch dir");
    }
    std::fs::create_dir_all(dir.join(".codemason")).expect("create scratch project");
    dir.canonicalize().expect("canonical scratch dir")
}

/// A `TaskCompleted` hook script that blocks until one byte is written to
/// the FIFO `gate`.
fn gated_hook(dir: &Path, gate: &Path) -> PathBuf {
    let status = std::process::Command::new("mkfifo")
        .arg(gate)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "mkfifo failed");
    let script = dir.join("gate-hook.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\nhead -c 1 '{}' >/dev/null\n", gate.display()),
    )
    .expect("write hook script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make hook executable");
    script
}

/// A swarm whose one teammate has finished but whose task is still in
/// progress, so the next completion pass auto-completes it, which fires the
/// gated hook.
async fn finishing_swarm(dir: &Path, hook: &Path) -> App {
    let store = TeamStore::create(TEAM, "lead-session", dir, true).expect("create team");
    let mut task = Task::new("t1", "Task t1");
    task.status = TaskStatus::InProgress;
    task.assigned_to = Some("tm-001".to_string());
    TaskStore::open(&store.dir)
        .expect("task store")
        .add_task(task)
        .await
        .expect("seed task");
    TeamStore::update(&store.dir, |config| {
        config.settings.hooks.push(HookEntry {
            event: HookEvent::TaskCompleted,
            command: hook.display().to_string(),
        });
        Ok(())
    })
    .expect("configure hook");

    let mut app = support::make_app();
    app.session.session_id = Some("lead-session".to_string());
    app.cwd_path = dir.to_path_buf();
    let mut member = TeamMember::new("swarm-t1", "tm-001", "general");
    member.status = MemberStatus::Idle;
    app.session.team_members = vec![member];
    app.session.swarm_state = Some(SwarmState {
        team_name: TEAM.to_string(),
        prompt: String::new(),
        decomposition: SwarmDecomposition {
            tasks: vec![SwarmSubtask {
                id: "t1".to_string(),
                title: "Task t1".to_string(),
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
    make_completion_due(&mut app);
    app
}

/// Move the 2 s completion throttle back so the next pass checks.
fn make_completion_due(app: &mut App) {
    app.session.swarm_completion_last_poll = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("clock");
}

/// Poll one completion pass once, as a frontend's `block_on` would start
/// it: whether it finished without waiting on anything.
fn pass_is_immediate(app: &mut App) -> bool {
    let mut pass = pin!(app.poll_swarm_completion());
    let mut cx = Context::from_waker(Waker::noop());
    matches!(pass.as_mut().poll(&mut cx), Poll::Ready(()))
}

/// Whether the swarm was reported complete.
fn reported_complete(app: &App) -> bool {
    let finalised = app
        .session
        .swarm_state
        .as_ref()
        .is_some_and(|s| s.completed);
    let message = app.session.messages.iter().any(|m| {
        m.parts.iter().any(|p| {
            matches!(p, MessagePart::Text { text } if text.contains("Swarm Complete: swarm-offload"))
        })
    });
    assert_eq!(finalised, message, "the flag and the message move together");
    finalised
}

/// Open the gate: the blocked hook reads one byte and exits.
async fn open_gate(gate: PathBuf) {
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let mut fifo = std::fs::OpenOptions::new()
            .write(true)
            .open(&gate)
            .expect("open gate");
        fifo.write_all(b"x").expect("write gate");
    })
    .await
    .expect("gate writer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_swarm_completion_hook_runs_off_the_frontend_thread() {
    let dir = scratch("completion");
    let gate = dir.join("gate");
    let hook = gated_hook(&dir, &gate);
    let mut app = finishing_swarm(&dir, &hook).await;

    // The pass starts the auto-complete job and returns at once, while the
    // hook is still blocked.
    let first_immediate = pass_is_immediate(&mut app);
    let in_flight_after_first = app.swarm_autocomplete_in_flight();
    let complete_after_first = reported_complete(&app);

    // A pass while the job runs starts nothing and finalises nothing.
    make_completion_due(&mut app);
    let second_immediate = pass_is_immediate(&mut app);
    let in_flight_after_second = app.swarm_autocomplete_in_flight();
    let complete_after_second = reported_complete(&app);

    // Release the hook before asserting, so a failing run leaves no blocked
    // process behind.
    open_gate(gate).await;
    assert!(first_immediate, "the pass waited on the hook");
    assert!(in_flight_after_first, "the auto-complete job is in flight");
    assert!(
        !complete_after_first,
        "nothing is finalised before the job ends"
    );
    assert!(second_immediate);
    assert!(in_flight_after_second);
    assert!(!complete_after_second);

    // The job ends; the next pass applies the tally without waiting out the
    // 2 s throttle.
    app.wait_swarm_autocomplete_for_tests().await;
    app.poll_swarm_completion().await;

    assert!(!app.swarm_autocomplete_in_flight());
    assert!(reported_complete(&app));
    let store = TeamStore::load_by_name(TEAM, &dir).expect("team");
    let tasks = TaskStore::open(&store.dir)
        .expect("task store")
        .read()
        .expect("tasks");
    assert_eq!(tasks.tasks[0].status, TaskStatus::Completed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_swarm_completion_job_of_a_replaced_swarm_is_not_its_tally() {
    let dir = scratch("replaced");
    let gate = dir.join("gate");
    let hook = gated_hook(&dir, &gate);
    let mut app = finishing_swarm(&dir, &hook).await;
    let immediate = pass_is_immediate(&mut app);
    let in_flight = app.swarm_autocomplete_in_flight();

    // The swarm is cancelled while its job runs.
    app.session.swarm_state = None;
    open_gate(gate).await;
    assert!(immediate, "the pass waited on the hook");
    assert!(in_flight);
    app.wait_swarm_autocomplete_for_tests().await;
    make_completion_due(&mut app);
    app.poll_swarm_completion().await;

    // The finished job is dropped and nothing is reported.
    assert!(!app.swarm_autocomplete_in_flight());
    assert!(!reported_complete(&app));
}
