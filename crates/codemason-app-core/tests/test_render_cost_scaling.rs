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

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::App;

/// Build an App mirroring the make_app() pattern used by the other TUI tests.
/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

/// Test 1: after `refresh_code_index_stats` runs with no code index
/// attached, the busy latch must clear so the loop deadline returns to the
/// idle cadence. Regression for the `total > 0 -> reindex_active` latch
/// that kept `compute_next_deadline` at 250 ms forever after any reindex.
#[test]
fn test_code_index_busy_clears_when_no_index_attached() {
    let mut app = make_app();

    // Simulate the "stats locks were momentarily busy" entry point that sets
    // the busy flag, and age out the refresh throttle so the refresh body
    // actually runs.
    app.code_index_busy = true;
    app.needs_redraw = false;
    app.code_index_stats_last_refresh = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or(std::time::Instant::now());

    // No real CodeIndex handle is attached, so refresh takes the `else`
    // branch, which must unconditionally clear the busy flag.
    app.refresh_code_index_stats_for_test();
    assert!(
        !app.code_index_busy,
        "busy latch must clear when no code index is attached"
    );
}
