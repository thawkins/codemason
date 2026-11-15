//! PERF-044 / PERF-045 / PERF-046: TUI render-loop idle guards.
//!
//! * **PERF-044** - the periodic safety wake only paints when a
//!   wall-clock-driven display (countdown/spinner) is live.
//! * **PERF-045** - the cheap periodic polls/refreshes run at most once per
//!   `HOUSEKEEPING_INTERVAL`, and the loop keeps a wake scheduled for the end
//!   of that window.
//! * **PERF-046** - the chat input is re-wrapped and the cursor re-measured
//!   only when one of `(input, cursor, selection, width)` changes.

mod support;

use std::time::{Duration, Instant};

use codemason_app_core::app::{HOUSEKEEPING_INTERVAL, InputRenderCache};

// -- PERF-044: idle safety redraw ----------------------------------------

// -- PERF-045: housekeeping gate -----------------------------------------

#[test]
fn test_housekeeping_runs_only_when_due() {
    let mut app = support::make_app();
    app.jobs_last_poll = Instant::now();
    assert!(
        !app.run_housekeeping_if_due(),
        "a wake inside the interval must skip the polls"
    );
    assert_eq!(app.housekeeping_runs, 0);

    std::thread::sleep(HOUSEKEEPING_INTERVAL + Duration::from_millis(20));

    assert!(
        app.run_housekeeping_if_due(),
        "a wake past the interval must run the polls"
    );
    assert_eq!(app.housekeeping_runs, 1);
    assert!(
        !app.run_housekeeping_if_due(),
        "the pass resets the interval window"
    );
}

// -- PERF-046: input render cache ----------------------------------------

#[test]
fn test_input_render_cache_default_is_empty() {
    let cache = InputRenderCache::default();
    assert_eq!(
        cache.lines,
        [] as [codemason_app_core::styled::StyledLine; 0]
    );
    assert_eq!(cache.height, 0);
}
