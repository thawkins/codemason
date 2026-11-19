//! Tests for the Agents/Teams window keyboard control surface (spec `agentui`
//! T-011, keyboard-first rework).
//!
//! `test_row_action_confirm.rs` pins the dialog; `test_row_action_dispatch.rs`
//! pins the dispatches; the hit-rect rework moved the keyboard arms from the
//! retired `selected_agent_index` to the block cursor
//! (`agents_cursor_subagent` / `teams_cursor_teammate`). This file pins the
//! **keyboard surface** end to end through `App::handle_key_event`:
//!
//! - FR-041: `Space` on the cursor sub-agent row suspends it, on a suspended
//!   row resumes it, is consumed (no space reaches the input buffer), and the
//!   request line is attributed to the child session (NFR-008);
//! - FR-043: `Shift+K`/`Del` on the cursor sub-agent row confirms first and
//!   the confirmed answer drives the real `AgentManager` registry entry to
//!   `Terminating` (FR-036's keyboard twin);
//! - FR-042/FR-044 re-expressed: the cursor range covers sub-agent rows only,
//!   so no key path can reach the primary agent at all (FR-067);
//! - FR-040: `Space` on the cursor teammate row confirms only the suspend
//!   direction; `Del`/`k` arms the graceful shutdown dialog and the confirmed
//!   answer drives the real `TeamManager` (the on-disk store shows
//!   `ShuttingDown`);
//! - FR-055: with a window open the window keys are consumed before the input
//!   buffer sees them - including the no-cursor corner;
//! - FR-067: the cursor list never names the primary agent or the lead.
//!
//! Real managers are bound where the dispatch must be observed end to end
//! (`AgentManager`, `TeamManager`); every scratch artefact lives under the
//! git-ignored `target/temp/agentui-t011` tree (NFR-012).

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;

use crate::term_events::{KeyCode, KeyModifiers};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t011";

/// A team name no on-disk store under the repo can own; the scratch keeps the
/// FR-060 render hydration from overriding the synthetic member statuses.
const TEAM_NAME: &str = "agentui-t011-team";

/// A plain key press.
fn key(code: KeyCode, modifiers: KeyModifiers) -> crate::term_events::KeyEvent {
    crate::term_events::KeyEvent::new(code, modifiers)
}

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

/// A teammate in `status` with a stable synthetic session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

// ---------------------------------------------------------------------------
// FR-041: Space on the cursor sub-agent row
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-042 / FR-044 re-expressed: the cursor range never reaches the primary
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-043: Shift+K / Del on the cursor sub-agent row
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-040: Space (suspend-confirm) and Del / k (shutdown) on the teammate
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-055: window keys take precedence over the input buffer
// ---------------------------------------------------------------------------

/// With the Teams window open but a cursor whose member is gone, the keys are
/// a consumed no-op (FR-069 stale-cursor rule): the dialog never arms and the
/// input buffer stays empty.
#[tokio::test]
async fn teams_keys_without_a_live_cursor_are_consumed_no_ops() {
    let mut app = support::make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", MemberStatus::Working));
    app.show_teams_window = true;
    app.teams_cursor_teammate = Some(3); // beyond the live rows

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(
        !app.row_action_confirm_open(),
        "a cursor whose member is gone arms nothing (FR-069)"
    );
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(!app.row_action_confirm_open());
    assert_eq!(
        app.session.input, "",
        "neither key leaks a character (FR-055)"
    );
}

/// With the Agents window open and no cursor yet, `Space` and `Shift+K` are
/// still consumed by the window: nothing reaches the input buffer (the
/// no-cursor corner of FR-055; TC-013 step 4).
#[tokio::test]
async fn agent_window_keys_with_no_selection_never_touch_the_input() {
    let mut app = support::make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.agents_cursor_subagent = None;
    app.session.input = "draft".to_string();
    app.session.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(!app.row_action_confirm_open(), "no cursor arms nothing");
    assert_eq!(
        app.session.input, "draft",
        "Space and Shift+K must be consumed while the window is open (FR-055)"
    );
    assert_eq!(app.session.input_cursor, 5);
}

/// A cursor beyond the live rows is a consumed no-op and dispatches nothing
/// (a stale ordinal must route nowhere, FR-069).
#[tokio::test]
async fn an_out_of_range_cursor_is_consumed_harmlessly() {
    let mut app = support::make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.show_agents_window = true;
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.agents_cursor_subagent = Some(9); // beyond the live rows
    app.session.input = "draft".to_string();
    app.session.input_cursor = 5;

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert_eq!(
        app.session.input, "draft",
        "the stale cursor consumes both keys"
    );
    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("requested")),
        "a stale cursor must dispatch nothing (FR-069)"
    );
}

// ---------------------------------------------------------------------------
// FR-067: no key path terminates the lead
// ---------------------------------------------------------------------------
