//! Team leaders and teammates in the Sessions pane (team-leader-agent spec,
//! sections 5 and 6): leader rows nest under the session that spawned them,
//! teammate rows under their leader; opening a row and typing sends a direct
//! user turn to that agent.
//!
//! Sessions live in in-memory storage; teams are written under the
//! git-ignored `target/temp/test_sidebar_team_rows/` tree only.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_agent::team::{
    MemberStatus, TeamManager, TeamMember, TeamStatus, TeamStore, TeammateNextTurn,
};
use codemason_app_core::app::App;
use codemason_app_core::sessions_sidebar_state::{
    LeaderState, SidebarRow, build_rows, click_row, refresh_if_dirty,
};
use codemason_storage::SessionKind;

#[path = "support/mod.rs"]
mod support;

/// A fresh scratch project under `target/temp/test_sidebar_team_rows/<test>`.
fn workspace(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_sidebar_team_rows")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(dir.join(".codemason")).expect("workspace");
    dir.canonicalize().expect("canonical workspace")
}

/// Points `app` at `ws` and turns the sidebar on.
fn pin_to(app: &mut App, ws: &Path) {
    app.cwd_path = ws.to_path_buf();
    app.cwd = ws.display().to_string();
    app.sidebar.visible = true;
}

/// An app over `ws` with one top-level (primary) session: `(app, primary)`.
fn app_with_primary(ws: &Path) -> (App, String) {
    let mut app = support::make_app();
    pin_to(&mut app, ws);
    let primary = app
        .session_processor
        .session_manager
        .create_session(ws.to_path_buf())
        .expect("primary session")
        .id;
    app.session.session_id = Some(primary.clone());
    (app, primary)
}

/// A team leader: its session, its team's on-disk directory and its
/// teammates' sessions (in roster order).
struct Leader {
    session: String,
    team_dir: PathBuf,
    teammates: Vec<String>,
}

/// Adds a leader child session of `parent` leading Active team `team` with
/// one idle teammate per name in `members`. A `running` leader also gets a
/// running `team-leader` task.
fn add_leader(app: &mut App, ws: &Path, parent: &str, team: &str, members: &[&str]) -> Leader {
    let sessions = &app.session_processor.session_manager;
    let leader = sessions
        .create_child_session(ws.to_path_buf(), parent, SessionKind::Subagent)
        .expect("leader session")
        .id;
    let mut store = TeamStore::create(team, &leader, ws, true).expect("create team");
    let mut teammates = Vec::new();
    for (n, name) in members.iter().enumerate() {
        let session = sessions
            .create_child_session(ws.to_path_buf(), &leader, SessionKind::Teammate)
            .expect("teammate session")
            .id;
        let mut member = TeamMember::new(*name, format!("tm-{:03}", n + 1), "general");
        member.session_id = Some(session.clone());
        member.status = MemberStatus::Idle;
        store.config.members.push(member);
        teammates.push(session);
    }
    store.save().expect("save team");
    Leader {
        session: leader,
        team_dir: store.dir.clone(),
        teammates,
    }
}

/// A running task of `agent` in `child`, spawned by `parent`.
fn running_task(parent: &str, child: &str, agent: &str) -> codemason_agent::task::TaskEntry {
    codemason_agent::task::TaskEntry {
        id: format!("task-{child}"),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "lead the team".to_string(),
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
    }
}

/// The sidebar rows after a full refresh.
fn fresh_rows(app: &mut App) -> Vec<SidebarRow> {
    app.sidebar.dirty = true;
    refresh_if_dirty(app);
    build_rows(app)
}

/// Index of `primary`'s session row.
fn session_at(rows: &[SidebarRow], primary: &str) -> usize {
    rows.iter()
        .position(|r| matches!(r, SidebarRow::Session { id, .. } if id == primary))
        .expect("primary row")
}

/// The leader row for `session`, if listed.
fn leader_row<'a>(rows: &'a [SidebarRow], session: &str) -> Option<&'a SidebarRow> {
    rows.iter()
        .find(|r| matches!(r, SidebarRow::TeamLeader { session_id, .. } if session_id == session))
}

