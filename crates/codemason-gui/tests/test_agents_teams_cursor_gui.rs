//! Focused coverage for the Agents / Teams window block cursors (spec
//! `agentui` T-011's keyboard-first rework).
//!
//! The popup windows are keyboard-driven through a block cursor, so this file
//! pins the cursor navigation and action helpers directly:
//!
//! - Up/Down/PgUp/PgDn navigation: a first press starts from the top (or the
//!   bottom when moving up), the cursor clamps at both ends, and pages move
//!   one viewport at a time (FR-034, FR-038);
//! - Agents Space/Del/Enter: the suspend toggle, the kill dialog and the
//!   output overlay (FR-035, FR-036, FR-034);
//! - Teams Space/Del/Enter: the suspend-confirm / direct-resume split, the
//!   shutdown dialog, and the focus + TeamMember overlay (FR-040, FR-038);
//! - stale cursors are consumed no-ops that report on the status line
//!   (FR-069);
//! - scroll sync: moving the cursor down past the visible edge adjusts the
//!   table offset so the row paints (FR-034, FR-038).
//!
//! Every case drives the real helpers on the test `App` (`make_app` state,
//! nothing written into the repository, NFR-012).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_agents_teams_cursor.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;
use codemason_app_core::app::OutputViewTarget;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-cursor";

/// A live sub-agent task.
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

/// A teammate in `status` with a spawned session.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render one frame with the Agents window open, publishing the row maps.
fn render_agents(app: &mut App, _width: u16, _height: u16) {
    app.show_agents_window = true;
    crate::frame::publish(app);
}

/// Render one frame with the Teams window open, publishing the row maps.
fn render_teams(app: &mut App, _width: u16, _height: u16) {
    app.show_teams_window = true;
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// Navigation: start, clamp, page
// ---------------------------------------------------------------------------

/// A first Down starts at row 0; continuing Down past both ends clamps, and Up
/// from no cursor starts at the last row (FR-034).
#[test]
fn agents_cursor_starts_clamps_and_pages() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    for i in 0..6 {
        app.active_tasks
            .push(task(&format!("task-{i}"), "explore", TaskStatus::Running));
    }
    render_agents(&mut app, 160, 44);

    // Down from None starts at 0.
    app.move_agents_cursor(1);
    assert_eq!(app.agents_cursor_subagent, Some(0));
    // Down at the last row clamps.
    for _ in 0..40 {
        app.move_agents_cursor(1);
    }
    assert_eq!(app.agents_cursor_subagent, Some(5));
    // Up past the top clamps.
    for _ in 0..40 {
        app.move_agents_cursor(-1);
    }
    assert_eq!(app.agents_cursor_subagent, Some(0));
    // Up from None starts at the last row.
    let mut app2 = make_app();
    app2.session.session_id = Some(LEAD_SESSION.to_string());
    app2.session.agent_name = "general".to_string();
    for i in 0..6 {
        app2.active_tasks
            .push(task(&format!("task-{i}"), "explore", TaskStatus::Running));
    }
    render_agents(&mut app2, 160, 44);
    app2.move_agents_cursor(-1);
    assert_eq!(app2.agents_cursor_subagent, Some(5));
}

/// PgDn on the Teams cursor pages one viewport at a time and clamps at the
/// last teammate; a no-cursor PgDn starts at the paged row (FR-038).
#[test]
fn teams_cursor_pages_and_clamps() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    for i in 0..10 {
        app.session.team_members.push(member(
            &format!("tm-{i:03}"),
            &format!("member-{i}"),
            MemberStatus::Idle,
        ));
    }
    render_teams(&mut app, 170, 44);

    // One page from the top must move the cursor at least one row.
    app.page_teams_cursor(1);
    let first_page = app.teams_cursor_teammate.expect("cursor set");
    assert!(
        first_page >= 1,
        "one page must move the cursor: {first_page}"
    );

    // Paging down repeatedly clamps at the last teammate row.
    for _ in 0..10 {
        app.page_teams_cursor(1);
    }
    assert_eq!(app.teams_cursor_teammate, Some(9));

    // Paging up repeatedly clamps at the first teammate row.
    for _ in 0..10 {
        app.page_teams_cursor(-1);
    }
    assert_eq!(app.teams_cursor_teammate, Some(0));
}

// ---------------------------------------------------------------------------
// Actions: Space / Del / Enter on the Agents cursor
// ---------------------------------------------------------------------------

/// Space suspends the cursor row's running sub-agent (the request line is
/// attributed, NFR-008) and toggles to resume for a suspended one.
#[tokio::test]
async fn agents_space_suspends_and_resumes() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("run-0001", "explore", TaskStatus::Running));
    render_agents(&mut app, 160, 44);

    app.move_agents_cursor(1);
    app.agents_cursor_toggle_stop_resume();

    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Suspend requested for task run-0001")),
        "Space on a running row must request suspension, got {:?}",
        app.log_entries.last().map(|e| e.message.clone())
    );
    assert!(
        !app.row_action_confirm_open(),
        "the suspend direction is not destructive (FR-035)"
    );

    // Now the suspended direction toggles to resume.
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("susp0002", "explore", TaskStatus::Suspended));
    render_agents(&mut app, 160, 44);
    app.move_agents_cursor(1);
    app.agents_cursor_toggle_stop_resume();

    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Resume requested for task susp0002")),
        "Space on a suspended row must request resumption, got {:?}",
        app.log_entries.last().map(|e| e.message.clone())
    );
}

