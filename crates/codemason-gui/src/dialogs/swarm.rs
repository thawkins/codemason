//! The Swarm dialog: asks for the goal a `/swarm` run decomposes.
//!
//! The Agent tab's Swarm ([`crate::ribbon`]) opens it ([`open`]); [`show`]
//! draws a Material dialog with one outlined text field for the prompt, a
//! Cancel and an Ok button. Ok (or Enter in the field) with a non-blank
//! prompt closes it and hands the shell the line `/swarm <prompt>`
//! ([`swarm_command`]) to run; Cancel, Esc or a click outside closes it and
//! runs nothing. While it is open the shell sends no keys or menu commands
//! to the App ([`is_open`]), so typing goes to the field.
//!
//! Whether it is open, and the prompt typed so far, are GUI state (egui
//! memory), not App state: no other frontend draws it.
//!
//! Dependencies: `egui`, and this crate's Material components and palette.

use egui::Id;

use crate::dialogs::material_modal_response;
use crate::material::button::{ButtonKind, button};
use crate::material::dialog::dialog_actions;
use crate::material::text_field::outlined_text_field;
use crate::metrics::space;
use crate::palette::Palette;

/// Title of the dialog, also its accessible name.
pub const SWARM_TITLE: &str = "Swarm";

/// Label of the prompt field, also its accessible name.
pub const PROMPT_LABEL: &str = "Swarm prompt";

/// The button that runs `/swarm` with the prompt.
pub const OK_SWARM: &str = "Ok";

/// The button that closes the dialog without running anything.
pub const CANCEL_SWARM: &str = "Cancel";

/// Width of the dialog, in points.
const WIDTH: f32 = 520.0;

/// The slash command that runs a swarm for `prompt`.
///
/// # Arguments
///
/// * `prompt` - the goal as typed; surrounding whitespace is dropped.
///
/// # Returns
///
/// `/swarm <prompt>`, or `None` for a blank prompt.
#[must_use]
pub fn swarm_command(prompt: &str) -> Option<String> {
    let prompt = prompt.trim();
    (!prompt.is_empty()).then(|| format!("/swarm {prompt}"))
}

/// The egui memory slot of the prompt; present while the dialog is open.
fn state_id() -> Id {
    Id::unique("codemason_swarm")
}

/// The egui memory slot set by [`open`] until [`show`] focuses the field.
fn focus_id() -> Id {
    Id::unique("codemason_swarm_focus")
}

/// Open the Swarm dialog with an empty prompt, its field focused.
///
/// # Arguments
///
/// * `ctx` - the window's context.
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(state_id(), String::new()));
    ctx.data_mut(|d| d.insert_temp(focus_id(), true));
}

/// Whether the Swarm dialog is open.
///
/// # Arguments
///
/// * `ctx` - the window's context.
#[must_use]
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<String>(state_id())).is_some()
}

/// Draw the dialog while it is open.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `palette` - the colours.
///
/// # Returns
///
/// The `/swarm <prompt>` line to run when Ok was chosen with a non-blank
/// prompt this frame (the dialog is then closed), else `None`.
pub fn show(ctx: &egui::Context, palette: &Palette) -> Option<String> {
    let mut prompt = ctx.data(|d| d.get_temp::<String>(state_id()))?;
    let mut ok = false;
    let mut cancel = false;
    let modal = material_modal_response(
        ctx,
        palette,
        "codemason_swarm_modal",
        SWARM_TITLE,
        WIDTH,
        |ui| {
            let field = outlined_text_field(
                ui,
                palette,
                PROMPT_LABEL,
                &mut prompt,
                Some("The goal to split into parallel subtasks"),
                false,
            );
            // Focus the field once it exists, on the dialog's first frame.
            if ui
                .ctx()
                .data_mut(|d| d.remove_temp::<bool>(focus_id()))
                .is_some()
            {
                field.request_focus();
            }
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(space::LG);
            let ready = swarm_command(&prompt).is_some();
            dialog_actions(ui, |ui| {
                let ok_button = ui
                    .add_enabled_ui(ready, |ui| {
                        button(ui, palette, ButtonKind::Filled, None, OK_SWARM)
                    })
                    .inner;
                ok = (ok_button.clicked() || entered) && ready;
                cancel = button(ui, palette, ButtonKind::Text, None, CANCEL_SWARM).clicked();
            });
        },
    );
    // Esc or a click outside cancels too.
    if ok || cancel || modal.should_close() {
        ctx.data_mut(|d| d.remove::<String>(state_id()));
        return if ok { swarm_command(&prompt) } else { None };
    }
    ctx.data_mut(|d| d.insert_temp(state_id(), prompt));
    None
}
