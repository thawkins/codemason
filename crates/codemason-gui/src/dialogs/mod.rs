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
//! While a dialog is open the App's input is locked (or, for the sidebar's
//! properties dialog and session menu, the prompt treats it as locked:
//! [`locks_prompt`]), so the key router gets [`crate::keys::Focus::Modal`]
//! from the prompt: every press goes to the App as in the TUI and egui never
//! sees it, except the typing and editing keys of a dialog's own text field
//! (the model filter, the question's answer). [`prepare`] runs before the
//! router for the dialogs that need it (the model picker's filtered list,
//! the provider and properties fields' paste, the run-cost banner's dismiss
//! rule). A closing dialog gives egui's keyboard focus back to the prompt,
//! so typing goes on there.
//!
//! The run-cost banner ([`banner`]) is not a modal: it is drawn above the
//! active dialog, if any.
//!
//! Dependencies: `codemason-app-core` (the App and its key types), `egui`
//! (`Modal`) and `craft-ui` (`ComboBox`).

pub mod banner;
pub mod confirm;
pub mod context_menu;
pub mod image_view;
pub mod lists;
pub mod loop_setup;
pub mod model_picker;
pub mod output_view;
pub mod permission;
pub mod pickers;
pub mod progress;
pub mod properties;
pub mod provider;
pub mod question;
pub mod session_menu;
pub mod shortcuts;
pub mod stores;

use codemason_app_core::app::{App, ProviderSetupStep};
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use egui::Id;

use crate::panels::prompt::prompt_id;

/// The dialog the App's key handling serves now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveDialog {
    /// The `/config list` save picker (`App::config_save_picker`).
    ConfigSavePicker,
    /// The `/theme` picker (`App::theme_picker`).
    ThemePicker,
    /// The input history picker (`App::history_picker`).
    HistoryPicker,
    /// The router configuration save confirmation
    /// (`App::pending_router_save`).
    RouterSave,
    /// The plugin-store browser (`App::plugin_store`).
    PluginStore,
    /// The connector-catalogue browser (`App::connector_store`).
    ConnectorStore,
    /// The `/backend` switcher (`App::backend_panel`).
    BackendPanel,
    /// The right-click Cut/Copy/Paste menu (`App::context_menu`).
    ContextMenu,
    /// The `?` keyboard shortcuts panel (`App::show_shortcuts`).
    Shortcuts,
    /// The provider setup flow (`App::provider_setup`), except its model
    /// lists.
    Provider,
    /// The model lists of the provider setup flow
    /// (`ProviderSetupStep::SelectModel` and `SelectRouterModel`).
    ModelPicker,
    /// The `/mcp discover` dialog (`App::mcp_discover`).
    McpDiscover,
    /// The `/plugins list` dialog (`App::plugins_list`).
    PluginsList,
    /// The `/loop` setup dialog (`App::loop_setup`).
    LoopSetup,
    /// The agent's front question (`session.question_queue`).
    Question,
    /// The front permission request (`session.permission_queue`).
    Permission,
    /// The single-button notice (`App::notice_open`).
    Notice,
    /// The `/db prune` confirmation (`App::db_prune_confirm_open`).
    DbPrune,
    /// The team force-cleanup confirmation
    /// (`session.pending_forcecleanup`).
    ForceCleanup,
    /// The post-loop rollback offer (`session.pending_rollback`).
    Rollback,
    /// The plan approval (`session.plan_approval_pending`).
    PlanApproval,
    /// The tool-output overlay (`App::output_view`).
    OutputView,
    /// The project properties dialog (`sidebar.properties`).
    Properties,
    /// The sidebar's session menu (`sidebar.menu`).
    SessionMenu,
    /// The destructive row-action confirmation (`App::row_action_confirm`).
    RowAction,
    /// The Memory panel's delete confirmation
    /// (`App::pending_memory_delete`).
    MemoryDelete,
    /// The `/memory clear` confirmation (`App::memory_clear_confirm_open`).
    MemoryClear,
    /// The full-memory view (`App::memory_view`).
    MemoryView,
}

