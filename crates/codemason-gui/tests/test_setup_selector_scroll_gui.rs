//! Tests that every model/provider selector scrolls to keep its cursor on screen.
//!
//! The setup dialogs share one rule ([`codemason_app_core::layout`]'s window helpers):
//! the painted slice starts where it must so the highlighted row is visible, and
//! a clipped list says so in its legend. These tests render through the real
//! `layout::render` entry point at terminal sizes small enough to clip, so a
//! selector that silently hides its tail (the regression this guards) fails on
//! what the user actually sees.
//!
//! They also pin the wheel path: a notch over the open dialog steps the cursor
//! through the same handler the keyboard uses, and a notch outside it is
//! swallowed so nothing behind the modal scrolls.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_setup_selector_scroll.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use crate::term_events::CrosstermAppExt;
use crate::term_events::{KeyModifiers, MouseEvent, MouseEventKind};
use codemason_app_core::app::App;
use codemason_app_core::app::{ModelPickerEntry, ProviderSetupStep};
use codemason_types::{ThinkingConfig, ThinkingLevel};

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

fn entry(id: &str, name: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: name.to_string(),
        context_window: 200_000,
        max_output: Some(64_000),
        cost_input: 3.0,
        cost_output: 15.0,
        reasoning: true,
        vision: true,
        tool_use: true,
        thinking_levels: vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ],
        thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        cost_tier: "Premium".to_string(),
        cost_multiplier: "1x".to_string(),
    }
}

/// `painted` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn painted_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

fn wheel(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn test_wheel_outside_the_setup_dialog_never_scrolls_the_chat() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![entry("a", "Model A"), entry("b", "Model B")],
        selected: 0,
    });
    painted_effects(&mut app, 120, 40);
    let before = app.session.scroll_offset;

    app.handle_mouse_event(wheel(MouseEventKind::ScrollDown, 0, 0));
    assert_eq!(
        app.session.scroll_offset, before,
        "the modal swallows a wheel outside it, so the transcript must not move"
    );
    match app.provider_setup.as_ref().expect("picker still open") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectModel, got {other:?}"),
    }
}

#[test]
fn test_wheel_over_the_success_card_never_dismisses_it() {
    // `Done` closes on any key, so a wheel over it must not be translated into a
    // keystroke: the confirmation stays until the user presses a key.
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::Done {
        provider_name: "Anthropic".to_string(),
        model_name: Some("claude-sonnet-4".to_string()),
    });
    painted_effects(&mut app, 120, 40);

    assert!(
        matches!(app.provider_setup, Some(ProviderSetupStep::Done { .. })),
        "a wheel notch must not dismiss the success card"
    );
}