/// Del arms the KillSubagent dialog naming the cursor row's task (FR-036).
#[tokio::test]
async fn agents_del_arms_the_kill_dialog() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("kill0001", "rust-coder", TaskStatus::Running));
    render_agents(&mut app, 160, 44);

    app.move_agents_cursor(1);
    app.agents_cursor_kill();

    assert!(app.row_action_confirm_open(), "Del confirms the kill");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(codemason_app_core::app::RowAction::KillSubagent {
            task_id,
            agent_name,
        }) => {
            assert_eq!(task_id, "kill0001");
            assert_eq!(agent_name, "rust-coder");
        }
        other => panic!("expected KillSubagent, got {other:?}"),
    }

    // Esc dismisses and touches nothing.
    app.row_action_confirm_cancel();
    assert!(!app.row_action_confirm_open());
    assert_eq!(
        app.active_tasks[0].status,
        TaskStatus::Running,
        "cancelling the dialog leaves the row untouched (FR-069)"
    );
}

/// A stale cursor (no task behind the ordinal) is a consumed no-op that
/// reports on the status line and dispatches nothing (FR-069).
#[tokio::test]
async fn agents_stale_cursor_actions_are_consumed_no_ops() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("gone0001", "explore", TaskStatus::Running));
    render_agents(&mut app, 160, 44);

    // The cursor survives but the row is gone (the task list changed under
    // the painted frame, FR-069).
    app.agents_cursor_subagent = Some(0);
    app.active_tasks.clear();

    app.agents_cursor_toggle_stop_resume();
    assert_eq!(
        app.status, "agents: selected row is gone",
        "Space reports the stale cursor (FR-069)"
    );
    app.agents_cursor_kill();
    assert!(
        !app.row_action_confirm_open(),
        "Del on a stale cursor arms no dialog"
    );
    app.status = String::new();
    app.agents_cursor_open_output();
    assert!(
        app.output_view.is_none(),
        "Enter on a stale cursor opens nothing"
    );
    assert_eq!(
        app.status, "agents: selected row is gone",
        "Enter reports the stale cursor (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// Actions: Space / Del / Enter on the Teams cursor
// ---------------------------------------------------------------------------

/// Space on a working teammate arms the suspend dialog; Space on a suspended
/// one resumes directly (FR-040's direction split).
#[tokio::test]
async fn teams_space_splits_by_member_status() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    render_teams(&mut app, 170, 44);

    app.move_teams_cursor(1);
    app.teams_cursor_toggle_stop_resume();

    assert!(
        app.row_action_confirm_open(),
        "a live member confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(codemason_app_core::app::RowAction::SuspendTeammate { agent_id, name, .. }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate, got {other:?}"),
    }
    app.row_action_confirm_cancel();

    // The suspended direction resumes directly.
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Suspended));
    render_teams(&mut app, 170, 44);
    app.move_teams_cursor(1);
    app.teams_cursor_toggle_stop_resume();

    assert!(
        !app.row_action_confirm_open(),
        "a resume needs no dialog (FR-040)"
    );
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("resume requested")),
        "the resume must dispatch, got {:?}",
        app.log_entries.last().map(|e| e.message.clone())
    );
}

/// Del arms the ShutdownTeammate dialog naming the cursor row's member
/// (FR-040).
#[tokio::test]
async fn teams_del_arms_the_shutdown_dialog() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    render_teams(&mut app, 170, 44);

    app.move_teams_cursor(1);
    app.teams_cursor_shutdown();

    assert!(app.row_action_confirm_open(), "Del confirms the shutdown");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(codemason_app_core::app::RowAction::ShutdownTeammate { agent_id, name, .. }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected ShutdownTeammate, got {other:?}"),
    }
}

/// Enter focuses the cursor row's teammate and opens its output overlay as a
/// TeamMember view (FR-038).
#[test]
fn teams_enter_focuses_and_opens_the_overlay() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    render_teams(&mut app, 170, 44);

    app.move_teams_cursor(1);
    app.teams_cursor_focus();

    assert_eq!(
        app.session
            .focused_teammate
            .as_ref()
            .map(|(_, id)| id.as_str()),
        Some("tm-001"),
        "Enter focuses the teammate (FR-038)"
    );
    assert_eq!(
        app.status, "team: focused -> reviewer",
        "the focus transition reports to the status line (FR-064)"
    );
    let view = app.output_view.as_ref().expect("the overlay must open");
    match &view.target {
        OutputViewTarget::TeamMember {
            agent_id,
            teammate_name,
            ..
        } => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(teammate_name, "reviewer");
        }
        other => panic!("expected a TeamMember target, got {other:?}"),
    }
}

