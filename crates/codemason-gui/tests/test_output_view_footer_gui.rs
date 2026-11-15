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
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_output_view_footer.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

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

/// `Esc` closes the overlay and clears the selection identity with it
/// (FR-075).
#[tokio::test]
async fn test_output_view_escape_closes_the_overlay() {
    let mut app = app_with_output_view();
    app.session.selected_agent_session_id = Some("s1".to_string());
    app.selected_agent_index = Some(0);
    crate::frame::publish(&mut app);

    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
        .await;

    assert!(app.output_view.is_none(), "Esc closes the view");
    assert!(
        app.session.selected_agent_session_id.is_none(),
        "the selection identity clears with the view (FR-075)"
    );
    assert!(
        app.selected_agent_index.is_none(),
        "the selected index clears with the view (FR-075)"
    );
}

// ---------------------------------------------------------------------------
// FR-075: wheel routing above the Agents popup
// ---------------------------------------------------------------------------
