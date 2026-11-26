//! A small window keeps the Material dialogs and the sessions popup menu
//! inside it: a tall dialog scrolls instead of running past the window's
//! edges, and a long menu scrolls instead of leaving items off-screen.

#[path = "support/mod.rs"]
mod support;

use codemason_app_core::app::ProviderSetupStep;
use codemason_app_core::geometry::Area;
use codemason_app_core::input_field::InputField;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::session_menu::{MenuAction, MenuItem, MenuTarget, SessionMenu};
use codemason_config::OtelProtocol;
use codemason_gui::Shell;
use codemason_gui::dialogs::session_menu::SESSION_MENU_TITLE;
use codemason_gui::keys::default_keymap;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A window this size is shorter than the telemetry dialog and the long
/// menu below.
const WINDOW: egui::Vec2 = egui::vec2(900.0, 360.0);

/// A shell in a small window over an App set up by `configure`.
fn rig(
    configure: impl FnOnce(&mut codemason_app_core::app::App),
) -> (tokio::runtime::Runtime, Harness<'static, Shell>) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        configure(&mut app);
        app
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(WINDOW)
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness)
}

/// Whether `rect` lies inside the window.
fn inside_window(rect: egui::Rect) -> bool {
    egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW)
        .expand(0.5)
        .contains_rect(rect)
}

#[test]
fn test_dialog_bounds_tall_material_dialog_stays_in_a_small_window() {
    let (_rt, mut harness) = rig(|app| {
        app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
            endpoint_field: InputField::with_text("http://localhost:4318"),
            protocol: OtelProtocol::Http,
            interval_field: InputField::with_text("60"),
            timeout_field: InputField::with_text("10"),
            port_field: InputField::new(),
            logs_field: InputField::new(),
            active_field: 0,
            error: None,
        });
    });
    harness.run();
    let dialog = harness
        .get_by_role_and_label(Role::Dialog, "Telemetry setup")
        .rect();
    assert!(
        inside_window(dialog),
        "dialog {dialog:?} in window {WINDOW:?}"
    );
}

#[test]
fn test_dialog_bounds_long_session_menu_stays_in_a_small_window() {
    let items = (0..40)
        .map(|i| MenuItem {
            label: format!("Section {i}"),
            accel: None,
            action: MenuAction::ClearSelection,
            group: 0,
            danger: false,
            enabled: true,
        })
        .collect();
    let (_rt, mut harness) = rig(|app| {
        app.sidebar.selected.insert("s-a".to_string());
        app.sidebar.menu = Some(SessionMenu {
            target: MenuTarget::Selection,
            row_y: 0,
            items,
            cursor: 39,
            area: Area::default(),
            scroll: 0,
            submenu: None,
        });
    });
    harness.run();
    let menu = harness
        .get_by_role_and_label(Role::Menu, SESSION_MENU_TITLE)
        .rect();
    assert!(inside_window(menu), "menu {menu:?} in window {WINDOW:?}");
    // The highlighted last item is scrolled into view.
    let last = harness.get_by_label("Section 39").rect();
    assert!(inside_window(last), "item {last:?} in window {WINDOW:?}");
}

#[test]
fn test_dialog_bounds_long_notice_detail_keeps_ok_in_a_small_window() {
    let (_rt, mut harness) = rig(|app| {
        app.notice_open = true;
        app.notice_title = "Error".to_string();
        app.notice_message = "Something failed".to_string();
        app.notice_detail = Some(
            (0..200)
                .map(|i| format!("    at frame {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    });
    harness.run();
    let ok = harness.get_by_role_and_label(Role::Button, "OK").rect();
    assert!(inside_window(ok), "OK {ok:?} in window {WINDOW:?}");
}

#[test]
fn test_dialog_bounds_long_picker_list_keeps_its_actions_in_a_small_window() {
    use codemason_app_core::app::HistoryPickerState;
    use codemason_gui::dialogs::pickers::{CLOSE, HISTORY_TITLE};
    let (_rt, mut harness) = rig(|app| {
        app.history_picker = Some(HistoryPickerState {
            entries: (0..60).map(|i| format!("entry {i}")).collect(),
            selected: 59,
            scroll_offset: 0,
        });
    });
    harness.run();
    let dialog = harness
        .get_by_role_and_label(Role::Dialog, HISTORY_TITLE)
        .rect();
    assert!(inside_window(dialog), "dialog {dialog:?} in {WINDOW:?}");
    let close = harness.get_by_role_and_label(Role::Button, CLOSE).rect();
    assert!(inside_window(close), "Close {close:?} in {WINDOW:?}");
}