/// Number of teammate rows.
fn teammate_rows(rows: &[SidebarRow]) -> usize {
    rows.iter()
        .filter(|r| matches!(r, SidebarRow::Teammate { .. }))
        .count()
}

/// Binds a manager for the team at `leader.team_dir` with a live handle for
/// `tm-001` and a live agent loop, as a running team has.
fn bind_manager(app: &App, team: &str, leader: &Leader) -> Arc<TeamManager> {
    let manager = TeamManager::new(
        team,
        &leader.session,
        leader.team_dir.clone(),
        Arc::clone(&app.session_processor),
        app.event_bus.clone(),
    )
    .into_arc();
    manager.register_handle_for_test("tm-001");
    // A running teammate has a live agent loop; `send_user_turn` refuses a member
    // whose loop is gone. The task parks until the test's runtime drops.
    manager.set_agent_loop_handle("tm-001", Some(tokio::spawn(std::future::pending::<()>())));
    assert!(
        app.session_processor
            .team_managers
            .bind(Arc::clone(&manager))
            .is_ok()
    );
    manager
}

/// Types `text` into the prompt and presses Enter.
async fn submit(app: &mut App, text: &str) {
    app.session.input = text.to_string();
    app.session.input_cursor = text.chars().count();
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
}

#[tokio::test]
async fn test_sidebar_team_rows_nested_under_parent() {
    let ws = workspace("nested");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1", "rev-2"]);
    let b = add_leader(&mut app, &ws, &primary, "beta", &["writer-1"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    app.active_tasks
        .push(running_task(&primary, &b.session, "team-leader"));

    let rows = fresh_rows(&mut app);
    let at = session_at(&rows, &primary);
    let shape: Vec<String> = rows[at + 1..]
        .iter()
        .take(5)
        .map(|r| match r {
            SidebarRow::TeamLeader {
                team_name, state, ..
            } => format!("leader:{team_name}:{state:?}"),
            SidebarRow::Teammate { name, status, .. } => format!("mate:{name}:{status:?}"),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        shape,
        [
            "leader:alpha:Running",
            "mate:rev-1:Idle",
            "mate:rev-2:Idle",
            "leader:beta:Running",
            "mate:writer-1:Idle",
        ],
        "rows: {rows:?}"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_leader_not_listed_as_subagent() {
    let ws = workspace("not_subagent");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));

    let rows = fresh_rows(&mut app);
    assert!(leader_row(&rows, &a.session).is_some(), "rows: {rows:?}");
    assert!(
        !rows
            .iter()
            .any(|r| matches!(r, SidebarRow::SubAgent { id, .. } if *id == a.session)),
        "the leader is listed twice: {rows:?}"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_running_leader_found_by_role_before_its_team() {
    let ws = workspace("by_role");
    install_custom_leader(&ws);
    let (mut app, primary) = app_with_primary(&ws);
    let leader = app
        .session_processor
        .session_manager
        .create_child_session(ws.clone(), &primary, SessionKind::Subagent)
        .expect("leader session")
        .id;
    app.active_tasks
        .push(running_task(&primary, &leader, "review-boss"));
    app.active_tasks
        .push(running_task(&primary, "plain-child", "explore"));

    let rows = fresh_rows(&mut app);
    assert!(
        matches!(
            leader_row(&rows, &leader),
            Some(SidebarRow::TeamLeader {
                state: LeaderState::Running,
                ..
            })
        ),
        "a custom-named leader is listed as a leader: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|r| matches!(r, SidebarRow::SubAgent { id, .. } if id == "plain-child")),
        "other sub-agents stay sub-agent rows: {rows:?}"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_crashed_leader_greyed() {
    let ws = workspace("crashed");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1", "rev-2"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    let _ = fresh_rows(&mut app);

    // The leader's task is gone while its team is still Active.
    app.active_tasks.clear();
    let rows = fresh_rows(&mut app);
    assert!(
        matches!(
            leader_row(&rows, &a.session),
            Some(SidebarRow::TeamLeader {
                state: LeaderState::Crashed,
                ..
            })
        ),
        "rows: {rows:?}"
    );
    assert_eq!(teammate_rows(&rows), 2, "rows: {rows:?}");
}

#[tokio::test]
async fn test_sidebar_team_rows_rebuilt_after_restart() {
    let ws = workspace("restart");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    let _ = fresh_rows(&mut app);

    // A fresh App over the same database and project: no tasks, no managers.
    let mut fresh = support::make_app_with_storage(Arc::clone(&app.storage));
    drop(app);
    pin_to(&mut fresh, &ws);
    let rows = fresh_rows(&mut fresh);
    let at = session_at(&rows, &primary);
    assert!(
        matches!(
            &rows[at + 1],
            SidebarRow::TeamLeader { session_id, team_name, state: LeaderState::Crashed, .. }
                if *session_id == a.session && team_name == "alpha"
        ),
        "rows: {rows:?}"
    );
    assert!(
        matches!(&rows[at + 2], SidebarRow::Teammate { name, .. } if name == "rev-1"),
        "rows: {rows:?}"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_teammates_hidden_when_team_closed() {
    let ws = workspace("closed");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1", "rev-2"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    let _ = fresh_rows(&mut app);

    // The leader cleaned its team up and finished.
    let mut store = TeamStore::load(&a.team_dir).expect("load team");
    store.config.status = TeamStatus::Disbanded;
    store.save().expect("save team");
    app.active_tasks.clear();

    let rows = fresh_rows(&mut app);
    assert_eq!(teammate_rows(&rows), 0, "rows: {rows:?}");
    assert!(
        matches!(
            leader_row(&rows, &a.session),
            Some(SidebarRow::TeamLeader { state: LeaderState::Done, team_name, .. })
                if team_name == "alpha"
        ),
        "a finished leader stays reachable: {rows:?}"
    );

    // Its directory removed by cleanup: the leader row still stays.
    std::fs::remove_dir_all(&a.team_dir).expect("remove team dir");
    let rows = fresh_rows(&mut app);
    assert!(
        matches!(
            leader_row(&rows, &a.session),
            Some(SidebarRow::TeamLeader {
                state: LeaderState::Done,
                ..
            })
        ),
        "rows: {rows:?}"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_leader_collapses() {
    let ws = workspace("collapse");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1", "rev-2"]);
    let _ = fresh_rows(&mut app);

    codemason_app_core::sessions_sidebar_state::toggle_leader(&mut app, &a.session);
    let rows = fresh_rows(&mut app);
    assert!(
        matches!(
            leader_row(&rows, &a.session),
            Some(SidebarRow::TeamLeader {
                collapsed: true,
                ..
            })
        ),
        "rows: {rows:?}"
    );
    assert_eq!(teammate_rows(&rows), 0, "rows: {rows:?}");
}

#[tokio::test]
async fn test_sidebar_team_rows_rows_open_their_session() {
    let ws = workspace("open");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    let rows = fresh_rows(&mut app);

    let at = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::Teammate { .. }))
        .expect("teammate row");
    click_row(&mut app, at, false);
    assert_eq!(
        app.session.session_id.as_deref(),
        Some(a.teammates[0].as_str())
    );

    let at = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::TeamLeader { .. }))
        .expect("leader row");
    click_row(&mut app, at, false);
    assert_eq!(app.session.session_id.as_deref(), Some(a.session.as_str()));
}

#[tokio::test]
async fn test_sidebar_team_rows_prompt_to_teammate() {
    let ws = workspace("prompt");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    let manager = bind_manager(&app, "alpha", &a);
    let rows = fresh_rows(&mut app);
    let at = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::Teammate { .. }))
        .expect("teammate row");
    click_row(&mut app, at, false);

    submit(&mut app, "go").await;

    match manager.next_teammate_turn("tm-001", &mut HashSet::new()) {
        TeammateNextTurn::Run(prompt) => assert!(prompt.contains("go"), "prompt: {prompt}"),
        other => panic!("the user turn was not queued: {other:?}"),
    }
    assert!(app.session.input.is_empty(), "input: {}", app.session.input);
    app.switch_to_session(&primary)
        .expect("back to the primary");
    assert!(
        !app.session
            .messages
            .iter()
            .any(|m| m.role == codemason_agent::message::Role::User),
        "the primary got the teammate's message"
    );
}

#[tokio::test]
async fn test_sidebar_team_rows_prompt_to_exited_teammate_errors() {
    let ws = workspace("exited");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    let _manager = bind_manager(&app, "alpha", &a);
    let mut store = TeamStore::load(&a.team_dir).expect("load team");
    store.config.members[0].status = MemberStatus::Stopped;
    store.save().expect("save team");
    let rows = fresh_rows(&mut app);
    let at = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::Teammate { .. }))
        .expect("teammate row");
    click_row(&mut app, at, false);

    submit(&mut app, "go").await;

    assert_eq!(app.session.input, "go");
    assert!(app.status.contains("has exited"), "status: {}", app.status);
    assert!(app.status.starts_with("[error]"), "status: {}", app.status);
}

#[tokio::test]
async fn test_sidebar_team_rows_prompt_to_finished_leader_resumes_once() {
    let ws = workspace("resume");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &[]);
    let agents = support::attach_leader_runtime(&mut app);
    // `attach_leader_runtime` made a new primary; the leader keeps its own.
    let rows = fresh_rows(&mut app);
    let at = rows
        .iter()
        .position(
            |r| matches!(r, SidebarRow::TeamLeader { session_id, .. } if *session_id == a.session),
        )
        .expect("leader row");
    click_row(&mut app, at, false);
    assert_eq!(app.session.session_id.as_deref(), Some(a.session.as_str()));

    // No task runs in the leader's session: the first submit starts a
    // detached resume; a second submit while it is starting is held.
    submit(&mut app, "status?").await;
    assert!(app.leader_resume_in_flight(&a.session));
    submit(&mut app, "again?").await;
    assert_eq!(
        app.status,
        "[warn] team-leader is still starting; send again in a moment"
    );
    assert_eq!(
        app.session.input, "again?",
        "a held prompt stays in the input"
    );

    drain_resumes(&mut app, &a.session).await;
    assert_eq!(
        app.status, "sent to team-leader",
        "one launch, which started"
    );
    let runs: Vec<String> = agents
        .tasks_snapshot()
        .await
        .into_iter()
        .filter(|t| t.child_session_id == a.session)
        .map(|t| t.agent_name)
        .collect();
    assert_eq!(
        runs,
        ["team-leader"],
        "exactly one turn in the leader's session"
    );
}

/// Polls until no resume for `session` is in flight, then a little longer
/// so a second, late launch would be seen too.
async fn drain_resumes(app: &mut App, session: &str) {
    for _ in 0..200 {
        app.poll_timers();
        if !app.leader_resume_in_flight(session) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        app.poll_timers();
    }
    assert!(!app.leader_resume_in_flight(session));
}

/// Writes custom agent `review-boss` with the team-leader role into `ws`.
fn install_custom_leader(ws: &Path) {
    let agents = ws.join(".codemason/agents");
    std::fs::create_dir_all(&agents).expect("agents dir");
    std::fs::write(
        agents.join("review-boss.md"),
        "---\n{\"name\":\"review-boss\",\"description\":\"Leads reviews\",\
         \"role\":\"team-leader\"}\n---\nLead the review team.\n",
    )
    .expect("write agent");
}

#[tokio::test]
async fn test_sidebar_team_rows_done_leader_listed_after_restart() {
    let ws = workspace("done_restart");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    let _ = fresh_rows(&mut app);
    // The leader cleaned up (its directory is gone) and finished.
    std::fs::remove_dir_all(&a.team_dir).expect("remove team dir");
    app.active_tasks.clear();
    let _ = fresh_rows(&mut app);

    let mut fresh = support::make_app_with_storage(Arc::clone(&app.storage));
    drop(app);
    pin_to(&mut fresh, &ws);
    let rows = fresh_rows(&mut fresh);
    assert!(
        matches!(
            leader_row(&rows, &a.session),
            Some(SidebarRow::TeamLeader { state: LeaderState::Done, team_name, .. })
                if team_name == "alpha"
        ),
        "rows: {rows:?}"
    );
    assert_eq!(teammate_rows(&rows), 0, "rows: {rows:?}");
}

#[tokio::test]
async fn test_sidebar_team_rows_custom_leader_resumes_as_itself_after_restart() {
    let ws = workspace("custom_resume");
    install_custom_leader(&ws);
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &[]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "review-boss"));
    let _ = fresh_rows(&mut app);

    // Restart: the leader's task is gone, its team still Active (crashed).
    let mut fresh = support::make_app_with_storage(Arc::clone(&app.storage));
    drop(app);
    pin_to(&mut fresh, &ws);
    let agents = support::attach_leader_runtime(&mut fresh);
    let rows = fresh_rows(&mut fresh);
    let at = rows
        .iter()
        .position(|r| {
            matches!(r, SidebarRow::TeamLeader { session_id, state: LeaderState::Crashed, .. }
                if *session_id == a.session)
        })
        .expect("crashed leader row");
    click_row(&mut fresh, at, false);

    submit(&mut fresh, "status?").await;
    drain_resumes(&mut fresh, &a.session).await;
    let runs: Vec<String> = agents
        .tasks_snapshot()
        .await
        .into_iter()
        .filter(|t| t.child_session_id == a.session)
        .map(|t| t.agent_name)
        .collect();
    assert_eq!(runs, ["review-boss"], "status: {}", fresh.status);
}

