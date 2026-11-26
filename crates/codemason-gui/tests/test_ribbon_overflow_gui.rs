//! A window too narrow for every ribbon tab keeps the hidden tabs reachable
//! through the tab strip's overflow button.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use codemason_gui::ribbon::MORE_TABS_LABEL;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A shell in a window `width` points wide.
fn rig(width: f32) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(width, 600.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

#[test]
fn test_ribbon_overflow_wide_window_has_no_overflow_button() {
    let (_rt, mut harness) = rig(1400.0);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Button, MORE_TABS_LABEL)
            .is_none()
    );
}

#[test]
fn test_ribbon_overflow_narrow_window_lists_hidden_tabs() {
    let (_rt, mut harness) = rig(360.0);
    harness.run();
    assert!(
        harness.query_by_label("Help").is_none(),
        "Help does not fit in the strip"
    );
    harness
        .get_by_role_and_label(Role::Button, MORE_TABS_LABEL)
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::MenuItem, "Help")
        .click();
    harness.run();
    assert!(
        harness.query_by_label("Help").is_some(),
        "the chosen tab opens in the strip"
    );
}
