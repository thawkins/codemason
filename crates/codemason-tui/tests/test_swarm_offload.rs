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

/// A `TaskCompleted` hook script that records its pid in `runs.log` (one
/// line per run), then becomes `head`, which blocks until one byte is
/// written to the FIFO `gate`. `exec` keeps it one process, so the pid is
/// the process the hook runner spawned and kills.
fn gated_hook(dir: &Path, gate: &Path) -> PathBuf {
    let status = std::process::Command::new("mkfifo")
        .arg(gate)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "mkfifo failed");
    let script = dir.join("gate-hook.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ >> '{}'\nexec head -c 1 '{}' >/dev/null\n",
            dir.join("runs.log").display(),
            gate.display()
        ),
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
    finishing_swarm_of(dir, hook, &["t1"]).await
}

/// [`finishing_swarm`] with one in-progress task per id in `task_ids`, so
/// the auto-complete job runs the gated hook once per task, in order.
async fn finishing_swarm_of(dir: &Path, hook: &Path, task_ids: &[&str]) -> App {
    let store = TeamStore::create(TEAM, "lead-session", dir, true).expect("create team");
    for id in task_ids {
        let mut task = Task::new(*id, format!("Task {id}"));
        task.status = TaskStatus::InProgress;
        task.assigned_to = Some("tm-001".to_string());
        TaskStore::open(&store.dir)
            .expect("task store")
            .add_task(task)
            .await
            .expect("seed task");
    }
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
    app.session.directory = dir.to_path_buf();
    app.session.active_team = Some(store.config.clone());
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

    // A new swarm under another team replaces it while its job runs; that
    // swarm's teammate is still working.
    let mut replacement = app.session.swarm_state.clone().expect("swarm");
    replacement.team_name = "swarm-other".to_string();
    app.session.swarm_state = Some(replacement);
    let mut member = TeamMember::new("swarm-t9", "tm-009", "general");
    member.status = MemberStatus::Working;
    app.session.team_members = vec![member];
    open_gate(gate).await;
    assert!(immediate, "the pass waited on the hook");
    assert!(in_flight);
    app.wait_swarm_autocomplete_for_tests().await;

    // The old job's end is not the new swarm's tally: the 2 s throttle still
    // applies to it, so the pass checks nothing and finalises nothing.
    let last_poll = Instant::now();
    app.session.swarm_completion_last_poll = last_poll;
    app.poll_swarm_completion().await;

    assert!(
        !app.swarm_autocomplete_in_flight(),
        "the finished job is dropped"
    );
    assert_eq!(
        app.session.swarm_completion_last_poll, last_poll,
        "the throttle applied: the job did not land for this swarm"
    );
    assert!(!reported_complete(&app));
}

/// The pids the gated hook recorded, one per run.
fn hook_runs(dir: &Path) -> Vec<u32> {
    std::fs::read_to_string(dir.join("runs.log"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().parse().expect("pid"))
        .collect()
}

/// Yield until the first hook run has started and recorded its pid.
async fn hook_started(dir: &Path) -> u32 {
    loop {
        if let Some(pid) = hook_runs(dir).first() {
            return *pid;
        }
        tokio::task::yield_now().await;
    }
}

/// Whether the process `pid` has exited (gone, or a zombie not yet reaped).
fn process_ended(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(_) => true,
        // The state follows the parenthesised command name.
        Ok(stat) => stat
            .rsplit_once(')')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('Z')),
    }
}

/// Yield until the hook process `pid` has ended. The stop sent it SIGKILL
/// (`kill_on_drop`); this only waits for the kernel to finish it.
async fn hook_ended(pid: u32) {
    while !process_ended(pid) {
        tokio::task::yield_now().await;
    }
}

/// A two-task finishing swarm whose job is blocked in the first task's
/// hook, and that hook's pid.
async fn swarm_blocked_in_its_hook(dir: &Path) -> (App, u32) {
    let gate = dir.join("gate");
    let hook = gated_hook(dir, &gate);
    let mut app = finishing_swarm_of(dir, &hook, &["t1", "t2"]).await;
    assert!(pass_is_immediate(&mut app), "the pass waited on the hook");
    assert!(app.swarm_autocomplete_in_flight());
    let pid = hook_started(dir).await;
    (app, pid)
}

/// After a cancel or removal: the job is gone, its running hook ended, and
/// no further hook started (the second task's never ran).
async fn assert_job_stopped(app: &App, dir: &Path, pid: u32) {
    assert!(!app.swarm_autocomplete_in_flight(), "the job was stopped");
    hook_ended(pid).await;
    assert_eq!(hook_runs(dir), vec![pid], "no hook ran after the stop");
}

