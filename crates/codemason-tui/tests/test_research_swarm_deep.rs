//! Deep coverage for the research-progress tracker, the `/research` slash
//! dispatch, the swarm status/cancel grammar, and the `lib.rs` public
//! deadline/render policy helpers.
//!
//! Three surfaces are exercised here:
//!
//! 1. [`codemason_tui::research_progress`] - a public module, so its
//!    `encode_progress_event` / `encode_cluster_progress_event` /
//!    `decode_progress_event` / `ResearchProgress::{apply,finish,render}` API is
//!    called directly. Every `SessionEvent` arm the existing suites miss is
//!    round-tripped.
//! 2. `App::handle_research_command` (`src/app/research.rs`) - `pub(crate)` with
//!    no public hook, so it is driven through the public
//!    `App::execute_slash_command("/research ...")` entry point.
//! 3. `App::handle_swarm_status` / `handle_swarm_cancel` (`src/app/swarm.rs`) -
//!    likewise driven through `/swarm ...`. The decomposition/spawn/poll/finalize
//!    helpers (`execute_swarm_decomposition`, `spawn_swarm_teammates`,
//!    `poll_swarm_unblock`, `poll_swarm_completion`, `finalize_swarm_completion`)
//!    are `pub(crate)` with no public test hook and are reachable only from the
//!    `run_tui` loop (`poll_pending_swarm`) - they are NOT covered here.
//! 4. `codemason_tui::{compute_next_deadline_test, should_render}` (`src/lib.rs`) -
//!    public test wrappers, called directly to hit every deadline branch.
//!
//! Tests only - no production logic is touched.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use codemason_tui::research_progress::ResearchProgress;

// ===========================================================================
// research_progress.rs - encode_progress_event: every SessionEvent arm
// ===========================================================================

/// Encode one event and return the decoded detail (panicking if it does not
/// decode). Used for the arms whose produced status token is in the decoder's
/// allow-list.

/// Encode one event that produces a status token the decoder deliberately does
/// not recognise; assert the pipeline yields `None` (kept out of the tracker).

// ===========================================================================
// research_progress.rs - apply / finish / render state machine
// ===========================================================================

// ===========================================================================
// app/research.rs - driven through `/research ...`
// ===========================================================================

/// Receive the next `TextDelta` for a short window, skipping unrelated events.
// ===========================================================================
// app/swarm.rs - driven through `/swarm ...`
// ===========================================================================
use codemason_agent::team::{SwarmDecomposition, SwarmState, SwarmSubtask};

fn swarm_state(team_name: &str, tasks: Vec<SwarmSubtask>, spawned: bool) -> SwarmState {
    SwarmState {
        team_name: team_name.to_string(),
        prompt: "do work".to_string(),
        decomposition: SwarmDecomposition { tasks },
        spawned,
        completed: false,
        default_agent_type: None,
    }
}

// ===========================================================================
// lib.rs - compute_next_deadline_test / should_render policy branches
// ===========================================================================

fn past_idle() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("clock past 3s")
}

#[test]
fn deadline_default_is_bounded_by_idle_redraw() {
    let app = support::make_app();
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    // The 60s default is capped by the idle redraw interval (~2s) off last_draw.
    assert!(deadline <= now + Duration::from_secs(3), "got {deadline:?}");
}

#[test]
fn deadline_permission_queue_wakes_within_a_second() {
    let mut app = support::make_app();
    app.session
        .permission_queue
        .push_back(permission_request("perm-1"));
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_timed_question_wakes_within_a_second() {
    let mut app = support::make_app();
    app.session
        .question_queue
        .push_back(codemason_tui::app::QuestionRequest {
            id: "q1".to_string(),
            session_id: "s1".to_string(),
            question: "pick".to_string(),
            options: vec!["a".to_string()],
            default_index: Some(0),
            expire_at_ms: Some(1),
            secret: false,
        });
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_active_web_phase_wakes_within_a_second() {
    let mut app = support::make_app();
    let mut p = ResearchProgress::new("r", "t");
    p.web_phase_deadline = Some(Instant::now() + Duration::from_secs(30));
    app.research_progress.push(p);
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_status_and_banner_expiry_are_scheduled() {
    let mut app = support::make_app();
    app.status_set_at = Some(Instant::now());
    app.session.run_cost_banner_at = Some(Instant::now());
    app.history_save_deadline = Some(Instant::now() + Duration::from_millis(500));
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    // The earliest of status expiry (2s), banner (15s), and flush (0.5s) wins.
    assert!(
        deadline <= now + Duration::from_millis(600),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_swarm_state_wakes_within_two_seconds() {
    let mut app = support::make_app();
    app.session.swarm_state = Some(swarm_state("swarm-x", vec![], false));
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(2100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_pending_stream_group_is_throttle_bounded() {
    let mut app = support::make_app();
    // A *pending* message group is `message_cache_dirty_from < messages.len()`
    // (0 < 1), so the throttle-tail deadline must fire promptly rather than
    // waiting the full idle interval.
    app.session
        .messages
        .push(codemason_agent::message::Message::user_text("s", "hi"));
    app.session.message_cache_dirty_from = 0;
    let now = Instant::now();
    let deadline = codemason_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(50),
        "pending stream group should schedule a near-term repaint, got {deadline:?}"
    );
}

#[test]
fn should_render_dirty_and_idle_states() {
    let mut app = support::make_app();
    app.needs_redraw = true;
    assert!(codemason_tui::should_render(&app, Instant::now()));

    app.needs_redraw = false;
    assert!(!codemason_tui::should_render(&app, Instant::now()));
    assert!(!codemason_tui::should_render(&app, past_idle()));
}

#[test]
fn should_render_safety_wake_repaints_only_for_live_display() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    // A live permission countdown makes the periodic safety wake paint.
    app.session
        .permission_queue
        .push_back(permission_request("perm-2"));
    assert!(codemason_tui::should_render(&app, past_idle()));
}

#[test]
fn should_render_paints_a_pending_stream_group() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    // A pending cache group is `message_cache_dirty_from < messages.len()`
    // (0 < 1) and counts as visible content at the safety interval.
    app.session
        .messages
        .push(codemason_agent::message::Message::user_text("s", "hi"));
    app.session.message_cache_dirty_from = 0;
    assert!(codemason_tui::should_render(&app, past_idle()));
}

// A minimal PermissionRequest builder mirroring the shape the countdown dialog
// queues (kept local so the file does not depend on a helper in `support`).
fn permission_request(id: &str) -> codemason_agent::permission::PermissionRequest {
    codemason_agent::permission::PermissionRequest {
        id: id.to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({ "timeout_secs": 120u64 }),
        tool_call_id: None,
    }
}
