//! The window's modal dialogs, drawn over the App's modal state.
//!
//! # One dispatch
//!
//! app-core owns every dialog's state and decides what each key does
//! (`codemason_app_core::input::handle_key`). [`active`] asks the App which
//! dialog its key handling serves now, in `handle_key`'s own priority order,
//! and [`show`] draws that one dialog, so the dialog on screen is always the
//! one the keyboard reaches. A state this crate does not draw yet (or that
//! another panel draws, such as the queue menu) that `handle_key` serves
//! first hides the dialogs below it, as it takes their keys.
//!
//! # Acting
//!
//! A dialog never decides anything itself. A button sends the TUI key press
//! that does the same thing ([`send_key`]): the shell runs the queued presses
//! through `App::handle_key` after drawing ([`take_keys`]), so a click and a
//! key press take one path through app-core. A dialog may move its list's
//! highlight (the state a TUI arrow key moves) before sending Enter.
//!
//! # Keyboard
//!
//! While a dialog is open the App's input is locked, so the key router gets
//! [`crate::keys::Focus::Modal`] from the prompt: every press goes to the App
//! as in the TUI and egui never sees it, except the typing and editing keys of
//! a dialog's own text field (the model filter). [`prepare`] runs before the
//! router for the one dialog that needs it (the model picker's filtered
//! list). A closing dialog gives egui's keyboard focus back to the prompt, so
//! typing goes on there.
//!
//! Dependencies: `codemason-app-core` (the App and its key types), `egui`
//! (`Modal`) and `craft-ui` (`ComboBox`).

pub mod model_picker;
pub mod permission;
pub mod provider;
pub mod shortcuts;

use codemason_app_core::app::{App, ProviderSetupStep};
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use egui::Id;

use crate::panels::prompt::prompt_id;

/// The dialog the App's key handling serves now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveDialog {
    /// The `?` keyboard shortcuts panel (`App::show_shortcuts`).
    Shortcuts,
    /// The provider setup flow (`App::provider_setup`), except its model
    /// lists.
    Provider,
    /// The model lists of the provider setup flow
    /// (`ProviderSetupStep::SelectModel` and `SelectRouterModel`).
    ModelPicker,
    /// The front permission request (`session.permission_queue`).
    Permission,
}

/// Which dialog the App's key handling serves now, if this crate draws it.
///
/// Follows the order of `App::handle_key` (`input_handler.rs`, then
/// `input.rs`): a modal state checked before a dialog takes its keys, so
/// while one is set (drawn elsewhere or not ported yet) the dialogs below it
/// are not drawn.
///
/// # Arguments
///
/// * `app` - the App whose modal state is read.
///
/// # Returns
///
/// The dialog to draw, or `None` when no drawn dialog has the keyboard.
#[must_use]
pub fn active(app: &App) -> Option<ActiveDialog> {
    let earlier = app.config_save_picker.is_some()
        || app.history_picker.is_some()
        || app.row_action_confirm.is_some()
        || app.pending_memory_delete.is_some()
        || app.pending_router_save.is_some()
        || app.plugin_store.is_some()
        || app.connector_store.is_some()
        || app.backend_panel.is_some()
        || app.queue_show_open
        || app.queue_menu_open
        || app.context_menu.is_some();
    if earlier {
        return None;
    }
    if app.show_shortcuts {
        return Some(ActiveDialog::Shortcuts);
    }
    if let Some(step) = app.provider_setup.as_ref() {
        return Some(match step {
            ProviderSetupStep::SelectModel { .. } | ProviderSetupStep::SelectRouterModel { .. } => {
                ActiveDialog::ModelPicker
            }
            _ => ActiveDialog::Provider,
        });
    }
    let between = app.mcp_discover.is_some()
        || app.plugins_list.is_some()
        || app.loop_setup.is_some()
        || !app.session.question_queue.is_empty();
    if between {
        return None;
    }
    if !app.session.permission_queue.is_empty() {
        return Some(ActiveDialog::Permission);
    }
    None
}

/// Draw the active dialog ([`active`]), if any.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose dialog state is drawn.
///
/// # Returns
///
/// `true` while a dialog is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let dialog = active(app);
    if dialog != Some(ActiveDialog::ModelPicker) {
        model_picker::reset(ctx);
    }
    let was_open = ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false);
    if dialog.is_none() && was_open && ctx.memory(|m| m.focused().is_none()) {
        // A closed dialog gives the keyboard back to the prompt, where the
        // TUI's keyboard always is. (The prompt lost it when the modal took
        // the input and the prompt stopped being editable.)
        ctx.memory_mut(|m| m.request_focus(prompt_id()));
    }
    ctx.data_mut(|d| d.insert_temp(open_id(), dialog.is_some()));
    match dialog {
        Some(ActiveDialog::Shortcuts) => shortcuts::show(ctx, app),
        Some(ActiveDialog::Provider) => provider::show(ctx, app),
        Some(ActiveDialog::ModelPicker) => model_picker::show(ctx, app),
        Some(ActiveDialog::Permission) => permission::show(ctx, app),
        None => false,
    }
}

/// Let the active dialog handle this frame's input before the key router.
///
/// Only the model picker uses it: while its filter hides rows, Up and Down
/// move between the shown rows and Enter on an empty list does nothing (see
/// [`model_picker::prepare`]).
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose dialog state is read.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
    if active(app) == Some(ActiveDialog::ModelPicker) {
        model_picker::prepare(ctx, app);
    }
}

/// The egui memory slot of whether a dialog was open last frame.
fn open_id() -> Id {
    Id::unique("codemason_dialog_open")
}

/// The egui memory slot of the queued key presses.
fn keys_id() -> Id {
    Id::unique("codemason_dialog_keys")
}

/// Queue a key press for `App::handle_key`, as if typed: the shell runs the
/// queue after drawing ([`take_keys`]).
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `key` - the TUI key press a dialog control stands for.
pub fn send_key(ctx: &egui::Context, key: KeyInput) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<KeyInput>>(keys_id())
            .push(key);
    });
    ctx.request_repaint();
}

/// Take the key presses the dialogs queued this frame.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
///
/// # Returns
///
/// The presses in the order they were queued; the queue is left empty.
pub fn take_keys(ctx: &egui::Context) -> Vec<KeyInput> {
    ctx.data_mut(|d| d.remove_temp::<Vec<KeyInput>>(keys_id()))
        .unwrap_or_default()
}

/// A TUI key press of `key` with no modifiers.
pub(crate) const fn plain(key: Key) -> KeyInput {
    KeyInput {
        key,
        mods: Modifiers::NONE,
    }
}

/// Draw a modal dialog titled `title`, with the title as its accessible
/// label (`Role::Dialog`) and as a heading.
///
/// The App's key handling decides when the dialog closes, so the modal's own
/// close requests (Esc, a backdrop click) are not used.
pub(crate) fn modal<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Modal::new(Id::unique(id))
        .show(ctx, |ui| {
            let inner = ui.vertical(|ui| {
                ui.heading(title);
                ui.add_space(4.0);
                add_contents(ui)
            });
            inner
                .response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Dialog, true, title));
            inner.inner
        })
        .inner
}

/// A row of dialog buttons, each sending its key press when clicked.
pub(crate) fn key_buttons(ui: &mut egui::Ui, buttons: &[(&str, KeyInput)]) {
    ui.horizontal(|ui| {
        for (label, key) in buttons {
            if ui.button(*label).clicked() {
                send_key(ui.ctx(), *key);
            }
        }
    });
}
