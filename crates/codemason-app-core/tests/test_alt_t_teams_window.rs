//! Tests for the `Alt+T` Teams-window toggle chord (spec `agentssetup` FR-045).
//!
//! The twin of `test_alt_a_agents_window.rs`: the chord resolves to an
//! `InputAction` and reaches the panel through the same
//! `App::toggle_teams_window` the Teams button's mouse click calls, so a
//! keyboard-only terminal user can open and dismiss the panel with or without
//! team members being present.
//!
//! Also covers NFR-002 (the `t` is never typed into the input buffer) and
//! NFR-009 (bare `t` still behaves exactly as before).

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use crate::term_events::handle_key;
use codemason_agent::team::TeamConfig;
use codemason_app_core::app::App;
use codemason_app_core::input::InputAction;
use support::make_app;

/// The lead session id used by every case.
const LEAD_SESSION: &str = "lead-session-0001";

/// An `Alt+T` key event.
fn alt_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT)
}

/// A bare `t` key event, for the NFR-002 / NFR-009 checks.
fn plain_t() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)
}

/// An app with an explicit redraw baseline of "clean".
fn app() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app
}

/// Build an `App` with a live team, so the Teams window has a subject.
fn app_with_team() -> App {
    let mut app = app();
    app.session.active_team = Some(TeamConfig::new("alpha", LEAD_SESSION));
    app
}

// ---------------------------------------------------------------------------
// FR-045: the chord resolves to an action, not an inline mutation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_resolves_to_the_teams_toggle_action() {
    let mut app = app();

    let action = handle_key(&mut app, alt_t()).await;
    assert!(
        matches!(action, Some(InputAction::ToggleTeamsWindow)),
        "FR-045: Alt+T must resolve to InputAction::ToggleTeamsWindow so it \
         reaches the same toggle the Teams button calls"
    );
    assert!(
        !app.show_teams_window,
        "resolving the action must not itself mutate the window"
    );
}

#[tokio::test]
async fn alt_t_is_bound_regardless_of_window_availability() {
    // The binding is unconditional; availability is the toggle's own guard.
    // An unconditional binding keeps the chord from silently meaning
    // "type a t" when no team exists yet.
    let mut app = app();
    let action = handle_key(&mut app, alt_t()).await;
    assert!(matches!(action, Some(InputAction::ToggleTeamsWindow)));
}

// ---------------------------------------------------------------------------
// FR-045: the keyboard produces the same outcome as the mouse click
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_toggles_the_window_and_requests_a_redraw() {
    let mut app = app_with_team();

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window,
        "FR-045: the first Alt+T opens the window"
    );

    app.handle_key_event(alt_t()).await;
    assert!(!app.show_teams_window, "FR-045: the second Alt+T closes it");
}

#[tokio::test]
async fn alt_t_without_a_team_still_opens_the_window() {
    // With no team the panel paints its lead row and the empty-state hint
    // (FR-026) rather than refusing to open.
    let mut app = app();
    app.status = "ready".to_string();

    app.handle_key_event(alt_t()).await;
    assert!(
        app.show_teams_window,
        "with no team the window still opens on its lead row and empty-state hint"
    );
    assert_eq!(
        app.status, "ready",
        "an empty open is not an error, so it reports nothing"
    );

    app.handle_key_event(alt_t()).await;
    assert!(!app.show_teams_window, "the second chord closes it again");
}

#[tokio::test]
async fn the_teams_refusal_names_the_condition_and_a_next_action() {
    // NFR-005: every new message names the condition and at least one
    // supported next action.
    let text = codemason_app_core::app::TEAMS_WINDOW_EMPTY_REFUSAL;
    assert!(
        text.contains("nothing to show"),
        "the refusal names the condition: {text}"
    );
    assert!(
        text.contains("team"),
        "the refusal names the subject the panel would show: {text}"
    );
    assert!(
        text.contains("/team") || text.contains("/swarm"),
        "the refusal names a supported next action: {text}"
    );
}

#[tokio::test]
async fn alt_t_closes_an_open_window_even_when_its_team_vanished() {
    // The team can end while the window is open; the chord that opened it must
    // still be able to dismiss it.
    let mut app = app();
    app.show_teams_window = true;

    app.handle_key_event(alt_t()).await;
    assert!(
        !app.show_teams_window,
        "an open window always closes, whatever its subject is now"
    );
}

// ---------------------------------------------------------------------------
// NFR-002 / NFR-009: the chord consumes the key and shadows nothing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alt_t_does_not_insert_t_into_the_input() {
    let mut app = app_with_team();
    app.session.input = "PROBE".to_string();
    app.session.input_cursor = app.input_len_chars();

    app.handle_key_event(alt_t()).await;

    assert_eq!(
        app.session.input, "PROBE",
        "NFR-002: Alt+T must not leak the 't' into the input buffer"
    );
    assert_eq!(
        app.session.input_cursor,
        app.input_len_chars(),
        "the cursor must not move"
    );
}

#[tokio::test]
async fn bare_t_still_types_t() {
    let mut app = app();

    let action = handle_key(&mut app, plain_t()).await;
    assert!(
        action.is_none(),
        "NFR-009: bare 't' is plain text, not the Teams chord"
    );
    assert_eq!(app.session.input, "t", "NFR-009: bare 't' is still typed");
    assert!(
        !app.show_teams_window,
        "bare 't' must not open the Teams window"
    );
}
