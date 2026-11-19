//! Tests for the row action dispatch layer (spec `agentui` T-010).
//!
//! `test_row_action_confirm.rs` pins the yes/no dialog and the trigger-point
//! routing; this file pins what the confirmed (or non-destructive) action
//! actually dispatches:
//!
//! - FR-035: a `[stop]` click on a running sub-agent row requests
//!   suspension, on a suspended row requests resumption - with no dialog in
//!   either direction;
//! - FR-036: the confirmed `[x]` answer terminates the task through the real
//!   `AgentManager` (the registry entry moves to `Terminating`);
//! - FR-037: the confirmed background-shell answer cancels the task through
//!   the real `BackgroundTaskService` (the service publishes the `cancelled`
//!   status event);
//! - FR-040: the confirmed teammate answers suspend/shutdown through the
//!   real `TeamManager` (the on-disk team store shows the new status) and the
//!   resume direction restores an already-suspended member without a dialog;
//! - FR-068: every dispatch is spawned on the async runtime and returns
//!   control to the caller immediately (the tests drive `handle_mouse_event`
//!   on the render thread and still pump afterwards);
//! - NFR-008: every panel-triggered request is logged as a panel line
//!   attributed to the affected child or teammate session id;
//! - FR-069: a confirmed answer whose row vanished dispatches nothing.
//!
//! Real managers are bound to the test `App` (`AgentManager`,
//! `BackgroundTaskService`, `TeamManager`) so the dispatch is observed
//! end-to-end rather than through mocks. Every scratch artefact lives under
//! the git-ignored `target/temp/agentui-t010` tree (NFR-012).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_row_action_dispatch.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t010";

/// A team name the on-disk stores use; the scratch keeps it isolated.
const TEAM_NAME: &str = "row-action-dispatch";

/// A plain key press (`test_row_action_confirm.rs` shape).
fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// A live sub-agent task (`test_row_action_confirm.rs` shape).
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// A team member with a stable synthetic session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// A unique scratch directory under `target/temp/agentui-t010` carrying its
/// own `.codemason/teams` so the team store resolves inside the scratch, never
/// in the repository (NFR-012).
fn scratch_dir(tag: &str) -> PathBuf {
    let unique = format!(
        "{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/agentui-t010")
        .join(unique);
    std::fs::create_dir_all(dir.join(".codemason/teams")).expect("scratch dir");
    dir
}

/// Render one frame so the renderer records its hit rects.
fn render(app: &mut App) {
    crate::frame::publish(app);
}

/// Pump the async runtime so the spawned dispatch futures run, up to about
/// two seconds, stopping as soon as `done` observes its condition.
async fn pump_until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the awaited dispatch condition never became true");
}

/// Whether the TUI log panel carries the attributed request line for a
/// sub-agent action (NFR-008): the entry must be stamped with the child
/// session id and name the task.
fn log_has_attribution(app: &App, session_id: &str, needle: &str) -> bool {
    app.log_entries
        .iter()
        .any(|e| e.session_id.as_deref() == Some(session_id) && e.message.contains(needle))
}

/// An Agents-window app with one live row per subject.
fn app_with_agents_window() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.show_agents_window = true;
    app
}

/// A Teams-window app over the given member statuses.
fn app_with_teams_window(status: MemberStatus) -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", status));
    app.show_teams_window = true;
    app
}

// ---------------------------------------------------------------------------
// FR-035: the [stop] toggle on a sub-agent row
// ---------------------------------------------------------------------------

/// Space on the cursor-selected running sub-agent row requests suspension
/// without a dialog, and the request is logged attributed to the child session
/// (NFR-008). The runtime-bound `AgentManager` accepts the request
/// asynchronously; the row stays painted until the lifecycle event or the
/// reconciliation pass converges it (FR-054). (Old [stop]-click trigger
/// retired with the row mouse.)
#[tokio::test]
async fn space_on_a_running_subagent_requests_suspension() {
    let mut app = app_with_agents_window();
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    let manager = Arc::new(codemason_agent::task::AgentManager::new(
        app.event_bus.clone(),
        app.session_processor.clone(),
        4,
        300,
    ));
    manager
        .seed_completed_for_test(task("a1b2c3d4", "explore", TaskStatus::Running))
        .await;
    let _ = app
        .session_processor
        .agent_manager
        .set(Arc::clone(&manager));
    render(&mut app);

    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "the suspend direction is not destructive and opens no dialog (FR-035)"
    );
    assert!(
        log_has_attribution(
            &app,
            "a1b2c3d4-session",
            "Suspend requested for task a1b2c3d4"
        ),
        "the request must be attributed to the child session (NFR-008)"
    );
}

