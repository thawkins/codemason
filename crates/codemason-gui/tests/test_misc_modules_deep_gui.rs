//! Deep coverage for the smaller `codemason-tui` modules: the tracing layer
//! that routes log records to the TUI log panel, the markdown worker, the `/toolchain` report engine, the team
//! blueprint helpers, the clipboard temp-file lifecycle, the layout utilities,
//! the execution-backend switcher panel, the graphify skill generator, the
//! goal-loop setup dialog, the cron scheduler, and the layout render branches
//! for the agents/teams/status-bar panels.
//!
//! Tests only - no production logic is touched. Writes are confined to
//! `target/temp/` or a `tempfile::TempDir`; the slash-command tests run inside
//! an isolated tempdir project (cwd + `XDG_CONFIG_HOME` redirected) so nothing
//! lands in the developer's real `~/.config/codemason/`.
//!
//! Env mutation is `unsafe` in edition 2024 and the workspace denies
//! `unsafe_code`, so this target opts back in explicitly for the isolation
//! harness; that harness holds a process-global mutex across the async body,
//! so `clippy::await_holding_lock` is also allowed for this file.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_misc_modules_deep.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use chrono::Utc;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;

#[path = "../../codemason-app-core/tests/slash_support/mod.rs"]
mod slash_support;
#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

// ===========================================================================
// Helpers
// ===========================================================================

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

/// Build a [`TaskEntry`] for the active-agents panel.
fn task_entry(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

// ===========================================================================
// tracing_layer
// ===========================================================================

// ===========================================================================
// md_worker
// ===========================================================================

// ===========================================================================
// toolchain
// ===========================================================================

// ===========================================================================
// blueprints
// ===========================================================================

// ===========================================================================
// helpers
// ===========================================================================

// ===========================================================================
// utils
// ===========================================================================

// ===========================================================================
// clipboard
// ===========================================================================

// ===========================================================================
// backend_panel
// ===========================================================================

// ===========================================================================
// skillgen
// ===========================================================================

// ===========================================================================
// loop_dialog
// ===========================================================================

// ===========================================================================
// cron scheduler
// ===========================================================================

// ===========================================================================
// Layout: agents / teams / status-bar render branches
// ===========================================================================

/// Populate the App with a two-level agent tree plus a background shell task.
fn app_with_agent_tree() -> App {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks.push(task_entry(
        "task-1",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        false,
    ));
    app.active_tasks.push(task_entry(
        "task-2",
        "s2",
        "s3",
        "build",
        TaskStatus::Suspended,
        true,
    ));
    app.active_tasks.push(task_entry(
        "task-3",
        "s1",
        "s4",
        "build",
        TaskStatus::Completed,
        false,
    ));
    app.bg_tasks.push(codemason_app_core::app::BgTaskView {
        id: "bg-1".to_string(),
        session_id: "s1".to_string(),
        command: "cargo build --workspace --all-targets".to_string(),
        status: "running".to_string(),
        created_at: Utc::now(),
        completed_at: None,
    });
    app.bg_tasks.push(codemason_app_core::app::BgTaskView {
        id: "bg-2".to_string(),
        session_id: "s1".to_string(),
        command: "ls".to_string(),
        status: "completed".to_string(),
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
    });
    app
}

#[test]
fn layout_agents_window_paints_the_tree_and_buttons() {
    let mut app = app_with_agent_tree();
    app.show_agents_window = true;
    render_effects(&mut app, 140, 44);
    // The renderer publishes the cursor row maps instead of the retired
    // hit-rect vectors: one key per painted row and the sub-agent slice for
    // the keyboard cursor (hit-rect rework).
    // Painted rows: header, lead, task-1, task-2, task-3 (completed), then the
    // background heading and bg-1/bg-2.
    assert_eq!(app.agents_row_task_ids.len(), 8);
    assert_eq!(app.agents_row_task_ids[1], "primary");
    assert_eq!(app.agents_row_task_ids[2], "task-1");
    assert_eq!(app.agents_row_task_ids[3], "task-2");
    assert_eq!(app.agents_row_task_ids[4], "task-3");
    assert_eq!(
        app.agents_subagent_task_ids,
        vec![
            "task-1".to_string(),
            "task-2".to_string(),
            "task-3".to_string(),
        ],
        "the cursor ordinal list names the sub-agent rows in paint order"
    );
}

#[test]
fn layout_agents_window_without_tasks_still_paints() {
    let mut app = support::make_app();
    app.show_agents_window = true;
    render_effects(&mut app, 120, 30);
    assert_eq!(app.active_agents_max_scroll, 0);
}

#[test]
fn layout_teams_window_paints_the_lead_and_teammates() {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app.show_teams_window = true;
    app.session.active_team = Some(TeamConfig::new("alpha", "s1"));

    let mut working = TeamMember::new("reviewer", "tm-001", "build");
    working.status = MemberStatus::Working;
    working.session_id = Some("s2".to_string());
    app.session.team_members.push(working);

    let mut suspended = TeamMember::new("tester", "tm-002", "build");
    suspended.status = MemberStatus::Suspended;
    app.session.team_members.push(suspended);

    let mut stopped = TeamMember::new("archivist", "tm-003", "build");
    stopped.status = MemberStatus::Stopped;
    app.session.team_members.push(stopped);

    app.session.focused_teammate = Some("tm-001".to_string());
    app.session
        .team_message_counts
        .insert("tm-001".to_string(), (3, 2));

    render_effects(&mut app, 160, 44);
    // The renderer publishes the cursor row maps instead of the retired
    // hit-rect vectors: one key per painted row (header, lead, three
    // teammates) and the teammate slice for the keyboard cursor (hit-rect
    // rework). A stopped member still carries its key - Space/Del on it
    // resolve (and are refused) through the member status. `tester` is
    // suspended, so the FR-028 parked-member hint row is painted too and
    // carries its own empty key.
    assert_eq!(app.teams_row_agent_ids.len(), 6);
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ]
    );
}

// ===========================================================================
// Slash-command surfaces (isolated project)
// ===========================================================================

// ===========================================================================
// Research progress (render surface the event tests do not reach)
// ===========================================================================

// ===========================================================================
// /spawn, /bench, compaction and /new poll arms
// ===========================================================================

// ===========================================================================
// helpers reached through the event handler
// (`loop_termination_banner`, `plural`, `files_preview`, `summarise_error`)
// ===========================================================================

// ===========================================================================
// Slash-command usage branches: bare form, unknown subcommand, config reads
// ===========================================================================

// ===========================================================================
// /spawn command branches and compaction start guards
// ===========================================================================

// ===========================================================================
// helpers reached through further slash surfaces
// ===========================================================================

// ===========================================================================
// status-bar abbreviation helper tables and prompt tag
// ===========================================================================
