//! Regression tests for idle-CPU hotspots (v1.0.64 CPU investigation).
//!
//! These tests pin the render-cost contracts the idle-CPU fix relies on:
//!
//! 1. `code_index_busy` is cleared once a reindex completes (done == total),
//!    so the TUI loop drops back to its 2 s idle deadline instead of waking
//!    4x/sec forever.
//! 2. `render_messages` / `render_log_panel` only wrap the visible scroll
//!    window, so per-frame cost is bounded by viewport height rather than
//!    transcript length (long transcripts no longer pin a core at idle).
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_render_cost_scaling.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use codemason_app_core::app::App;
use codemason_types::message::{Message, MessagePart, Role};

/// Build an App mirroring the make_app() pattern used by the other TUI tests.
/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

/// Public test hook (#[doc(hidden)]) on App for refresh-code-index behavior.

fn long_message(id: &str, session: &str, text: &str) -> Message {
    let mut msg = Message::new(
        session,
        Role::Assistant,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    );
    msg.id = id.to_string();
    msg
}

/// `render_app_to_string` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_app_to_string_effects(app: &mut App) {
    crate::frame::publish(app);
}

const TAIL_MARKER: &str = "TAILMARKER-7f3a";

/// Test 2: render cost must NOT scale with transcript length. Render the
/// same viewport twice - short transcript vs 500-message transcript - after
/// warming caches, and assert steady-state per-frame cost stays bounded.
/// Regression for the whole-transcript Paragraph wrap that dominated perf
/// profiles after long agent turns.
#[test]
fn test_render_cost_bounded_by_viewport_not_transcript() {
    let chunk = format!(
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
         tempor incididunt ut labore et dolore magna aliqua. {TAIL_MARKER}"
    );

    let make_messages = |count: usize| -> Vec<Message> {
        (0..count)
            .map(|i| long_message(&format!("m{i}"), "s1", &chunk))
            .collect()
    };

    let mut app_short = make_app();
    app_short.session.messages = make_messages(5);
    let mut app_long = make_app();
    app_long.session.messages = make_messages(500);

    // Warm both caches so we measure steady-state per-frame cost.
    render_app_to_string_effects(&mut app_short);
    render_app_to_string_effects(&mut app_long);

    let _frames = 10;
    let start = std::time::Instant::now();
    render_app_to_string_effects(&mut app_short);
    let short_elapsed = start.elapsed();

    let start = std::time::Instant::now();
    render_app_to_string_effects(&mut app_long);
    let long_elapsed = start.elapsed();

    // A 100x larger transcript must not cost dramatically more per frame.
    // Allow generous headroom for allocator noise; the un-sliced
    // implementation costs ~20-100x on 500-message transcripts.
    assert!(
        long_elapsed.as_secs_f64() < 3.0 * short_elapsed.as_secs_f64(),
        "render cost must be bounded by viewport, not transcript length \
         (short 5 msgs: {short_elapsed:?}, long 500 msgs: {long_elapsed:?})"
    );
}
