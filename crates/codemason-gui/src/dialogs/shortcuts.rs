//! The `?` keyboard shortcuts panel.
//!
//! Lists `codemason_app_core::shortcuts::KEYBINDINGS`, the table the TUI's
//! `?` panel shows, while `App::show_shortcuts` is set. `?` on an empty
//! prompt opens it (the key router forwards that `?`, see
//! [`crate::keys::is_prompt_app_key`]); Esc or `?` closes it, as in the TUI,
//! and the Close button sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::shortcuts::KEYBINDINGS;

use super::{key_buttons, modal, plain};

/// Title and accessible label of the panel.
pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";

/// The button that closes the panel (Esc).
pub const CLOSE: &str = "Close";

/// Draw the shortcuts panel while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App; `show_shortcuts` says whether the panel is open.
///
/// # Returns
///
/// `true` while the panel is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    if !app.show_shortcuts {
        return false;
    }
    modal(ctx, "codemason_shortcuts_panel", SHORTCUTS_TITLE, |ui| {
        let height = (ctx.content_rect().height() * 0.7).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .show(ui, |ui| {
                egui::Grid::new("codemason_shortcuts_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        for (keys, description) in KEYBINDINGS {
                            ui.strong(*keys);
                            ui.label(*description);
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(6.0);
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Press Esc or ? to close");
    });
    true
}
