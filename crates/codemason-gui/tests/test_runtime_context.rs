//! Regression tests: every App call the shell makes from the window thread
//! runs inside the tokio runtime context.
//!
//! eframe runs the window on the process main thread, which is not a runtime
//! thread. App paths that `tokio::spawn` or `spawn_blocking` outside a
//! `Handle::block_on` used to panic there ("there is no reactor running"),
//! e.g. cancelling the provider API-key dialog on a real desktop. Each test
//! drives the shell on a plain `std::thread` with no runtime entered, as
//! eframe does; the runtime exists but is never entered by the test.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::{App, ProviderSetupStep};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::dialogs::provider::CANCEL;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Build a shell over an App configured by `configure` (built inside the
/// runtime, as `run_gui` builds it on the runtime thread), then run `drive`
/// on a fresh plain thread that never enters the runtime.
fn on_plain_thread(
    configure: impl FnOnce(&mut App) + Send + 'static,
    drive: impl FnOnce(&mut Harness<'static, Shell>) + Send + 'static,
) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        configure(&mut app);
        app
    };
    let handle = runtime.handle().clone();
    let window = std::thread::spawn(move || {
        assert!(
            tokio::runtime::Handle::try_current().is_err(),
            "the window thread starts outside the runtime"
        );
        let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
        let (_logs_tx, logs) = tui_log_channel(16);
        let shell = Shell::new(app).with_sources(events, logs, handle);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1000.0, 800.0))
            .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
        // The window's fonts: a new session's banner draws block glyphs.
        codemason_gui::fonts::install(&harness.ctx, craft_ui::fonts::SystemFonts::Off);
        drive(&mut harness);
    });
    if let Err(panic) = window.join() {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn test_provider_key_dialog_cancel_outside_the_runtime() {
    on_plain_thread(
        |app| {
            // First start: no session yet, the provider key is being asked.
            app.session.session_id = None;
            app.provider_setup = Some(ProviderSetupStep::EnterKey {
                provider_id: "openai".to_string(),
                provider_name: "OpenAI (GPT)".to_string(),
                key_field: codemason_app_core::input_field::InputField::new(),
                endpoint_field: codemason_app_core::input_field::InputField::new(),
                active_field: 0,
                error: None,
            });
        },
        |harness| {
            harness.run();
            harness.get_by_label(CANCEL).click();
            harness.run();
            harness.run();
            assert!(harness.state().app().provider_setup.is_none(), "cancelled");
        },
    );
}

#[test]
fn test_sidebar_new_session_click_outside_the_runtime() {
    // A click on the sidebar's "+ New session" row runs
    // `sessions_sidebar_state::click_row` straight from the drawing code. It
    // opens a session, whose start announces the instruction files with
    // `spawn_blocking` (`App::announce_session_startup`): the reported
    // "no reactor running" panic.
    on_plain_thread(
        |app| app.sidebar.visible = true,
        |harness| {
            harness.run();
            let before = harness.state().app().session.session_id.clone();
            harness.get_by_label_contains("+ New session").click();
            harness.run();
            assert_ne!(
                harness.state().app().session.session_id,
                before,
                "a new session was opened"
            );
        },
    );
}