#[tokio::test]
async fn test_sidebar_team_rows_prompt_to_ended_teammate_refused() {
    let ws = workspace("ended_mate");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    let rows = fresh_rows(&mut app);
    let at = rows
        .iter()
        .position(|r| matches!(r, SidebarRow::Teammate { .. }))
        .expect("teammate row");
    click_row(&mut app, at, false);
    // The team is cleaned up while the teammate's session stays open.
    std::fs::remove_dir_all(&a.team_dir).expect("remove team dir");
    let _ = fresh_rows(&mut app);
    assert_eq!(
        app.session.session_id.as_deref(),
        Some(a.teammates[0].as_str())
    );

    submit(&mut app, "go").await;

    assert_eq!(app.status, "[error] this teammate's team has ended");
    assert_eq!(app.session.input, "go");
    assert!(!app.session.is_processing, "no turn was started");
}

/// A team refresh before the session list has loaded must not prune the
/// recorded leaders: none of their parents is listed yet only because
/// nothing is.
#[tokio::test]
async fn test_sidebar_team_rows_records_kept_until_sessions_load() {
    let ws = workspace("records_kept");
    let (mut app, primary) = app_with_primary(&ws);
    let a = add_leader(&mut app, &ws, &primary, "alpha", &["rev-1"]);
    app.active_tasks
        .push(running_task(&primary, &a.session, "team-leader"));
    let _ = fresh_rows(&mut app);
    let recorded = app
        .storage
        .get_setting("team.leaders")
        .expect("read")
        .expect("leaders recorded");
    assert!(recorded.contains(&a.session), "recorded: {recorded}");

    // A fresh App over the same database whose session list has not loaded
    // (or failed to load): only the team rows refresh.
    let mut fresh = support::make_app_with_storage(Arc::clone(&app.storage));
    drop(app);
    pin_to(&mut fresh, &ws);
    fresh.sidebar.dirty = false;
    fresh.sidebar.teams_dirty = true;
    refresh_if_dirty(&mut fresh);

    let after = fresh
        .storage
        .get_setting("team.leaders")
        .expect("read")
        .expect("leaders still recorded");
    assert!(after.contains(&a.session), "recorded: {after}");
}
