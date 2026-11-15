//! Tests for the macOS app menu (`codemason_gui::platform::menu`).
//!
//! The menu model, its Mac layout and the command mapping are platform-free,
//! so they are tested on every platform; only the muda backend that shows
//! the menu is macOS-only. A stand-in [`Backend`] reports menu clicks into a
//! shell, as the macOS backend does between frames.

#[path = "support/mod.rs"]
mod support;

use std::sync::{Arc, Mutex};

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use codemason_gui::platform::menu::{
    ABOUT_COMMAND, MenuAction, mac_menu_layout, menu_action, menu_bar,
};
use codemason_gui::platform::window::QUIT_TITLE;
use craft_keymap::{Chord, CommandRef, Platform};
use craft_menu::{Backend, Builtin, Dispatch, Event, ItemRole, MenuBar, MenuRole, NativeMenu};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The panel toggles the keymap defines, each of which the View menu lists.
const PANELS: [&str; 4] = ["panel.log", "panel.profile", "panel.tasks", "panel.memory"];

/// The items directly in `menu`, in order.
fn items(menu: &craft_menu::Menu) -> Vec<&craft_menu::Item> {
    menu.children
        .iter()
        .filter_map(|node| match node {
            craft_menu::Node::Item(item) => Some(item),
            _ => None,
        })
        .collect()
}

#[test]
fn test_menu_bar_lists_about_and_the_panel_toggles_with_their_shortcuts() {
    let bar = menu_bar(&default_keymap(Platform::Mac));
    let about = bar
        .find(&CommandRef::new(ABOUT_COMMAND))
        .expect("About item");
    assert_eq!(about.role, Some(ItemRole::About));
    let view = bar.menu(MenuRole::View).expect("View menu");
    let commands: Vec<String> = items(view).iter().map(|i| i.command.id.clone()).collect();
    assert_eq!(commands, PANELS.map(String::from).to_vec());
    let log = bar.find(&CommandRef::new("panel.log")).expect("log item");
    assert_eq!(
        log.shortcut,
        Some(Chord::parse("Cmd+Shift+L").expect("chord")),
        "the keymap's macOS chord"
    );
}

#[test]
fn test_mac_layout_has_about_and_a_quit_that_asks_the_window_to_close() {
    let layout = mac_menu_layout(&default_keymap(Platform::Mac));
    let app_menu = layout.bar.menu(MenuRole::App).expect("app menu");
    let items = items(app_menu);
    assert!(
        items
            .iter()
            .any(|i| i.command == CommandRef::new(ABOUT_COMMAND)),
        "About in the app menu"
    );
    let quit = items
        .iter()
        .find(|i| i.role == Some(ItemRole::Quit))
        .expect("Quit item");
    assert_eq!(
        quit.command,
        Builtin::Quit.command(),
        "Quit closes the window"
    );
    assert_eq!(quit.shortcut, Some(Chord::parse("Cmd+Q").expect("chord")));
    assert!(layout.clashes.is_empty(), "{:?}", layout.clashes);
}

#[test]
fn test_menu_commands_run_the_keymap_chord_or_about() {
    assert_eq!(
        menu_action(&CommandRef::new("panel.log")),
        Some(MenuAction::Key(KeyInput {
            key: Key::Char('l'),
            mods: Modifiers::ALT,
        }))
    );
    assert_eq!(
        menu_action(&CommandRef::new(ABOUT_COMMAND)),
        Some(MenuAction::Slash("/about"))
    );
    assert_eq!(menu_action(&CommandRef::new("no.such.command")), None);
}

/// A native menu stand-in: the test pushes the events the macOS backend
/// would report, and the driver drains them.
#[derive(Default)]
struct FakeMenu {
    events: Arc<Mutex<Vec<Event>>>,
}

impl Backend for FakeMenu {
    fn sync(&mut self, _bar: &MenuBar) {}
    fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut *self.events.lock().expect("events"))
    }
    fn attach(&mut self, _dispatch: Dispatch) {}
}

/// A shell with a stand-in native menu, the queue its clicks go into, and
/// the runtime driving it.
fn rig(
    configure: impl FnOnce(&mut codemason_app_core::app::App),
) -> (
    tokio::runtime::Runtime,
    Arc<Mutex<Vec<Event>>>,
    Harness<'static, Shell>,
) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some("session-menu".to_string());
        configure(&mut app);
        app
    };
    let fake = FakeMenu::default();
    let events = Arc::clone(&fake.events);
    let (_events_tx, agent_events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let keymap = default_keymap(Platform::Mac);
    let shell = Shell::new(app)
        .with_keymap(keymap.clone())
        .with_sources(agent_events, logs, runtime.handle().clone())
        .with_native_menu(NativeMenu::new(Box::new(fake)), &keymap);
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(
            |ui, shell: &mut Shell| {
                let mut raw = egui::RawInput::default();
                shell.native_menu_input(&mut raw);
                for event in raw.events {
                    ui.ctx().input_mut(|i| i.events.push(event));
                }
                shell.update(ui);
            },
            shell,
        );
    (runtime, events, harness)
}

#[test]
fn test_menu_click_toggles_a_panel_like_its_shortcut() {
    let (_runtime, events, mut harness) = rig(|_| {});
    harness.run();
    assert!(!harness.state().app().show_log);

    events
        .lock()
        .expect("events")
        .push(Event::Click(CommandRef::new("panel.log")));
    harness.step();
    harness.step();

    assert!(
        harness.state().app().show_log,
        "View > Toggle Log Panel ran"
    );
}

#[test]
fn test_menu_quit_takes_the_close_path() {
    let (_runtime, events, mut harness) = rig(|app| app.session.is_processing = true);
    harness.run();

    // Cmd+Q on the built-in Quit item asks the window to close; with a
    // running turn the close confirmation answers it.
    events
        .lock()
        .expect("events")
        .push(Event::Click(Builtin::Quit.command()));
    harness.step();
    let close_sent = harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .is_some_and(|o| o.commands.contains(&egui::ViewportCommand::Close));
    assert!(close_sent, "Quit asks the window to close");
    harness
        .input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    harness.step();
    harness.step();
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, QUIT_TITLE)
            .is_some()
    );
    assert!(harness.state().app().is_running);
}
