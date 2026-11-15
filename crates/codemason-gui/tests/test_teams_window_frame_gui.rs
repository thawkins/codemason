//! Tests for the Teams window frame (spec `agentui` T-004).
//!
//! Covers the modal geometry (FR-017), the footer key legend carrying the `Esc`
//! dismissal (FR-018), the ` Team: <name>  (lead + <n> teammate) ` table title
//! (FR-019), the twelve-column dimmed header (FR-020) and the guarantee that
//! every popup rectangle stays inside the terminal rectangle (NFR-010).
//!
//! Every case drives the real public render entry point through a ratatui
//! `TestBackend` and reads the painted buffer back, so no renderer logic is
//! re-implemented here (NFR-001, NFR-012).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_teams_window_frame.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_agent::team::{TeamConfig, TeamMember};
use codemason_app_core::app::App;

use support::make_app;

/// An app with the Teams window open on team `alpha`.
fn open_teams(members: usize) -> App {
    let mut app = make_app();
    app.session.session_id = Some("lead-session-0001".to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-session-0001"));
    for i in 0..members {
        let mut member = TeamMember::new(format!("reviewer-{i}"), format!("tm-{i:03}"), "general");
        member.session_id = Some(format!("child-session-{i}"));
        app.session.team_members.push(member);
    }
    app.show_teams_window = true;
    app
}

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// -- FR-017: geometry -------------------------------------------------------

// -- FR-018: footer key legend ----------------------------------------------

/// `Esc` dismisses the Teams window and leaves the team and its members
/// untouched (FR-018, FR-032).
#[tokio::test]
async fn test_teams_escape_dismisses_the_window() {
    let mut app = open_teams(2);
    render_effects(&mut app, 120, 40);
    assert!(app.show_teams_window, "the window starts open");

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
        .await;

    assert!(
        !app.show_teams_window,
        "Esc must dismiss the Teams window (FR-018)"
    );
    assert_eq!(
        app.session.team_members.len(),
        2,
        "dismissing the window must not touch the member list"
    );
    assert!(
        app.session.active_team.is_some(),
        "dismissing the window must not clear the active team"
    );
}

// -- FR-019: table title ----------------------------------------------------

// -- FR-020: header ---------------------------------------------------------

// -- NFR-010: responsive minimum --------------------------------------------