/// Stale team cursors are consumed no-ops that report on the status line
/// (FR-069).
#[tokio::test]
async fn teams_stale_cursor_actions_are_consumed_no_ops() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    render_teams(&mut app, 170, 44);

    app.teams_cursor_teammate = Some(0);
    app.session.team_members.clear();

    app.teams_cursor_toggle_stop_resume();
    assert_eq!(
        app.status, "teams: selected row is gone",
        "Space reports the stale cursor (FR-069)"
    );
    app.teams_cursor_shutdown();
    assert!(
        !app.row_action_confirm_open(),
        "Del on a stale cursor arms no dialog"
    );
    app.status = "sentinel".to_string();
    app.teams_cursor_focus();
    assert!(
        app.session.focused_teammate.is_none(),
        "Enter focuses nothing"
    );
    assert!(app.output_view.is_none(), "Enter opens nothing");
    assert_eq!(
        app.status, "sentinel",
        "a stale focus path mutates nothing beyond the status untouched (FR-069)"
    );
}

/// A stale cursor whose ordinal is beyond the published ids reports the
/// consumed no-op on the status line (FR-069).
#[tokio::test]
async fn teams_out_of_range_cursor_reports_the_no_op() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("cursor-team", LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    render_teams(&mut app, 170, 44);

    app.teams_cursor_teammate = Some(9); // beyond the painted teammate rows
    app.teams_cursor_toggle_stop_resume();
    assert_eq!(
        app.status, "teams: selected row is gone",
        "Space beyond the range reports the consumed no-op (FR-069)"
    );
    assert!(!app.row_action_confirm_open());
}

// ---------------------------------------------------------------------------
// Scroll sync: the cursor stays visible
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Mouse click-to-select (restored with the block cursor)
// ---------------------------------------------------------------------------

/// A left click on a sub-agent row moves the block cursor onto that row, so
/// the click-then-Space flow works again after the keyboard-first rework: the
/// click only SELECTS, Space/Del/Enter remain the single action surface.
#[tokio::test]
async fn agents_click_selects_then_space_toggles() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("clck0001", "explore", TaskStatus::Running));
    render_agents(&mut app, 160, 44);

    // Rows 0/1 are the header and the primary row; the first sub-agent row
    // paints at absolute row 2.
    assert!(
        app.agents_cursor_from_click(2),
        "a click on the sub-agent row is handled"
    );
    assert_eq!(
        app.agents_cursor_subagent,
        Some(0),
        "the click moves the cursor onto the clicked row"
    );

    app.agents_cursor_toggle_stop_resume();
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Suspend requested for task clck0001")),
        "Space after a click-select must request suspension"
    );
}

/// A click on the primary row, the header, the background section or below the
/// painted rows never moves the cursor (FR-067: the lead is not addressable).
#[tokio::test]
async fn agents_click_on_non_subagent_rows_never_selects() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("clck0002", "explore", TaskStatus::Running));
    render_agents(&mut app, 160, 44);

    // Header row: consumed, cursor untouched.
    assert!(app.agents_cursor_from_click(0));
    assert_eq!(app.agents_cursor_subagent, None);
    // Primary row: consumed, cursor untouched (FR-067).
    assert!(app.agents_cursor_from_click(1));
    assert_eq!(app.agents_cursor_subagent, None);
    // Beyond the painted rows: bare consume.
    assert!(!app.agents_cursor_from_click(999));
    assert_eq!(app.agents_cursor_subagent, None);
}

/// A left click on a teammate row moves the Teams cursor onto that row; the
/// lead row is consumed without selection (FR-067).
#[tokio::test]
async fn teams_click_selects_the_clicked_member_row() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.active_team = Some(codemason_agent::team::TeamConfig::new(
        "agentui-click-team",
        LEAD_SESSION,
    ));
    app.session.team_members.push(member(
        "tm-001",
        "reviewer",
        codemason_agent::team::MemberStatus::Working,
    ));
    render_teams(&mut app, 160, 44);

    // Row 0 is the team title, row 2 the lead (no selection), row 3 the
    // first member (row 1 is the column header).
    assert!(app.teams_cursor_from_click(2));
    assert_eq!(
        app.teams_cursor_teammate, None,
        "the lead is not addressable"
    );
    assert!(
        app.teams_cursor_from_click(3),
        "a click on the member row is handled"
    );
    assert_eq!(app.teams_cursor_teammate, Some(0));
}

/// A teammate waiting on a plan paints a `plan awaiting approval:` line
/// under its row; that line is not a cursor row, so Down from the planning
/// member lands on the next teammate.
#[test]
fn teams_cursor_skips_the_plan_line_of_a_planning_member() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.team_members = vec![
        member("agent-plan", "planner", MemberStatus::PlanPending),
        member("agent-work", "worker", MemberStatus::Working),
    ];
    render_teams(&mut app, 200, 40);
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            (String::new(), "agent-plan".to_string()),
            (String::new(), "agent-work".to_string()),
        ]
    );

    app.move_teams_cursor(1);
    app.move_teams_cursor(1);
    assert_eq!(app.teams_cursor_teammate, Some(1));
    assert_eq!(
        app.teams_teammate_agent_ids
            .get(1)
            .map(|(_, id)| id.as_str()),
        Some("agent-work")
    );
}
