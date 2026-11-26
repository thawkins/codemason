//! The `/loop` setup dialog, over `App::loop_setup`.
//!
//! app-core runs the dialog as a key-driven form
//! (`handle_loop_setup_key` in `app/loop_dialog.rs`): Tab and Shift+Tab move
//! between the fields, Up/Down pick the agent or toggle checkpoints (and move
//! between text fields), Space toggles checkpoints, typed characters fill the
//! focused text field, Enter validates and starts the loop (a missing goal
//! keeps the dialog open with an error) and Esc cancels, keeping the values
//! for the next `/loop`. The keys reach the App through the router, as in
//! the TUI.
//!
//! Typed text the router cannot pair with a key (AltGr and Option
//! characters, dead keys, IME commits) is forwarded as its characters by the
//! router (`Focus::Modal`), and a paste into a text field is typed
//! character by character by [`prepare`], so the form gets every character
//! once, through the same key path as the TUI. (The fields stay key-driven, like the provider
//! fields; binding text editors to them would give two editors of one
//! field.)
//!
//! The form is drawn as a Material dialog: each text field is an outlined
//! read-only field ([`outlined_value`]) showing what the App holds, the
//! active one drawn focused (with the caret the TUI draws as `> `), the
//! agent picker is a Material list with the highlight on the selected agent,
//! and the checkpoints switch is a filter chip. Clicking an agent moves the
//! agent highlight (the state Up/Down move); clicking the chip moves to that
//! field and sends Space. Start sends Enter and Cancel sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::{App, LoopSetupField};
use codemason_app_core::input_event::Key;

use super::{
    DIALOG_CHROME, body_height, material_actions, material_hint, material_modal, plain, send_key,
};
use crate::material::chip::{ChipKind, chip};
use crate::material::list::list_item;
use crate::material::text_field::outlined_value;
use crate::metrics::{space, text_size};
use crate::palette::{palette_for, rgb};

/// Title and accessible label of the dialog (the TUI's border title).
pub const LOOP_TITLE: &str = "/loop setup";

/// The button that validates and starts the loop (Enter).
pub const START: &str = "Start";

/// The button that cancels, keeping the values (Esc).
pub const CANCEL: &str = "Cancel";

/// Accessible label of the agent list.
pub const AGENTS_LABEL: &str = "Loop agent";

/// The checkpoints toggle.
pub const CHECKPOINTS_LABEL: &str = "checkpoints";

/// Width of the dialog's content, in points.
const WIDTH: f32 = 480.0;

/// Tallest the agent list grows before it scrolls, in points.
const AGENT_LIST_HEIGHT: f32 = 3.0 * crate::material::list::TWO_LINE_HEIGHT;

/// The text fields in display order, with the TUI's labels.
const TEXT_FIELDS: [(&str, LoopSetupField); 7] = [
    ("goal", LoopSetupField::Goal),
    ("verify cmd", LoopSetupField::VerifyCmd),
    ("scope", LoopSetupField::Scope),
    ("read-only", LoopSetupField::ReadOnly),
    ("tools", LoopSetupField::ToolSet),
    ("max steps", LoopSetupField::MaxSteps),
    ("cost limit", LoopSetupField::CostLimit),
];

/// Type this frame's pastes into the focused text field, as key presses.
///
/// app-core has no paste path for the form, so while a text field is active
/// each pasted character is sent as its own [`Key::Char`] through
/// `App::handle_key` ([`send_key`]): the same path as typing, and once.
/// Carriage returns are dropped, and line breaks and tabs become spaces,
/// since the fields are single lines and Enter would start the loop. On the
/// agent picker or the checkpoints toggle a paste is ignored: typed as keys
/// it would pick agents or toggle checkpoints. The paste is removed from the
/// input either way, so it never reaches the prompt behind the dialog.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose active loop field is read.
pub fn prepare(ctx: &egui::Context, app: &App) {
    let mut pasted = String::new();
    ctx.input_mut(|input| {
        input.events.retain(|event| match event {
            egui::Event::Paste(text) => {
                pasted.push_str(text);
                false
            }
            _ => true,
        });
    });
    let on_text_field = app.loop_setup.as_ref().is_some_and(|state| {
        TEXT_FIELDS
            .iter()
            .any(|(_, field)| *field == state.active_field)
    });
    if !on_text_field {
        return;
    }
    for c in pasted.chars().filter(|&c| c != '\r') {
        let c = if c == '\n' || c == '\t' { ' ' } else { c };
        send_key(ctx, plain(Key::Char(c)));
    }
}

