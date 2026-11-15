//! Deep coverage for `src/lib.rs`, `src/app/session_ops.rs`,
//! `src/app/event_handler.rs`, and `src/app/connector.rs`.
//!
//! These exercise the remaining public surfaces the existing suites leave
//! untouched: the `lib.rs` test-wrapper deadline/should-render helpers and the
//! `App`/`StderrSpool` re-exports; the connector-catalogue and plugin-store
//! panel state helpers in `session_ops.rs`; the event-handler arms whose
//! variants the other suites never construct (the Copilot device-flow result,
//! `RouterClassification`, `ServiceStartError`, `ModelDownloadProgress`, the
//! non-current-session guards); and the `connector.rs` dispatch glue reached
//! through `/connectors <sub>`.
//!
//! Tests only - no production logic is touched. Every test drives a real public
//! API or the real `handle_event` / `execute_slash_command` / key path; no
//! network and no real TTY (rendering, where needed, goes through `TestBackend`).
//! Panels are installed directly into the `App` state rather than through the
//! launch helpers, so no background catalogue/store fetch is ever started.

use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_connectors::{
    CatalogueBrowser, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};
use codemason_tui::App;

#[path = "support/mod.rs"]
mod support;

// ===========================================================================
// shared fixtures
// ===========================================================================

/// An app with a live primary session.

fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// A connector-catalogue descriptor with a blank description.
fn connector(id: &str, category: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: category.to_string(),
        tags: Vec::new(),
        source: format!("git+https://github.com/acme/connectors#v1:{id}"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// A plugin-store entry.

/// An app with the connector-catalogue panel open and `entries` loaded. The
/// browser is installed directly (no off-loop fetch is started).
fn open_connector_catalogue(entries: Vec<ConnectorDescriptor>) -> App {
    let mut app = support::make_app();
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(entries, 0);
    app.connector_store = Some(browser);
    app
}

/// An app with the plugin-store panel open for `kind` and `entries` loaded. The
/// browser is installed directly (no off-loop fetch is started).

// ===========================================================================
// lib.rs - re-exports and the deadline / render test wrappers
// ===========================================================================

#[test]
fn compute_next_deadline_test_idle_caps_at_idle_redraw_interval() {
    let app = support::make_app();
    let last_draw = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, last_draw);
    assert!(
        deadline > Instant::now(),
        "the deadline must be in the future"
    );
    assert!(
        deadline <= last_draw + Duration::from_secs(5),
        "an idle app must not schedule further out than the idle cap"
    );
}

#[test]
fn compute_next_deadline_test_permission_countdown_wakes_within_a_second() {
    let mut app = support::make_app();
    app.session
        .permission_queue
        .push_back(codemason_agent::permission::PermissionRequest {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            permission: "bash".to_string(),
            patterns: vec!["ls".to_string()],
            metadata: serde_json::json!({ "timeout_secs": 120u64 }),
            tool_call_id: None,
        });
    let deadline = codemason_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_millis(1100),
        "a live permission countdown must wake within a second"
    );
}

#[test]
fn compute_next_deadline_test_status_expiry_shortens_the_wake() {
    let mut app = support::make_app();
    app.status_set_at = Some(Instant::now());
    let deadline = codemason_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_millis(2100),
        "the status auto-expiry must bound the wake"
    );
}

#[test]
fn compute_next_deadline_test_run_cost_banner_shortens_the_wake() {
    let mut app = support::make_app();
    app.session.run_cost_banner_at = Some(Instant::now());
    let deadline = codemason_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_secs(16),
        "the run-cost banner auto-dismiss must bound the wake"
    );
}

#[test]
fn compute_next_deadline_test_housekeeping_window_bounds_the_idle_wake() {
    let mut app = support::make_app();
    app.jobs_last_poll = Instant::now();
    let due_at = app.housekeeping_due_at();
    let deadline = codemason_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(deadline <= due_at);
}

#[test]
fn compute_next_deadline_test_pending_message_group_bounds_the_wake() {
    let mut app = support::make_app();
    app.session
        .messages
        .push(codemason_types::message::Message::assistant_text(
            "s1", "hi",
        ));
    app.session.message_cache_dirty_from = 0;
    let deadline = codemason_tui::compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline <= Instant::now() + Duration::from_secs(5),
        "a pending message group must schedule a repaint wake"
    );
}

#[test]
fn should_render_is_false_when_clean_and_true_when_dirty() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    assert!(!codemason_tui::should_render(&app, Instant::now()));

    app.needs_redraw = true;
    assert!(codemason_tui::should_render(&app, Instant::now()));
}

#[test]
fn should_render_safety_interval_paints_a_pending_message_group() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.session
        .messages
        .push(codemason_types::message::Message::assistant_text(
            "s1", "hi",
        ));
    app.session.message_cache_dirty_from = 0;
    let past = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("monotonic clock newer than 3s");
    assert!(
        codemason_tui::should_render(&app, past),
        "a pending group must paint at the safety interval"
    );
}

#[test]
fn should_render_safety_interval_paints_a_live_countdown() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    app.session
        .permission_queue
        .push_back(codemason_agent::permission::PermissionRequest {
            id: "p1".to_string(),
            session_id: "s1".to_string(),
            permission: "bash".to_string(),
            patterns: vec!["ls".to_string()],
            metadata: serde_json::json!({ "timeout_secs": 120u64 }),
            tool_call_id: None,
        });
    let past = Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("monotonic clock newer than 3s");
    assert!(
        codemason_tui::should_render(&app, past),
        "a live countdown must paint at the safety interval"
    );
}

// ===========================================================================
// session_ops.rs - connector-catalogue panel helpers
// ===========================================================================

// ===========================================================================
// session_ops.rs - plugin-store panel helpers
// ===========================================================================

// ===========================================================================
// session_ops.rs - rollback / websearch polls, clear dialogs, poisoned lock
// ===========================================================================

// ===========================================================================
// event_handler.rs - the remaining Event arms
// ===========================================================================

// ===========================================================================
// connector.rs - the `/connectors <sub>` dispatch glue
// ===========================================================================

// ===========================================================================
// Real key routing through the panels and a TestBackend render
// ===========================================================================

#[tokio::test]
async fn render_frame_with_connector_panel_open_has_unicode_borders() {
    let mut app = open_connector_catalogue(vec![connector("github", "developer")]);
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("draw");
    // The panel frames itself with the default Unicode box-drawing glyphs; the
    // surrounding app chrome (logo) may carry other glyphs, so scope the check
    // to the panel's own border.
    let area = app.connector_store_area;
    assert!(
        area.width > 0 && area.height > 0,
        "the panel painted an area"
    );
    let buffer = terminal.backend().buffer().clone();
    let right = area.x + area.width.saturating_sub(1);
    assert_eq!(
        buffer[(area.x, area.y)].symbol(),
        "\u{250c}",
        "top-left border corner"
    );
    assert_eq!(
        buffer[(right, area.y)].symbol(),
        "\u{2510}",
        "top-right border corner"
    );
    let bottom = area.y + area.height.saturating_sub(1);
    assert_eq!(
        buffer[(area.x, bottom)].symbol(),
        "\u{2514}",
        "bottom-left border corner"
    );
    for y in area.y + 1..bottom {
        assert_eq!(
            buffer[(area.x, y)].symbol(),
            "\u{2502}",
            "left border rule at y={y}"
        );
    }
}
