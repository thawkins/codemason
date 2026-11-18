//! The sidebar's session menu, over `App::sidebar.menu` (and its open
//! `submenu`).
//!
//! app-core runs the menu (`codemason_app_core::session_menu`): Up/Down,
//! Home and End move the highlight, Enter runs the highlighted item (or
//! opens its submenu), Right opens a submenu, Left and Esc close one level,
//! and an item's accelerator letter runs it. Every action calls the same App
//! method as the sidebar's shortcut. The keys reach the App through the
//! router, as in the TUI.
//!
//! This dialog lists the menu's items in their groups, with the highlight,
//! the accelerators and the disabled (greyed out) items, and the open
//! submenu below them. Clicking an item moves the highlight to it and sends
//! Enter, so app-core runs it; clicking an item of the menu while a
//! submenu is open first closes the submenu (Left), as a click on the menu
//! does in the TUI. Close sends Esc.
//!
//! The menu is not one of the App's input locks, but it takes every key
//! while open, so the prompt treats it as one
//! ([`super::locks_prompt`]).
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::session_menu::SessionMenu;

use super::{key_buttons, modal, plain, send_key};

/// Title and accessible label of the menu.
pub const SESSION_MENU_TITLE: &str = "Session menu";

/// Accessible label of the open submenu.
pub const SUBMENU_LABEL: &str = "Submenu";

/// The button that closes one level of the menu (Esc).
pub const CLOSE: &str = "Close";

/// Draw `menu`'s items; returns the index of a clicked item.
fn items(ui: &mut egui::Ui, menu: &SessionMenu) -> Option<usize> {
    let mut clicked = None;
    for (i, item) in menu.items.iter().enumerate() {
        if i > 0 && menu.items[i - 1].group != item.group {
            ui.separator();
        }
        ui.horizontal(|ui| {
            let mut text = egui::RichText::new(item.label.as_str());
            if item.danger {
                text = text.color(ui.visuals().error_fg_color);
            }
            let row = ui.add_enabled(
                item.enabled,
                egui::Button::selectable(i == menu.cursor, text),
            );
            if row.clicked() {
                clicked = Some(i);
            }
            if let Some(accel) = item.accel {
                ui.weak(accel.to_ascii_uppercase().to_string());
            }
        });
    }
    clicked
}

/// Draw the session menu while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `sidebar.menu` is drawn.
///
/// # Returns
///
/// `true` while the menu is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return false;
    };
    let mut top: Option<usize> = None;
    let mut sub: Option<usize> = None;
    modal(ctx, "codemason_session_menu", SESSION_MENU_TITLE, |ui| {
        top = items(ui, menu);
        if let Some(submenu) = menu.submenu.as_deref() {
            ui.separator();
            let list = ui.vertical(|ui| {
                sub = items(ui, submenu);
            });
            list.response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, SUBMENU_LABEL));
        }
        ui.add_space(crate::metrics::space::SM);
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Up/Down move  Enter run  Right open  Left/Esc close  letter runs its item");
    });
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return true;
    };
    if let Some(i) = sub {
        if let Some(submenu) = menu.submenu.as_deref_mut() {
            submenu.cursor = i;
            send_key(ctx, plain(Key::Enter));
        }
    } else if let Some(i) = top {
        menu.cursor = i;
        if menu.submenu.is_some() {
            send_key(ctx, plain(Key::Left));
        }
        send_key(ctx, plain(Key::Enter));
    }
    true
}
