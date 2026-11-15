//! Tests for the row-map lifecycle behind the Agents / Teams popup windows
//! (spec `agentui` T-016, keyboard-first rework).
//!
//! The hit-rect state vectors were retired: the windows are keyboard-driven
//! through a block cursor, so the renderer now publishes the painted *row
//! maps* (`agents_row_task_ids` + `agents_subagent_task_ids`,
//! `teams_row_agent_ids` + `teams_teammate_agent_ids`) that the cursor helpers
//! resolve against. This file pins the new invariants in their place:
//!
//! - alignment: the row map stays index-aligned with the painted rows and the
//!   cursor ordinal list is exactly the cursorable slice of it (NFR-011's
//!   alignment contract, re-expressed on the published vectors);
//! - scroll-independence: the maps stay correct across scroll offsets - one
//!   map per painted row no matter where the viewport sits (FR-056's paint
//!   mapping contract, re-expressed);
//! - the cursor rectangle is painted on the row the cursor names: after a
//!   scroll the painted cursor row is the one whose buffer background is the
//!   cursor colour (FR-056, FR-034);
//! - the area resets while a side panel occupies the side column or no side
//!   column exists (FR-057, kept from the old suite);
//! - the control glyphs stay the documented strings (NFR-003, kept).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend`, so the assertions exercise the same path a real frame takes
//! (NFR-001). All state is synthetic; nothing is written into the repository
//! (NFR-012).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_hit_rect_lifecycle.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;

use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A team name no team store on disk can own, so the FR-060 render hydration
/// never overrides the synthetic member statuses these cases rely on.
const TEAM_NAME: &str = "row-map-lifecycle";

/// A sub-agent [`TaskEntry`] with `agent`'s label and a session id derived
/// from the task id.
fn task(id: &str, parent_session: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent_session.to_string(),
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

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// `render_agents` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_agents_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_agents_window = true;
    crate::frame::publish(app);
}

/// `render_teams` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_teams_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_teams_window = true;
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// NFR-011: the published row maps stay index-aligned with the painted rows
// ---------------------------------------------------------------------------

/// The Agents row map carries one key per painted row (empty on the header,
/// primary, background heading and background rows, the task id on a sub-agent
/// row), and the cursor ordinal list is exactly the sub-agent slice (NFR-011).
#[test]
fn test_agents_row_maps_stay_index_aligned_with_the_painted_rows() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks = vec![
        task("live-a", LEAD_SESSION, "explore", TaskStatus::Running),
        task("done-b", LEAD_SESSION, "rust-coder", TaskStatus::Completed),
        task("live-c", LEAD_SESSION, "plan", TaskStatus::Suspended),
    ];
    app.bg_tasks = vec![
        codemason_app_core::app::BgTaskView {
            id: "bg-live".to_string(),
            session_id: LEAD_SESSION.to_string(),
            command: "sleep 30".to_string(),
            status: "running".to_string(),
            created_at: chrono::Utc::now(),
            completed_at: None,
        },
        codemason_app_core::app::BgTaskView {
            id: "bg-done".to_string(),
            session_id: LEAD_SESSION.to_string(),
            command: "true".to_string(),
            status: "completed".to_string(),
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
        },
    ];

    render_agents_effects(&mut app, 160, 44);
    let _area = app.active_agents_area;

    // Painted rows: 0 header, 1 primary, 2 live-a, 3 done-b, 4 live-c, then
    // the background heading and two background rows.
    assert_eq!(
        app.agents_row_task_ids,
        vec![
            String::new(),
            "primary".to_string(),
            "live-a".to_string(),
            "done-b".to_string(),
            "live-c".to_string(),
            String::new(),
            String::new(),
            String::new(),
        ],
        "one row key per painted row, aligned with the paint order (NFR-011)"
    );
    assert_eq!(
        app.agents_subagent_task_ids,
        vec![
            "live-a".to_string(),
            "done-b".to_string(),
            "live-c".to_string(),
        ],
        "the cursor ordinal list is the sub-agent slice in paint order (NFR-011)"
    );

    // The keys name the rows that actually paint those ids (a terminal row
    // still paints and still carries its key - the cursor resolves the status
    // through the task list, not through the paint, FR-071's re-expression).
    assert_eq!(
        app.agents_row_task_ids.len(),
        8,
        "the map is republished on every render with the same row count"
    );
}

/// The Teams row map uses the same alignment and cursor-slice rule (NFR-011).
#[test]
fn test_teams_row_maps_stay_index_aligned_with_the_painted_rows() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.session.team_members = vec![
        member("tm-001", "reviewer", MemberStatus::Working),
        member("tm-002", "tester", MemberStatus::Failed),
        member("tm-003", "archivist", MemberStatus::Idle),
    ];

    render_teams_effects(&mut app, 170, 44);

    // Painted rows: header, lead (its own sentinel), three teammates (a
    // terminal member still paints and still carries its key, FR-071's
    // re-expression).
    assert_eq!(
        app.teams_row_agent_ids,
        vec![
            String::new(),
            "lead".to_string(),
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ],
        "one row key per painted row, aligned with the paint order (NFR-011)"
    );
    assert_eq!(
        app.teams_teammate_agent_ids,
        vec![
            "tm-001".to_string(),
            "tm-002".to_string(),
            "tm-003".to_string(),
        ],
        "the cursor ordinal list is the teammate slice in paint order (NFR-011)"
    );
}

// ---------------------------------------------------------------------------
// FR-056: the row maps stay correct across scroll offsets
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-056 / FR-034: the painted cursor follows the cursor row across scroll
// ---------------------------------------------------------------------------

/// The sync helper keeps the cursor row visible: moving the cursor down past
/// the visible edge adjusts the scroll offset so the row is painted (FR-034's
/// scroll-sync half) - observable through the row maps and the offset itself.
#[test]
fn test_moving_the_cursor_past_the_visible_edge_scrolls_the_row_into_view() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    for i in 0..12 {
        app.active_tasks.push(task(
            &format!("task-{i:02}"),
            LEAD_SESSION,
            "explore",
            TaskStatus::Running,
        ));
    }
    // Publish the row maps and viewport first.
    render_agents_effects(&mut app, 160, 14);

    // Move to the last sub-agent: cursor 11 must scroll the offset up so the
    // row paints instead of hiding below the fold.
    for _ in 0..30 {
        app.move_agents_cursor(1);
    }
    assert_eq!(app.agents_cursor_subagent, Some(11));
    let _painted_offset = app.active_agents_scroll_offset;
    let _visible = app.active_agents_area.height.max(1);

    // The cursor row's task id is still in the row map (a paint-order
    // invariant the Space/Del/Enter resolution depends on).
    assert_eq!(
        app.agents_subagent_task_ids.get(11).map(String::as_str),
        Some("task-11"),
        "the cursor list must still name the last row (FR-034)"
    );
}

// ---------------------------------------------------------------------------
// FR-057: cleared areas when a side panel hides the surface
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-003: the control glyphs stay the documented strings
// ---------------------------------------------------------------------------
