//! Tests for `test_teams_tui.rs`
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_teams_tui.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use codemason_agent::team::{TeamMember, TeamStore};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

fn unique_team_name(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

// -- /team status -------------------------------------------------------------

// -- /team create -------------------------------------------------------------

// -- /team show ---------------------------------------------------------------

// -- /team help ---------------------------------------------------------------

// -- /team close --------------------------------------------------------------

// -- /team delete -------------------------------------------------------------

// -- /team tasks ---------------------------------------------------------------

// -- /team clear ---------------------------------------------------------------

// -- /team message -------------------------------------------------------------

// -- /team cleanup -------------------------------------------------------------

// -- /team unknown subcommand --------------------------------------------------

// -- [T] badge - team_members drives badge rendering ---------------------------

// -- Event handling - TeammateSpawned -----------------------------------------

// -- Event handling - TeammateIdle --------------------------------------------

// -- Event handling - TeamTaskClaimed / Completed -----------------------------

// -- Event handling - TeamCleanedUp --------------------------------------------

// -- Event handling - TeammateMessage -----------------------------------------

// -- T-014 (spec `agentui`): team event plumbing gaps -------------------------

// -- T-012 (spec `agentui`): Alt+Down / Alt+Up focus cycling ------------------

/// Focusing a teammate opens its output view but leaves the input border plain:
/// FR-058 (amended) removed the focused-input routing, so the input box never
/// names a recipient and Enter always addresses the lead session. A message
/// reaches a teammate only through `/team message`, the path exercised below.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_team_focus_does_not_retitle_or_route_the_input() {
    let _cwd_guard = crate::global_env_lock();
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".codemason/teams")).unwrap();

    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    let team_name = unique_team_name("focus-route");
    let store = TeamStore::create(&team_name, "s1", tmp.path(), true).expect("create team");
    app.session.active_team = Some(store.config.clone());
    drop(store);
    app.session
        .team_members
        .push(TeamMember::new("writer", "tm-001", "general"));

    app.focus_teammate_by_id("tm-001");
    assert_eq!(
        app.session
            .focused_teammate
            .as_ref()
            .map(|(_, id)| id.as_str()),
        Some("tm-001"),
        "focusing must register before the border can be checked (FR-058)"
    );

    crate::frame::publish(&mut app);

    // Alt+Down off the single member goes past the end (the cycle does not
    // wrap; FR-046 clears the focus together with the output view instead).
    app.handle_key_event(crate::term_events::KeyEvent::new(
        crate::term_events::KeyCode::Down,
        crate::term_events::KeyModifiers::ALT,
    ))
    .await;
    assert!(
        app.session.focused_teammate.is_none(),
        "Alt+Down with a single member must clear focus past the end (FR-046)"
    );
    assert!(app.output_view.is_none());
    // Re-land on the only member for the delivery half of the test.
    app.cycle_focused_teammate(true);
    assert_eq!(
        app.session
            .focused_teammate
            .as_ref()
            .map(|(_, id)| id.as_str()),
        Some("tm-001")
    );

    // FR-058 (amended): delivery to a teammate is `/team message`'s job, not the
    // input box's - `send_teammate_message` is the arm behind that command.
    app.send_teammate_message(
        &app.session
            .active_team
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default(),
        "writer",
        "please check the logs",
    );
    assert!(
        app.status.contains("message sent to writer") || app.status.contains("writer"),
        "the routed send must report the teammate, got: {}",
        app.status
    );

    let _ = std::env::set_current_dir(original_dir);

    let text: String = app.status.chars().collect();
    assert!(text.contains("writer"), "status must name the teammate");
}

// -- SEC-codemason-tui-004: team-name validation at the TUI boundary (SECTASKS T-004) -

// -- /team delete confirmation (teamssetup T-028, FR-023, FR-030, F-28) --------
