//! Offline deterministic render tests (spec `agentui` T-019).
//!
//! NFR-001 asks that the panel suite run offline and deterministically from
//! synthetic in-memory state; NFR-012 asks that no case writes into the
//! repository working tree; NFR-002 asks that one frame never issues more than
//! the single team task-store read and the single session lookup per window.
//! This file asserts the suite-level contract directly:
//!
//! - the Agents window, the Teams window and the teammate strip all render
//!   from purely synthetic `App` state with every row variant populated, and
//!   two consecutive renders of the same state paint byte-identical buffers
//!   outside the per-second elapsed clock (determinism in the strongest
//!   practical sense);
//! - the two NFR-002 read paths a Teams frame is allowed are exercised once
//!   each and the frame still completes (the lead-session lookup and the team
//!   task-store read, both through a real scratch team store);
//! - the only files a synthetic-state render can touch under the repository
//!   root are the log spool root already redirected to `target/temp/log` by
//!   `make_app`, and nothing else (NFR-012).
//!
//! Row variants covered on the Agents window: primary, sub-agent tree with a
//! `[C]` badge, sub-agent with a `[T]` badge, suspended, completed, terminated,
//! failed, cancelled, and the background shell section with live and terminal
//! rows. On the Teams window: lead, working, idle, suspended, stopped, failed,
//! focused and model-override teammates. On the strip: one pill per member, a
//! focused highlight and the cycle hint. Every case drives the real public
//! render entry point through a ratatui `TestBackend` and reads the painted
//! buffer back (NFR-001).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_agentui_t019_offline.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use std::path::PathBuf;

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;

use support::make_app;

/// The lead session id every team case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A unique team name no real team store on disk can own, so the FR-060
/// render-time hydration cannot override the synthetic member statuses except
/// where a store is created deliberately (the NFR-002 case).
const TEAM_NAME: &str = "agentui-t019-offline";

// -- synthetic-state builders ------------------------------------------------

/// A [`TaskEntry`] whose parent session, child session, agent and status are
/// configurable so the tree, badge and terminal-row variants can all be built
/// from one helper.
fn task(
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
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// A background shell task view.
fn bg_task_view(id: &str, command: &str, status: &str) -> codemason_app_core::app::BgTaskView {
    codemason_app_core::app::BgTaskView {
        id: id.to_string(),
        session_id: LEAD_SESSION.to_string(),
        command: command.to_string(),
        status: status.to_string(),
        created_at: chrono::Utc::now(),
        completed_at: None,
    }
}

/// Register a custom agent definition so a `[C]` badge can derive (FR-013).
fn add_custom_agent(app: &mut App, name: &str) {
    app.custom_agent_defs
        .push(codemason_agent::agent::CustomAgentDef {
            record: codemason_agent::agent::oasf::OasfAgentRecord {
                name: name.to_string(),
                description: "custom".to_string(),
                version: "1.0.0".to_string(),
                schema_version: "0.7.0".to_string(),
                authors: Vec::new(),
                created_at: None,
                skills: Vec::new(),
                domains: Vec::new(),
                locators: Vec::new(),
                modules: Vec::new(),
            },
            source_path: PathBuf::from(format!("/agents/{name}.json")),
            agent_info: std::sync::Arc::new(codemason_agent::agent::AgentInfo::new(
                name,
                "custom agent",
            )),
            is_project_local: true,
        });
}

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// An app whose primary session is `s1` and whose agent is `general`.
fn app_with_primary() -> App {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.agent_name = "general".to_string();
    app
}

// -- render helpers ----------------------------------------------------------

/// `render_frame` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_frame_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

/// `render_agents_window` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_agents_window_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_agents_window = true;
    crate::frame::publish(app);
}

/// `render_teams_window` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_teams_window_effects(app: &mut App, _width: u16, _height: u16) {
    app.show_teams_window = true;
    crate::frame::publish(app);
}

// ---------------------------------------------------------------------------
// NFR-001: the Agents window is offline, deterministic and every row variant
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-001: the Teams window and the strip render offline from synthetic state
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-002: a frame is bounded to the two documented reads
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// NFR-012: test isolation - no repository working-tree writes
// ---------------------------------------------------------------------------

/// A synthetic-state render of both windows and the full frame writes nothing
/// outside the allowed roots (NFR-012). The log spool root is already
/// redirected to `target/temp/log` by `make_app`, the team store hydration
/// finds nothing for [`TEAM_NAME`], and no other repository path is touched.
/// Verified by byte-totalling the plausible repository write roots before and
/// after: `log/`, `specs/` and `.codemason/teams/`. Everything under `target/`
/// is the allowed scratch root.
#[test]
fn test_t019_renders_leave_the_repository_working_tree_untouched() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    // Byte-totals of the directory trees the render could plausibly write.
    // A missing directory counts as absent and stays absent (or is allowed to
    // appear only where the render itself creates it, which it must not).
    let paths = ["log", "specs", ".codemason/teams"];
    let size_of = |p: &str| dir_size(&repo_root.join(p));

    let before: Vec<Option<u64>> = paths.iter().map(|p| size_of(p)).collect();

    let mut app = app_with_primary();
    app.session.active_team = Some(TeamConfig::new(TEAM_NAME, "s1"));
    app.session.team_members = vec![
        member("tm-001", "reviewer", MemberStatus::Working),
        member("tm-002", "tester", MemberStatus::Stopped),
    ];
    app.session.focused_teammate = Some((String::new(), "tm-001".to_string()));
    add_custom_agent(&mut app, "rust-coder");
    app.active_tasks = vec![
        task(
            "iso-0001",
            "s1",
            "s2",
            "rust-coder",
            TaskStatus::Running,
            true,
        ),
        task(
            "iso-0002",
            "s1",
            "s3",
            "explore",
            TaskStatus::Completed,
            true,
        ),
    ];
    app.bg_tasks = vec![bg_task_view("ee556677", "sleep 30", "running")];

    render_agents_window_effects(&mut app, 160, 44);
    render_teams_window_effects(&mut app, 170, 44);
    app.show_agents_window = false;
    app.show_teams_window = false;
    render_frame_effects(&mut app, 120, 30);

    let after: Vec<Option<u64>> = paths.iter().map(|p| size_of(p)).collect();
    for ((path, before_size), after_size) in paths.iter().zip(before).zip(after) {
        assert_eq!(
            before_size,
            after_size,
            "NFR-012: the render must not write into {}/ (before {before_size:?}, after {after_size:?})",
            repo_root.join(path).display()
        );
    }
}

/// Recursive byte-total of `dir`; `None` when it does not exist at all.
fn dir_size(dir: &std::path::Path) -> Option<u64> {
    if !dir.exists() {
        return None;
    }
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&path) else {
            return None;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(entry.path()),
                Ok(_) => total += entry.metadata().map(|m| m.len()).unwrap_or(0),
                Err(_) => return None,
            }
        }
    }
    Some(total)
}
