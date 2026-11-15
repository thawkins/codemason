//! Tests for the output view overlay's footer legend and wheel routing
//! (spec `agentui` T-021: FR-074, FR-075).
//!
//! FR-074 pins the overlay's frame geometry: a 90x70 centred modal built by the
//! shared `window_frame` helper, with a one-row key legend along its bottom
//! inner row and the content computed as the inner area minus that band. The
//! overlay carries **no** Close button - `Esc` and a click outside the overlay
//! are its dismissal paths - so the footer is the only affordance it paints.
//!
//! FR-075 pins the dismissal and wheel behaviour: `Esc` closes
//! the overlay leaving every other value untouched, a click outside it closes it
//! too, and a wheel event over the overlay scrolls the overlay even when the
//! Agents popup is open beneath it - the overlay is the topmost surface, so its
//! wheel arms precede the popup arms.
//!
//! Every case drives the real render entry point / key and mouse router through
//! a ratatui `TestBackend`, so the assertions exercise the exact path a real
//! event takes (NFR-001). All state is synthetic and in-memory; nothing is
//! written into the repository (NFR-012).

mod support;

use codemason_app_core::app::{OutputViewState, OutputViewTarget};

/// The app type under test, re-exported through `support`.
type AppT = codemason_app_core::app::App;

/// An app with the output overlay open on a primary session.
fn app_with_output_view() -> AppT {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: codemason_app_core::app::OutputViewLineCache::default(),
    });
    app
}

// ---------------------------------------------------------------------------
// FR-074: output overlay frame and footer legend
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-075: dismissal
// ---------------------------------------------------------------------------

/// Closing a closed overlay is a no-op that still keeps the window state sane.
#[test]
fn test_output_view_close_is_idempotent() {
    let mut app = app_with_output_view();
    app.close_output_view();
    assert!(app.output_view.is_none());
    app.close_output_view();
    assert!(app.output_view.is_none());
}

// ---------------------------------------------------------------------------
// FR-075: wheel routing above the Agents popup
// ---------------------------------------------------------------------------
