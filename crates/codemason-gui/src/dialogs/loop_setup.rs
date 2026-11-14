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
//! This dialog draws the form as the App holds it, the active field marked
//! as the TUI marks it. Clicking an agent moves the agent highlight (the
//! state Up/Down move); clicking the checkpoints value moves to that field
//! and sends Space. Start sends Enter and Cancel sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::{App, LoopSetupField};
use codemason_app_core::input_event::Key;

use super::{key_buttons, modal, plain, send_key};

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

/// The TUI's active-field marker.
fn marker(active: bool) -> &'static str {
    if active { "> " } else { "  " }
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
    let mut click: Option<Click> = None;
    modal(ctx, "codemason_loop_dialog", LOOP_TITLE, |ui| {
        ui.strong("Start a goal-driven loop");
        let agent_active = state.active_field == LoopSetupField::Agent;
        let agents = ui.horizontal_wrapped(|ui| {
            ui.monospace(format!("{}agent", marker(agent_active)));
            for (i, (name, _)) in state.agents.iter().enumerate() {
                if ui
                    .selectable_label(i == state.selected_agent, name.as_str())
                    .clicked()
                {
                    click = Some(Click::Agent(i));
                }
            }
        });
        agents
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, AGENTS_LABEL));
        egui::Grid::new("codemason_loop_fields")
            .num_columns(2)
            .show(ui, |ui| {
                for (label, field) in TEXT_FIELDS {
                    let active = state.active_field == field;
                    ui.monospace(format!("{}{label}", marker(active)));
                    let text = state.field_ref(field).map_or("", |f| f.text());
                    ui.monospace(text);
                    ui.end_row();
                }
                let active = state.active_field == LoopSetupField::Checkpoints;
                ui.monospace(format!("{}{CHECKPOINTS_LABEL}", marker(active)));
                let value = if state.checkpoints { "on" } else { "off" };
                if ui.selectable_label(state.checkpoints, value).clicked() {
                    click = Some(Click::Checkpoints);
                }
                ui.end_row();
            });
        if let Some(error) = state.error.as_deref() {
            ui.colored_label(ui.visuals().error_fg_color, format!("error: {error}"));
        }
        ui.add_space(6.0);
        key_buttons(ui, &[(START, plain(Key::Enter)), (CANCEL, plain(Key::Esc))]);
        ui.weak("Tab next field - Up/Down navigate - Enter start - Esc cancel");
    });
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