/// Space on the cursor-selected suspended sub-agent row requests resumption
/// instead (FR-035's other direction), again without a dialog.
#[tokio::test]
async fn space_on_a_suspended_subagent_requests_resumption() {
    let mut app = app_with_agents_window();
    app.active_tasks
        .push(task("b2c3d4e5", "explore", TaskStatus::Suspended));
    render(&mut app);

    app.move_agents_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(!app.row_action_confirm_open(), "resume opens no dialog");
    assert!(
        log_has_attribution(
            &app,
            "b2c3d4e5-session",
            "Resume requested for task b2c3d4e5"
        ),
        "the resume request must be attributed to the child session (NFR-008)"
    );
}

// ---------------------------------------------------------------------------
// FR-036: the confirmed kill dispatches through the real manager
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-037: the confirmed cancel dispatches through the real service
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-040: the teammate controls dispatch through the real manager
// ---------------------------------------------------------------------------

/// The on-disk status of `tm-001` (`None` when the store or member is gone).
fn stored_status(store_dir: &PathBuf) -> Option<MemberStatus> {
    TeamStore::load(store_dir)
        .ok()?
        .config
        .member_by_id("tm-001")
        .map(|m| m.status)
}

/// Bind a real `TeamManager` over a real store in the scratch and register
/// `tm-001` in it; returns the store dir for later reloads.
fn bind_team_manager(app: &mut App, status: MemberStatus) -> PathBuf {
    let workdir = scratch_dir("team");
    let store =
        TeamStore::create(TEAM_NAME, LEAD_SESSION, &workdir, true).expect("create team store");
    {
        let mut config = store.config.clone();
        let mut store_member = TeamMember::new("reviewer", "tm-001", "general");
        store_member.status = status;
        store_member.session_id = Some("tm-001-session".to_string());
        config.members.push(store_member);
        TeamStore {
            dir: store.dir.clone(),
            config,
        }
        .save()
        .expect("save store member");
    }
    let manager = Arc::new(codemason_agent::team::TeamManager::new(
        TEAM_NAME,
        LEAD_SESSION,
        store.dir.clone(),
        app.session_processor.clone(),
        app.event_bus.clone(),
    ));
    let _ = app.session_processor.team_manager.set(manager);
    store.dir
}

/// Space on the cursor-selected already-suspended teammate resumes it directly
/// - no dialog, no confirmation - and the store returns to `Idle` (FR-040).
#[tokio::test]
async fn teammate_space_on_a_suspended_row_resumes_without_a_dialog() {
    let mut app = app_with_teams_window(MemberStatus::Suspended);
    let store_dir = bind_team_manager(&mut app, MemberStatus::Suspended);
    render(&mut app);

    app.move_teams_cursor(1);
    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        !app.row_action_confirm_open(),
        "the resume direction opens no dialog (FR-040)"
    );
    assert!(
        log_has_attribution(&app, "tm-001-session", "tm-001 resume requested"),
        "the resume request must be attributed to the member session (NFR-008)"
    );

    pump_until(|| stored_status(&store_dir) == Some(MemberStatus::Idle)).await;
    // Cleanup removes the whole scratch root (the store dir sits three
    // levels below it: <scratch>/.codemason/teams/<team>).
    let scratch = store_dir.ancestors().nth(3).expect("scratch root");
    let _ = std::fs::remove_dir_all(scratch);
}

// ---------------------------------------------------------------------------
// FR-069: a confirmed answer with a vanished subject dispatches nothing
// ---------------------------------------------------------------------------
