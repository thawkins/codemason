//! The empty state of the Messages pane.
//!
//! A session with no messages shows the product name, the version, three
//! starter actions and one tip instead of a blank pane. A starter action
//! fills the prompt with its text and leaves sending to the user.
//!
//! Dependencies: `egui` for the widgets, `codemason-app-core` for the `App`
//! whose prompt the actions fill.

use codemason_app_core::app::App;
use egui::accesskit::Role;
use egui::{RichText, WidgetInfo};

use crate::palette::Palette;

/// Accessible name of the empty-state group.
pub const EMPTY_STATE_LABEL: &str = "Welcome";

/// The product name shown as the heading.
pub const WORDMARK: &str = "codemason";

/// The tip shown under the starter actions.
pub const TIP: &str = "Tip: type / for commands, @ to mention a file, ! to run a shell command.";

/// The starter actions: the button label and the text it puts in the prompt.
pub const STARTERS: [(&str, &str); 3] = [
    (
        "Explain this project",
        "Explain what this project does and how it is organised.",
    ),
    ("List the commands", "/help"),
    ("Choose a model", "/model"),
];

/// Whether the focused session should show the empty state.
#[must_use]
pub fn is_empty_session(app: &App) -> bool {
    app.session.messages.is_empty()
}

/// Draw the empty state.
///
/// # Arguments
///
/// * `ui` - the Messages pane's ui; the block is drawn at its top.
/// * `app` - the App whose prompt a starter action fills.
/// * `palette` - the active palette.
pub fn empty_state_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    let group = ui.vertical(|ui| {
        ui.add_space(24.0);
        ui.label(RichText::new(WORDMARK).heading().color(palette.accent));
        ui.label(
            RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(palette.muted),
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            for (label, text) in STARTERS {
                // Click-only: a button that can hold the keyboard focus would
                // share Enter with the prompt, which sends the draft.
                let starter = egui::Button::new(label).sense(egui::Sense::CLICK);
                if ui.add(starter).clicked() {
                    app.session.input = text.to_string();
                    // Moves the cursor to the end and rebuilds the completion
                    // menus for the new draft, so a menu opened for the old
                    // one cannot act on Enter.
                    app.input_edited(text.chars().count());
                    ui.ctx()
                        .memory_mut(|memory| memory.request_focus(super::prompt::prompt_id()));
                }
            }
        });
        ui.add_space(12.0);
        ui.label(RichText::new(TIP).color(palette.muted));
        ui.add_space(12.0);
    });
    group
        .response
        .widget_info(|| WidgetInfo::labeled(Role::Group, true, EMPTY_STATE_LABEL));
}