/// Which dialog the App's key handling serves now, if this crate draws it.
///
/// Follows the order of `App::handle_key` (`dispatch_key` in
/// `input_handler.rs`, then `input::handle_key` in `input.rs`, which calls
/// the sidebar's `handle_key` last): a modal state checked before a dialog
/// takes its keys, so while one is set (drawn elsewhere or not ported yet)
/// the dialogs below it are not drawn.
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
    if app.config_save_picker.is_some() {
        return Some(ActiveDialog::ConfigSavePicker);
    }
    if app.theme_picker.is_some() {
        return Some(ActiveDialog::ThemePicker);
    }
    if app.history_picker.is_some() {
        return Some(ActiveDialog::HistoryPicker);
    }
    // `dispatch_key` serves the row-action confirmation before
    // `input::handle_key`, which serves the memory delete first.
    if app.row_action_confirm.is_some() {
        return Some(ActiveDialog::RowAction);
    }
    if app.pending_memory_delete.is_some() {
        return Some(ActiveDialog::MemoryDelete);
    }
    if app.pending_router_save.is_some() {
        return Some(ActiveDialog::RouterSave);
    }
    if app.plugin_store.is_some() {
        return Some(ActiveDialog::PluginStore);
    }
    if app.connector_store.is_some() {
        return Some(ActiveDialog::ConnectorStore);
    }
    if app.backend_panel.is_some() {
        return Some(ActiveDialog::BackendPanel);
    }
    // The queue panel draws the queue entry panel and the queue menu.
    if app.queue_show_open || app.queue_menu_open {
        return None;
    }
    if app.context_menu.is_some() {
        return Some(ActiveDialog::ContextMenu);
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
    if app.mcp_discover.is_some() {
        return Some(ActiveDialog::McpDiscover);
    }
    if app.plugins_list.is_some() {
        return Some(ActiveDialog::PluginsList);
    }
    if app.loop_setup.is_some() {
        return Some(ActiveDialog::LoopSetup);
    }
    if !app.session.question_queue.is_empty() {
        return Some(ActiveDialog::Question);
    }
    if !app.session.permission_queue.is_empty() {
        return Some(ActiveDialog::Permission);
    }
    if app.notice_open {
        return Some(ActiveDialog::Notice);
    }
    // The queue panel draws the queue clear confirm.
    if app.queue_clear_confirm_open {
        return None;
    }
    if app.memory_clear_confirm_open {
        return Some(ActiveDialog::MemoryClear);
    }
    if app.db_prune_confirm_open {
        return Some(ActiveDialog::DbPrune);
    }
    if app.session.pending_forcecleanup.is_some() {
        return Some(ActiveDialog::ForceCleanup);
    }
    if app.session.pending_rollback.is_some() {
        return Some(ActiveDialog::Rollback);
    }
    if app.session.plan_approval_pending.is_some() {
        return Some(ActiveDialog::PlanApproval);
    }
    // The prompt draws the slash and file completion menus; the research
    // view is not ported yet.
    let later = app.slash_menu.is_some() || app.file_menu.is_some() || app.research_view.is_some();
    if later {
        return None;
    }
    if app.memory_view.is_some() {
        return Some(ActiveDialog::MemoryView);
    }
    if app.output_view.is_some() {
        return Some(ActiveDialog::OutputView);
    }
    if app.sidebar.properties.is_some() {
        return Some(ActiveDialog::Properties);
    }
    if app.sidebar.menu.is_some() {
        return Some(ActiveDialog::SessionMenu);
    }
    None
}