/// The team's directory.
fn team_dir(dir: &Path) -> PathBuf {
    dir.join(".codemason/teams").join(TEAM)
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_swarm_cancel_while_the_hook_blocks_stops_the_job() {
    let dir = scratch("cancel");
    let (mut app, pid) = swarm_blocked_in_its_hook(&dir).await;
    assert!(team_dir(&dir).is_dir());

    app.execute_slash_command("/swarm cancel").await;

    assert_job_stopped(&app, &dir, pid).await;
    assert!(app.session.swarm_state.is_none());
    assert_eq!(app.status, "swarm: cancelled");
    // Nothing of the job writes into the team any more: removing it takes
    // the whole directory.
    app.execute_slash_command(&format!("/team delete {TEAM} --force"))
        .await;
    assert!(!team_dir(&dir).exists(), "status: {}", app.status);
    assert_eq!(hook_runs(&dir), vec![pid]);
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_team_close_while_the_hook_blocks_stops_the_job() {
    let dir = scratch("close");
    let (mut app, pid) = swarm_blocked_in_its_hook(&dir).await;

    app.execute_slash_command("/team close").await;

    assert_job_stopped(&app, &dir, pid).await;
    assert!(app.session.swarm_state.is_none());
    assert_eq!(app.status, "team closed");
    // Closing keeps the team on disk.
    assert!(team_dir(&dir).is_dir());
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_team_removal_while_the_hook_blocks_removes_the_whole_directory() {
    for (tag, command) in [
        ("delete", format!("/team delete {TEAM} --force")),
        ("forcecleanup", "/team forcecleanup confirm".to_string()),
        ("cleanup", "/team cleanup".to_string()),
    ] {
        let dir = scratch(tag);
        let (mut app, pid) = swarm_blocked_in_its_hook(&dir).await;

        app.execute_slash_command(&command).await;

        assert_job_stopped(&app, &dir, pid).await;
        assert!(
            !team_dir(&dir).exists(),
            "{command}: the directory is fully removed (status: {})",
            app.status
        );
    }
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_team_delete_stops_the_job_of_a_background_session() {
    let dir = scratch("background");
    let (mut app, pid) = swarm_blocked_in_its_hook(&dir).await;
    // The finalising session goes to the background.
    app.open_new_session().expect("new session");
    assert_ne!(app.session.session_id.as_deref(), Some("lead-session"));

    app.execute_slash_command(&format!("/team delete {TEAM} --force"))
        .await;

    hook_ended(pid).await;
    assert_eq!(hook_runs(&dir), vec![pid], "no hook ran after the stop");
    assert!(!team_dir(&dir).exists(), "status: {}", app.status);
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_app_exit_aborts_the_job_and_leaves_the_store_consistent() {
    let dir = scratch("exit");
    let (app, pid) = swarm_blocked_in_its_hook(&dir).await;

    // The frontend exits: the App (and the session holding the job) drops.
    drop(app);

    hook_ended(pid).await;
    assert_eq!(hook_runs(&dir), vec![pid], "no hook ran after exit");
    // Each completion is one locked write: the task whose hook was running
    // is completed, the one the job never reached stays in progress.
    let store = TeamStore::load_by_name(TEAM, &dir).expect("team");
    let tasks = TaskStore::open(&store.dir)
        .expect("task store")
        .read()
        .expect("tasks.json parses");
    let status = |id: &str| {
        tasks
            .tasks
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.status)
            .expect("task")
    };
    assert_eq!(status("t1"), TaskStatus::Completed);
    assert_eq!(status("t2"), TaskStatus::InProgress);
}

/// A `TaskCompleted` hook script that records its pid in `runs.log`, starts
/// a grandchild (`head`, blocked on the FIFO `gate`) whose pid it records in
/// `kids.log`, and waits for it: the hook process is `sh`, and the blocked
/// process is its child, in the hook's process group.
fn grandchild_hook(dir: &Path, gate: &Path) -> PathBuf {
    let status = std::process::Command::new("mkfifo")
        .arg(gate)
        .status()
        .expect("run mkfifo");
    assert!(status.success(), "mkfifo failed");
    let script = dir.join("grandchild-hook.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ >> '{runs}'\nhead -c 1 '{gate}' >/dev/null &\necho $! >> '{kids}'\nwait\n",
            runs = dir.join("runs.log").display(),
            kids = dir.join("kids.log").display(),
            gate = gate.display()
        ),
    )
    .expect("write hook script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make hook executable");
    script
}

/// Yield until the hook has recorded its grandchild's pid.
async fn grandchild_started(dir: &Path) -> u32 {
    loop {
        let kids = std::fs::read_to_string(dir.join("kids.log")).unwrap_or_default();
        if let Some(pid) = kids.lines().next().and_then(|l| l.trim().parse().ok()) {
            return pid;
        }
        tokio::task::yield_now().await;
    }
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_swarm_cancel_kills_the_hooks_whole_process_group() {
    let dir = scratch("group");
    let gate = dir.join("gate");
    let hook = grandchild_hook(&dir, &gate);
    let mut app = finishing_swarm_of(&dir, &hook, &["t1", "t2"]).await;
    assert!(pass_is_immediate(&mut app), "the pass waited on the hook");
    let hook_pid = hook_started(&dir).await;
    let grandchild = grandchild_started(&dir).await;

    app.execute_slash_command("/swarm cancel").await;

    assert!(!app.swarm_autocomplete_in_flight(), "the job was stopped");
    hook_ended(hook_pid).await;
    // The grandchild was killed with the group, not left blocked on the gate.
    hook_ended(grandchild).await;
    assert_eq!(
        hook_runs(&dir),
        vec![hook_pid],
        "no hook ran after the stop"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_team_cleaned_up_event_stops_the_job() {
    let dir = scratch("cleaned-up");
    let (mut app, pid) = swarm_blocked_in_its_hook(&dir).await;

    // The lead's `team_cleanup` tool removed the team and reports it.
    app.handle_event(codemason_agent::event::Event::TeamCleanedUp {
        session_id: "lead-session".to_string(),
        team_name: TEAM.to_string(),
    })
    .await;

    assert_job_stopped(&app, &dir, pid).await;
    assert!(app.session.active_team.is_none());
    assert!(app.session.swarm_state.is_none());
}
