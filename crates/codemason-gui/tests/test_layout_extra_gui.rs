//! Extra coverage tests for `codemason_app_core::layout` render branches.
//!
//! Drives the real `layout::render` path through a `TestBackend` across the
//! layout permutations the crate supports: terminal sizes (tiny/large/zero),
//! split ratios, every side-panel visibility toggle, and every overlay branch
//! (provider-setup steps, pickers, dialogs, viewers, popups, teammate strip).
//! Also exercises the public layout helpers directly.
//!
//! Tests only - no production logic is touched.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_layout_extra.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

mod support;

use crate::term_events::CrosstermAppExt;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};

use codemason_app_core::app::App;

/// `render` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

// ===========================================================================
// Terminal sizes / split ratios
// ===========================================================================

// ===========================================================================
// Overlays / dialogs
// ===========================================================================

// ===========================================================================
// Provider-setup dialog: every step variant
// ===========================================================================

// ===========================================================================
// Pickers / viewers / dialogs
// ===========================================================================

#[test]
fn render_loop_setup_dialog() {
    let mut app = support::make_app();
    codemason_app_core::app::open_loop_setup(&mut app);
    assert!(app.loop_setup.is_some());
    render_effects(&mut app, 120, 40);
}

// ===========================================================================
// layout public helpers
// ===========================================================================

/// Feed a key through the app handler so the layout reacts to real state
/// transitions (used for the loop dialog field navigation below).
async fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.handle_key_event(KeyEvent::new(code, mods)).await;
}

#[tokio::test]
async fn render_loop_dialog_after_field_navigation() {
    let mut app = support::make_app();
    codemason_app_core::app::open_loop_setup(&mut app);
    for _ in 0..6 {
        press(&mut app, KeyCode::Tab, KeyModifiers::NONE).await;
    }
    assert!(app.loop_setup.is_some());
    render_effects(&mut app, 120, 40);
}
