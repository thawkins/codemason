//! The list dialogs: `/mcp discover` (`App::mcp_discover`), `/plugins list`
//! (`App::plugins_list`) and the `/backend` switcher (`App::backend_panel`).
//!
//! app-core runs all three from the keyboard (`input::handle_key`):
//! - `/mcp discover` and `/plugins list`: Up/Down (or k/j) and Home/End move
//!   the highlight, Space toggles the highlighted server or plugin (the
//!   result is shown as a feedback line), Esc closes;
//! - `/backend`: Up/Down move the highlight, Enter switches to the
//!   highlighted backend, Esc closes.
//!
//! The keys reach the App through the router, as in the TUI. These dialogs
//! draw the rows as the TUI does; a click moves the highlight (the state
//! the arrows move), and the buttons send the key that does the same thing:
//! Toggle sends Space, Switch sends Enter, Close sends Esc.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_agent::mcp::McpDiscoverySource;
use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;

use super::pickers::entry_list;
use super::{key_buttons, modal, plain};

/// Title of the `/mcp discover` dialog, as the TUI's heading.
pub const MCP_DISCOVER_TITLE: &str = "MCP Server Discovery";

/// Title of the `/plugins list` dialog, as the TUI's border title.
pub const PLUGINS_LIST_TITLE: &str = "/plugins list";

/// Accessible label of the `/mcp discover` server list.
pub const SERVERS_LABEL: &str = "Servers";

/// Accessible label of the `/plugins list` rows.
pub const PLUGINS_LABEL: &str = "Plugins";

/// Accessible label of the `/backend` rows.
pub const BACKENDS_LABEL: &str = "Backends";

/// The button that toggles the highlighted row (Space).
pub const TOGGLE: &str = "Toggle";

/// The button that switches to the highlighted backend (Enter).
pub const SWITCH: &str = "Switch";

/// The button that closes a list dialog (Esc).
pub const CLOSE: &str = "Close";

/// The `/backend` title, naming the active backend's kind, as the TUI's.
#[must_use]
pub fn backend_title(active_kind: &str) -> String {
    format!("Execution backend - active: {active_kind}")
}

/// The TUI's source label of a discovered server.
fn source_label(source: &McpDiscoverySource) -> &'static str {
    match source {
        McpDiscoverySource::SystemPath => "PATH",
        McpDiscoverySource::NpmGlobal { .. } => "npm global",
        McpDiscoverySource::McpRegistry { .. } => "MCP registry",
        McpDiscoverySource::Plugin { .. } => "plugin",
        McpDiscoverySource::Connector { .. } => "connector",
    }
}

/// Draw the `/mcp discover` dialog while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `mcp_discover` is drawn.
///
/// # Returns
///
/// `true` while the dialog is open.
pub fn show_mcp_discover(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(state) = app.mcp_discover.as_ref() else {
        return false;
    };
    let rows: Vec<String> = state
        .servers
        .iter()
        .enumerate()
        .map(|(i, srv)| {
            let marker = if state.enabled.get(i).copied().unwrap_or(false) {
                "[x]"
            } else {
                "[ ]"
            };
            let source = match &srv.definition_file {
                Some(path) => format!("{} [{}]", source_label(&srv.source), path.display()),
                None => source_label(&srv.source).to_string(),
            };
            format!("{marker} {}  {}  {source}", srv.id, srv.name)
        })
        .collect();
    let selected = state.selected;
    let feedback = state.feedback.clone();
    let mut clicked = None;
    modal(ctx, "codemason_mcp_discover", MCP_DISCOVER_TITLE, |ui| {
        if rows.is_empty() {
            ui.label("No MCP servers detected.");
            ui.weak(
                "Install MCP servers via npm (e.g. @modelcontextprotocol/server-filesystem) \
                 or place configs in ~/.config/codemason/servers/ and retry.",
            );
        } else {
            clicked = entry_list(ui, SERVERS_LABEL, &rows, selected);
        }
        if let Some(feedback) = &feedback {
            ui.label(feedback);
        }
        ui.add_space(crate::metrics::space::SM);
        ui.add_enabled_ui(!rows.is_empty(), |ui| {
            key_buttons(ui, &[(TOGGLE, plain(Key::Char(' ')))]);
        });
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Up/Down navigate * Space enable/disable * Esc close");
    });
    if let Some((i, _)) = clicked
        && let Some(state) = app.mcp_discover.as_mut()
    {
        state.selected = i;
    }
    true
}

/// Draw the `/plugins list` dialog while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `plugins_list` is drawn.
///
/// # Returns
///
/// `true` while the dialog is open.
pub fn show_plugins_list(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(state) = app.plugins_list.as_ref() else {
        return false;
    };
    let view = &state.view;
    let rows = view.body.clone();
    let header = view.header.clone();
    let heading = state.heading.clone();
    let attribution = view.attribution.clone();
    let selected = state.selected;
    let feedback = state.feedback.clone();
    let mut clicked = None;
    modal(ctx, "codemason_plugins_list", PLUGINS_LIST_TITLE, |ui| {
        if let Some(heading) = &heading {
            ui.strong(heading);
        }
        if !attribution.is_empty() {
            ui.weak(&attribution);
        }
        if let Some(header) = &header {
            ui.monospace(header);
        }
        clicked = entry_list(ui, PLUGINS_LABEL, &rows, selected);
        if let Some(feedback) = &feedback {
            ui.label(feedback);
        }
        ui.add_space(crate::metrics::space::SM);
        ui.add_enabled_ui(!rows.is_empty(), |ui| {
            key_buttons(ui, &[(TOGGLE, plain(Key::Char(' ')))]);
        });
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Up/Down navigate * Space enable/disable * Esc close");
    });
    if let Some((i, _)) = clicked
        && let Some(state) = app.plugins_list.as_mut()
    {
        state.selected = i;
    }
    true
}

/// Draw the `/backend` switcher while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `backend_panel` is drawn.
///
/// # Returns
///
/// `true` while the panel is open.
pub fn show_backend_panel(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(state) = app.backend_panel.as_ref() else {
        return false;
    };
    let active_kind = state
        .rows
        .iter()
        .find(|row| row.active)
        .map_or("none", |row| row.kind);
    let rows: Vec<String> = state
        .rows
        .iter()
        .map(|row| {
            let marker = if row.active { " [active]" } else { "" };
            let detail = row
                .detail
                .as_deref()
                .map(|d| format!(" - {d}"))
                .unwrap_or_default();
            format!(
                "{}  {}  {}  {}{marker}{detail}",
                row.id, row.kind, row.health, row.connection
            )
        })
        .collect();
    let selectable = state
        .rows
        .get(state.cursor)
        .is_some_and(|row| row.is_selectable());
    let cursor = state.cursor;
    let title = backend_title(active_kind);
    let mut clicked = None;
    modal(ctx, "codemason_backend_panel", &title, |ui| {
        clicked = entry_list(ui, BACKENDS_LABEL, &rows, cursor);
        ui.add_space(crate::metrics::space::SM);
        ui.add_enabled_ui(selectable, |ui| {
            key_buttons(ui, &[(SWITCH, plain(Key::Enter))]);
        });
        key_buttons(ui, &[(CLOSE, plain(Key::Esc))]);
        ui.weak("Up/Down move * Enter switch * Esc close");
    });
    if let Some((i, _)) = clicked
        && let Some(state) = app.backend_panel.as_mut()
    {
        state.cursor = i;
    }
    true
}