/// Whether the prompt must give up the keyboard: an app-core modal locks
/// the input (`App::is_input_locked`), or the sidebar's properties dialog,
/// the session menu, the connector-catalogue browser, or the `/memory clear`
/// or `/db prune` confirmation is open. Those take
/// every key in app-core's `handle_key` without being input locks, so the
/// prompt treats them as locks too: the router forwards every key and the
/// prompt stops editing.
///
/// # Arguments
///
/// * `app` - the App whose modal state is read.
///
/// # Returns
///
/// `true` while the App, not the prompt, takes the keyboard.
#[must_use]
pub fn locks_prompt(app: &App) -> bool {
    app.is_input_locked()
        || app.sidebar.properties.is_some()
        || app.sidebar.menu.is_some()
        || app.connector_store.is_some()
        || app.memory_clear_confirm_open
        || app.db_prune_confirm_open
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
    if dialog != Some(ActiveDialog::OutputView) {
        output_view::reset(ctx);
    }
    let was_open = ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false);
    let unfocused = ctx.memory(|m| {
        m.focused().is_none_or(|id| {
            id == model_picker::filter_field_id() || id == question::answer_field_id()
        })
    });
    if dialog.is_none() && was_open && unfocused {
        // A closed dialog gives the keyboard back to the prompt, where the
        // TUI's keyboard always is. (The prompt lost it when the modal took
        // the input and the prompt stopped being editable.)
        ctx.memory_mut(|m| m.request_focus(prompt_id()));
    }
    ctx.data_mut(|d| d.insert_temp(open_id(), dialog.is_some()));
    // The model loading and download windows take no keys; they show
    // whenever their state is set, as in the TUI.
    progress::show(ctx, app);
    // The run-cost banner is not a modal; it sits above everything.
    banner::show(ctx, app);
    match dialog {
        Some(ActiveDialog::ConfigSavePicker) => pickers::show_config_saves(ctx, app),
        Some(ActiveDialog::ThemePicker) => pickers::show_theme(ctx, app),
        Some(ActiveDialog::HistoryPicker) => pickers::show_history(ctx, app),
        Some(ActiveDialog::RouterSave) => confirm::show_router_save(ctx, app),
        Some(ActiveDialog::PluginStore) => stores::show_plugin_store(ctx, app),
        Some(ActiveDialog::ConnectorStore) => stores::show_connector_store(ctx, app),
        Some(ActiveDialog::BackendPanel) => lists::show_backend_panel(ctx, app),
        Some(ActiveDialog::ContextMenu) => context_menu::show(ctx, app),
        Some(ActiveDialog::Shortcuts) => shortcuts::show(ctx, app),
        Some(ActiveDialog::Provider) => provider::show(ctx, app),
        Some(ActiveDialog::ModelPicker) => model_picker::show(ctx, app),
        Some(ActiveDialog::McpDiscover) => lists::show_mcp_discover(ctx, app),
        Some(ActiveDialog::PluginsList) => lists::show_plugins_list(ctx, app),
        Some(ActiveDialog::LoopSetup) => loop_setup::show(ctx, app),
        Some(ActiveDialog::Question) => question::show(ctx, app),
        Some(ActiveDialog::Permission) => permission::show(ctx, app),
        Some(ActiveDialog::Notice) => confirm::show_notice(ctx, app),
        Some(ActiveDialog::DbPrune) => confirm::show_db_prune(ctx, app),
        Some(ActiveDialog::ForceCleanup) => confirm::show_force_cleanup(ctx, app),
        Some(ActiveDialog::Rollback) => confirm::show_rollback(ctx, app),
        Some(ActiveDialog::PlanApproval) => confirm::show_plan_approval(ctx, app),
        Some(ActiveDialog::OutputView) => output_view::show(ctx, app),
        Some(ActiveDialog::Properties) => properties::show(ctx, app),
        Some(ActiveDialog::SessionMenu) => session_menu::show(ctx, app),
        Some(ActiveDialog::RowAction) => confirm::show_row_action(ctx, app),
        Some(ActiveDialog::MemoryDelete) => confirm::show_memory_delete(ctx, app),
        Some(ActiveDialog::MemoryClear) => confirm::show_memory_clear(ctx, app),
        Some(ActiveDialog::MemoryView) => confirm::show_memory_view(ctx, app),
        None => false,
    }
}

/// Let the active dialog handle this frame's input before the key router.
///
/// The model picker uses it so that, while its filter hides rows, Up and
/// Down move between the shown rows and Enter on an empty list does nothing
/// ([`model_picker::prepare`]); the provider and properties dialogs to paste
/// into their text fields ([`provider::prepare`],
/// [`properties::prepare`]). The run-cost banner, shown over any dialog,
/// applies its dismiss rule first ([`banner::prepare`]).
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose dialog state is read.
pub fn prepare(ctx: &egui::Context, app: &mut App) {
    banner::prepare(ctx, app);
    match active(app) {
        Some(ActiveDialog::ModelPicker) => model_picker::prepare(ctx, app),
        Some(ActiveDialog::Provider) => provider::prepare(ctx, app),
        Some(ActiveDialog::Properties) => properties::prepare(ctx, app),
        Some(ActiveDialog::LoopSetup) => loop_setup::prepare(ctx, app),
        Some(ActiveDialog::ContextMenu) => context_menu::prepare(ctx, app),
        Some(
            ActiveDialog::ConfigSavePicker
            | ActiveDialog::ThemePicker
            | ActiveDialog::HistoryPicker
            | ActiveDialog::RouterSave
            | ActiveDialog::PluginStore
            | ActiveDialog::ConnectorStore
            | ActiveDialog::BackendPanel
            | ActiveDialog::McpDiscover
            | ActiveDialog::PluginsList
            | ActiveDialog::OutputView
            | ActiveDialog::Shortcuts
            | ActiveDialog::Question
            | ActiveDialog::Permission
            | ActiveDialog::Notice
            | ActiveDialog::DbPrune
            | ActiveDialog::ForceCleanup
            | ActiveDialog::Rollback
            | ActiveDialog::PlanApproval
            | ActiveDialog::SessionMenu
            | ActiveDialog::RowAction
            | ActiveDialog::MemoryDelete
            | ActiveDialog::MemoryClear
            | ActiveDialog::MemoryView,
        )
        | None => {}
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
                ui.add_space(crate::metrics::space::XS);
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
