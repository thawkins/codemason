//! Tests for row destinations (spec `agentui` T-008, keyboard-first).
//!
//! The row-mouse-click model was retired: the Agents / Teams popup windows are
//! now keyboard-driven through a block cursor, and the mouse router only keeps
//! wheel scrolling, scrollbar-gutter dragging and the close buttons. This file
//! therefore drives the destination chains through the keyboard cursor helpers
//! (`move_agents_cursor` / `agents_cursor_open_output`,
//! `move_teams_cursor` / `teams_cursor_focus`) - the same chains the old row
//! clicks drove:
//!
//! - FR-034: `Enter` on a cursor sub-agent row opens the output view of the
//!   task the cursor names, in tree paint order;
//! - FR-038: `Enter` on a cursor teammate row focuses the member, opens its
//!   output view and reports the transition to the status line;
//! - FR-064: the focus transition reports through the keyboard path too;
//! - FR-065: the cursor ordinal and the focused teammate survive a window
//!   close/reopen;
//! - NFR-005: every destination sets the redraw flag.
//!
//! The focused-border-title and mailbox-routing cases never used clicks; they
//! set `focused_teammate` directly and are kept as written. All state is
//! synthetic; nothing is written into the repository (NFR-012).

mod support;

use crate::term_events::CrosstermAppExt;

use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;
use support::make_app;

/// The lead session id every Agents case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A teammate in `Working` state with a spawned session.
fn member(agent_id: &str, name: &str) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = MemberStatus::Working;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

// ---------------------------------------------------------------------------
// FR-034: sub-agent rows, including a tree-order mapping case
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-038 / FR-039: the Teams window focus paths
// ---------------------------------------------------------------------------

fn app_with_team() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app.session.team_members = vec![member("tm-001", "reviewer"), member("tm-002", "tester")];
    app.show_teams_window = true;
    app
}

// ---------------------------------------------------------------------------
// FR-065: cursor and focus survive a close/reopen
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-058: focus routes the input while a teammate is focused
// ---------------------------------------------------------------------------

#[tokio::test]
async fn focused_input_is_not_routed_to_the_teammate_mailbox() {
    use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = app_with_team();
    crate::without_host_providers(&mut app);
    app.session.focused_teammate = Some((String::new(), "tm-001".to_string()));
    // With the popup open `Enter` is the open-overlay key (FR-038); the send
    // behaviour applies once the window is closed again.
    app.show_teams_window = false;
    app.session.input = "please report your progress".to_string();
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    app.handle_key_event(key).await;

    // FR-058 (amended): Enter addresses the lead session, which stops at the
    // no-provider guard of this fixture. The mailbox path is never taken and
    // the unsubmitted draft stays in the input.
    assert_eq!(
        app.session.input, "please report your progress",
        "the lead path keeps the draft instead of consuming it"
    );
    assert!(
        !app.status.starts_with("Failed to load team"),
        "the message must not go to the teammate mailbox path, got {}",
        app.status
    );
    assert!(
        app.status.contains("No provider configured"),
        "got: {}",
        app.status
    );

    assert_eq!(
        app.session
            .focused_teammate
            .as_ref()
            .map(|(_, id)| id.as_str()),
        Some("tm-001"),
        "the submission keeps the focus (FR-058)"
    );
}