/// What a click in the dialog asks for.
enum Click {
    Agent(usize),
    Checkpoints,
}

/// Draw the `/loop` setup dialog while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `loop_setup` is drawn.
///
/// # Returns
///
/// `true` while the dialog is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(state) = app.loop_setup.as_ref() else {
        return false;
    };
    let palette = palette_for(app, ctx);
    let mut click: Option<Click> = None;
    material_modal(
        ctx,
        &palette,
        "codemason_loop_dialog",
        LOOP_TITLE,
        WIDTH,
        |ui| {
            material_hint(ui, &palette, "Start a goal-driven loop");
            ui.add_space(space::SM);
            // The agent picker: a Material list, capped so a long agent
            // roster scrolls instead of pushing the form off the dialog.
            let height = AGENT_LIST_HEIGHT.min(body_height(ctx, 0.3, DIALOG_CHROME));
            let list = egui::ScrollArea::vertical()
                .id_salt("codemason_loop_agents")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, (name, description)) in state.agents.iter().enumerate() {
                            let hint = if description.is_empty() {
                                None
                            } else {
                                Some(description.as_str())
                            };
                            if list_item(ui, &palette, name, hint, None, i == state.selected_agent)
                                .clicked()
                            {
                                click = Some(Click::Agent(i));
                            }
                        }
                    })
                    .response
                });
            list.inner
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, AGENTS_LABEL));
            ui.add_space(space::LG);
            // The form: the goal on its own line, the rest two to a row, so
            // all seven fields and the actions fit a normal window.
            let (goal_label, goal_field) = TEXT_FIELDS[0];
            outlined_value(
                ui,
                &palette,
                goal_label,
                state.field_ref(goal_field).map_or("", |f| f.text()),
                state.active_field == goal_field,
                None,
                false,
            );
            ui.add_space(space::MD);
            for pair in TEXT_FIELDS[1..].chunks(2) {
                ui.columns(2, |columns| {
                    for (column, (label, field)) in columns.iter_mut().zip(pair) {
                        outlined_value(
                            column,
                            &palette,
                            label,
                            state.field_ref(*field).map_or("", |f| f.text()),
                            state.active_field == *field,
                            None,
                            false,
                        );
                    }
                });
                ui.add_space(space::MD);
            }
            let checkpoints_active = state.active_field == LoopSetupField::Checkpoints;
            if ui
                .horizontal(|ui| {
                    chip(
                        ui,
                        &palette,
                        ChipKind::Filter {
                            selected: state.checkpoints,
                        },
                        None,
                        CHECKPOINTS_LABEL,
                    )
                })
                .inner
                .clicked()
            {
                click = Some(Click::Checkpoints);
            }
            if checkpoints_active {
                // The active row is the chip's, whose own state carries the
                // focus mark; the hint says what Space does.
                material_hint(ui, &palette, "Space toggles the checkpoints");
            }
            if let Some(error) = state.error.as_deref() {
                ui.add_space(space::SM);
                ui.label(
                    egui::RichText::new(format!("error: {error}"))
                        .size(text_size::BODY)
                        .color(rgb(palette.m3.error)),
                );
            }
            ui.add_space(space::LG);
            material_actions(
                ui,
                &palette,
                &[(START, plain(Key::Enter)), (CANCEL, plain(Key::Esc))],
                true,
            );
            ui.add_space(space::MD);
            material_hint(
                ui,
                &palette,
                "Tab next field - Up/Down navigate - Enter start - Esc cancel",
            );
        },
    );
    match click {
        Some(Click::Agent(i)) => {
            if let Some(state) = app.loop_setup.as_mut() {
                state.selected_agent = i;
            }
        }
        Some(Click::Checkpoints) => {
            if let Some(state) = app.loop_setup.as_mut() {
                state.active_field = LoopSetupField::Checkpoints;
            }
            send_key(ctx, plain(Key::Char(' ')));
        }
        None => {}
    }
    true
}
