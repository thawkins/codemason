//! Tests for the `Alt+O` autopilot toggle and its status-bar indicator.
//!
//! `Alt+O` used to toggle the Telemetry side panel; it now toggles autopilot
//! mode. These tests drive the real `handle_key_event` path (not a private
//! helper) and a full-frame `layout::render` on a ratatui `TestBackend`, so the
//! assertion covers the whole pipeline: key chord -> `InputAction` -> state ->
//! rendered status bar.
//!
//! Tests only - no production logic is touched.

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_app_core::app::App;

/// Press the `Alt+O` chord the way the TUI main loop does.
async fn press_alt_o(app: &mut App) {
    app.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::ALT))
        .await;
}

// -----------------------------------------------------------------------------
// Alt+O toggles autopilot
// -----------------------------------------------------------------------------

#[tokio::test]
async fn alt_o_enables_autopilot() {
    let mut app = support::make_app();
    assert!(!app.session.autopilot_enabled, "autopilot starts disabled");

    press_alt_o(&mut app).await;

    assert!(app.session.autopilot_enabled, "Alt+O must enable autopilot");
    assert!(
        app.session.autopilot_started_at.is_some(),
        "the start instant is stamped so the time limit can be enforced"
    );
    assert!(
        app.session.autopilot_time_limit_secs.is_some(),
        "the key toggle applies a bounded time limit"
    );
    assert_eq!(
        app.session.autopilot_time_limit_secs,
        Some(5000),
        "Alt+O arms the 5000-second default limit"
    );
}

#[tokio::test]
async fn alt_o_second_press_disables_autopilot_and_clears_limits() {
    let mut app = support::make_app();

    press_alt_o(&mut app).await;
    assert!(app.session.autopilot_enabled);

    press_alt_o(&mut app).await;
    assert!(
        !app.session.autopilot_enabled,
        "second Alt+O must disable autopilot"
    );
    assert!(app.session.autopilot_started_at.is_none());
    assert!(app.session.autopilot_time_limit_secs.is_none());
    assert!(app.session.autopilot_token_budget.is_none());
    assert!(app.session.autopilot_pending_continue.is_none());
}

#[tokio::test]
async fn alt_o_does_not_toggle_the_telemetry_panel() {
    let mut app = support::make_app();
    assert!(!app.show_telemetry);

    press_alt_o(&mut app).await;

    assert!(
        !app.show_telemetry,
        "Alt+O no longer opens the Telemetry panel; use /telemetry_panel"
    );
}

#[tokio::test]
async fn alt_o_is_not_inserted_into_the_input_buffer() {
    let mut app = support::make_app();
    app.session.input.clear();

    press_alt_o(&mut app).await;

    assert_eq!(
        app.session.input, "",
        "the chord must be consumed, never typed as a literal 'o'"
    );
}

#[tokio::test]
async fn slash_autopilot_off_and_alt_o_share_one_state_machine() {
    let mut app = support::make_app();

    // Enabled by the key, then disabled by the slash command.
    press_alt_o(&mut app).await;
    assert!(app.session.autopilot_enabled);
    app.execute_slash_command("/autopilot off").await;
    assert!(
        !app.session.autopilot_enabled,
        "/autopilot off must clear state set by Alt+O"
    );
    assert!(app.session.autopilot_started_at.is_none());

    // Enabled by the slash command, then disabled by the key.
    app.execute_slash_command("/autopilot on --max-time 60")
        .await;
    assert!(app.session.autopilot_enabled);
    assert_eq!(app.session.autopilot_time_limit_secs, Some(60));
    press_alt_o(&mut app).await;
    assert!(
        !app.session.autopilot_enabled,
        "Alt+O must clear state set by /autopilot on"
    );
    assert!(app.session.autopilot_time_limit_secs.is_none());
}

// -----------------------------------------------------------------------------
// Status bar reflects the state
// -----------------------------------------------------------------------------
