//! The macOS app menu, built with craft-menu.
//!
//! On macOS the window gets a native menu bar (`install`, `cfg(target_os =
//! "macos")`): the app menu (About codemason, Services, Hide, Hide Others,
//! Show All, Quit codemason with Cmd+Q), a View menu with the panel toggles
//! the keymap defines (with their keymap chords as shortcuts), and AppKit's
//! Window and Help menus. Elsewhere the window's own ribbon
//! ([`crate::ribbon`]) carries the commands and nothing here is installed; the model ([`menu_bar`]), its Mac layout
//! ([`mac_menu_layout`]) and the command mapping ([`menu_action`]) are
//! platform-free, so they build and are tested everywhere.
//!
//! What each item runs:
//!
//! - **Quit** (Cmd+Q) is craft-menu's built-in Quit, which asks the window
//!   to close (`craft_quit::on_menu_event`): the shell's close path decides,
//!   asking first while an agent turn runs ([`super::window`]).
//! - **About** opens the About dialog ([`crate::dialogs::about`]).
//! - **A panel toggle** sends the App the TUI chord of its keymap command
//!   (`keys::command_key_input`), as the keyboard shortcut does. Its
//!   shortcut, pressed, comes back to egui as the key press it was
//!   (craft-menu's key equivalents), so the key router runs it.
//!
//! Dependencies: `craft-menu` (model, Mac layout, driver; muda backend on
//! macOS), `craft-keymap` (the shortcuts) and `codemason-app-core`.

use codemason_app_core::input_event::KeyInput;
use craft_keymap::{CommandRef, Keymap};
use craft_menu::{Item, ItemRole, Layout, LayoutOptions, Menu, MenuBar, MenuRole, Node};

use crate::keys::command_key_input;

/// The app's name in the menu ("About codemason", "Quit codemason").
pub const APP_NAME: &str = "codemason";

/// The command of the About item.
pub const ABOUT_COMMAND: &str = "app.about";

/// The panel-toggle commands of the keymap, in the View menu's order.
const PANEL_COMMANDS: [&str; 4] = ["panel.log", "panel.profile", "panel.tasks", "panel.memory"];

/// What a menu command runs in the App.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Send this key press through `App::handle_key`.
    Key(KeyInput),
    /// Run this slash command through `App::execute_slash_command`.
    Slash(&'static str),
    /// Open the About dialog ([`crate::dialogs::about`]).
    About,
}

/// The app's menus, before the platform layout: About, and a View menu with
/// the keymap's panel toggles, each labelled and bound as in `keymap`.
///
/// # Arguments
///
/// * `keymap` - the window's keymap, user overrides included.
///
/// # Returns
///
/// The menu bar craft-menu lays out.
#[must_use]
pub fn menu_bar(keymap: &Keymap) -> MenuBar {
    let panels = PANEL_COMMANDS
        .into_iter()
        .map(|id| {
            let command = CommandRef::new(id);
            let label = keymap
                .bindable(&command)
                .map_or_else(|| id.to_string(), |b| b.label.clone());
            Node::Item(Item::new(command, label))
        })
        .collect();
    let mut bar = MenuBar {
        menus: vec![
            Menu::new(
                APP_NAME,
                vec![Node::Item(
                    Item::new(CommandRef::new(ABOUT_COMMAND), "About").role(ItemRole::About),
                )],
            )
            .role(MenuRole::App),
            Menu::new("View", panels).role(MenuRole::View),
        ],
        documents: Vec::new(),
    };
    craft_menu::apply_keymap(&mut bar, keymap);
    bar
}

/// [`menu_bar`] laid out the macOS way (`craft_menu::mac_layout`): the app
/// menu gains Services, Hide, Hide Others, Show All and the built-in Quit
/// (Cmd+Q), and the Window and Help menus are added.
///
/// # Arguments
///
/// * `keymap` - the window's keymap.
///
/// # Returns
///
/// The layout the native menu shows.
#[must_use]
pub fn mac_menu_layout(keymap: &Keymap) -> Layout {
    craft_menu::mac_layout(&menu_bar(keymap), &LayoutOptions::new(APP_NAME))
}

/// What choosing the item of `command` runs.
///
/// # Arguments
///
/// * `command` - the command a menu click reported.
///
/// # Returns
///
/// The action, or `None` for a command the menu does not hold.
#[must_use]
pub fn menu_action(command: &CommandRef) -> Option<MenuAction> {
    if *command == CommandRef::new(ABOUT_COMMAND) {
        return Some(MenuAction::About);
    }
    command_key_input(command).map(MenuAction::Key)
}

/// Install the native menu bar. Call it once, on the main thread, in the
/// eframe creator closure (winit builds its default menu before that).
///
/// # Arguments
///
/// * `ctx` - the window's context; the built-in Quit asks it to close.
/// * `keymap` - the window's keymap.
///
/// # Returns
///
/// The driver the shell runs each frame, or `None` when AppKit refused the
/// menus (winit's default menu then stays, whose Quit does not ask).
#[cfg(target_os = "macos")]
#[must_use]
pub fn install(ctx: &egui::Context, keymap: &Keymap) -> Option<craft_menu::NativeMenu> {
    craft_menu::mac::install(ctx, &mac_menu_layout(keymap).bar)
}
