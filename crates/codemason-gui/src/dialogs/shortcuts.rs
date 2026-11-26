//! The `?` keyboard shortcuts panel.
//!
//! Lists `codemason_app_core::shortcuts::KEYBINDINGS`, the table the TUI's
//! `?` panel shows, while `App::show_shortcuts` is set. `?` on an empty
//! prompt opens it (the key router forwards that `?`, see
//! [`crate::keys::is_prompt_app_key`]); Esc or `?` closes it, as in the TUI,
//! and the Close button sends Esc.
//!
//! The table is drawn as Material two-line list items: the description is the
//! row's headline and the key chord its trailing text, so a long chord never
//! squeezes the description. One filled `Close` is the panel's only action.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::shortcuts::KEYBINDINGS;

use super::{body_height, material_actions, material_hint, material_modal, plain};
use crate::material::list::list_item;
use crate::metrics::space;
use crate::palette::palette_for;

/// Title and accessible label of the panel.
pub const SHORTCUTS_TITLE: &str = "Keyboard shortcuts";

/// The button that closes the panel (Esc).
pub const CLOSE: &str = "Close";

/// Width of the panel's content, in points.
const WIDTH: f32 = 480.0;

/// Share of the window the key table may take before it scrolls.
const LIST_SHARE: f32 = 0.7;

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
    let palette = palette_for(app, ctx);
    material_modal(
        ctx,
        &palette,
        "codemason_shortcuts_panel",
        SHORTCUTS_TITLE,
        WIDTH,
        |ui| {
            let height = body_height(ctx, LIST_SHARE, super::DIALOG_CHROME);
            egui::ScrollArea::vertical()
                .id_salt("codemason_shortcuts_list")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (keys, description) in KEYBINDINGS {
                            list_item(ui, &palette, description, None, Some(keys), false);
                        }
                    });
                });
            ui.add_space(space::LG);
            material_actions(ui, &palette, &[(CLOSE, plain(Key::Esc))], true);
            ui.add_space(space::MD);
            material_hint(ui, &palette, "Press Esc or ? to close");
        },
    );
    true
}
