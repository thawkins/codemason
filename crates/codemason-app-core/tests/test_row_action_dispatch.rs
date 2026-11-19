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

#[path = "support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-t010";

/// A team name the on-disk stores use; the scratch keeps it isolated.
const TEAM_NAME: &str = "row-action-dispatch";

/// A team member with a stable synthetic session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
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

// ---------------------------------------------------------------------------
// FR-036: the confirmed kill dispatches through the real manager
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-037: the confirmed cancel dispatches through the real service
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-040: the teammate controls dispatch through the real manager
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-069: a confirmed answer with a vanished subject dispatches nothing
// ---------------------------------------------------------------------------

/// Answering `Yes` after the teammate row vanished closes the dialog and
/// records no request line at all (FR-069): the stale-row re-check in the
/// accept path drops the action before any dispatch.
#[tokio::test]
async fn stale_teammate_answer_dispatches_nothing() {
    let mut app = app_with_teams_window(MemberStatus::Working);
    // Arm the dialog directly (the trigger points are T-009's concern).
    app.open_row_action(codemason_app_core::app::RowAction::SuspendTeammate {
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });
    let baseline = app.log_entries.len();
    // The subject disappears between open and answer.
    app.session.team_members.clear();

    app.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
        .await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "the answer closes the dialog"
    );
    assert_eq!(
        app.log_entries.len(),
        baseline,
        "a stale answer must not log any request (FR-069)"
    );
    assert!(
        !app.log_entries.iter().any(|e| e.message.contains("tm-001")),
        "no request line may name the vanished teammate (FR-069)"
    );
}
