//! The command palette (`App::palette`).
//!
//! app-core owns the palette like any dialog: its entries, their ranking
//! against the query, and its keys (Up/Down move the highlight, Enter runs
//! the highlighted row, Esc closes; see `codemason_app_core::palette`). This
//! dialog draws it: a search field that edits the App's query in place and
//! takes the keyboard while the palette is open, then the ranked rows with
//! the highlight, each row's dimmer hint and, for an action, its key. A
//! click moves the highlight; a double click runs the row.
//!
//! An action's key is the binding the window's keymap gives it now (a user
//! override included), which the shell publishes with [`set_action_keys`];
//! without it, the action's default key.
//!
//! The agent and session pickers are the palette scoped to one kind of
//! entry (`PaletteScope`); the dialog's title follows the scope.
//!
//! Dependencies: `codemason-app-core` and `egui`.

use std::collections::BTreeMap;
use std::sync::Arc;

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use codemason_app_core::palette::PaletteScope;
use egui::Id;

use super::{material_hint, material_modal, plain, send_key};
use crate::material::list::list_item;
use crate::material::text_field::{field_id, outlined_text_field};
use crate::palette::Palette;

/// Title and accessible label of the palette.
pub const PALETTE_TITLE: &str = PaletteScope::All.title();

/// Accessible label of the search field.
pub const SEARCH_LABEL: &str = "Search commands";

/// Accessible label of the list of rows.
pub const ROWS_LABEL: &str = "Commands";

/// The search field's supporting text: what it searches.
pub const SEARCH_HINT: &str = "Type a command, theme, agent or session";

/// The message a palette with no matching row shows.
pub const NOTHING_MATCHES: &str = "Nothing matches";

/// The keys the palette answers to.
const HINT: &str = "Up/Down move  Enter run  Esc close";

/// The egui memory slot of the action keys the shell publishes.
fn keys_id() -> Id {
    Id::unique("codemason_palette_action_keys")
}

/// Widget id of the search field.
///
/// # Returns
///
/// The id of the Material field's editor, whose state holds the query's
/// selection.
#[must_use]
pub fn search_field_id() -> Id {
    field_id(SEARCH_LABEL)
}

/// Draw the search field: the Material outlined text field, labelled
/// [`SEARCH_LABEL`] and reporting `Role::SearchInput` so assistive
/// technology announces it as a search box.
///
/// # Arguments
///
/// * `ui` - the dialog's ui.
/// * `palette` - the colours.
/// * `text` - the query being edited.
///
/// # Returns
///
/// The editor's response.
fn search_field(ui: &mut egui::Ui, palette: &Palette, text: &mut String) -> egui::Response {
    let field = outlined_text_field(ui, palette, SEARCH_LABEL, text, Some(SEARCH_HINT), false);
    // A Material field is a text input; the palette's is a search box.
    ui.ctx().accesskit_node_builder(field.id, |node| {
        node.set_role(egui::accesskit::Role::SearchInput);
        node.set_label(SEARCH_LABEL);
    });
    field
}

/// Publish the key each action is bound to now, by action id, for the rows'
/// key hints.
pub fn set_action_keys(ctx: &egui::Context, keys: BTreeMap<String, String>) {
    ctx.data_mut(|d| d.insert_temp(keys_id(), Arc::new(keys)));
}

/// Hold back this frame's Up, Down and Enter presses when the frame also
/// types into the search field, and send them after it is drawn: the router
/// runs before the field applies the text, so Enter would otherwise run a row
/// of the old query.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
pub fn prepare(ctx: &egui::Context) {
    let held = ctx.input_mut(|input| {
        let types = input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Text(_) | egui::Event::Paste(_)));
        if !types {
            return Vec::new();
        }
        let mut held = Vec::new();
        input.events.retain(|event| {
            let egui::Event::Key { key, pressed, .. } = event else {
                return true;
            };
            let app_key = match key {
                egui::Key::Enter => Key::Enter,
                egui::Key::ArrowUp => Key::Up,
                egui::Key::ArrowDown => Key::Down,
                _ => return true,
            };
            if *pressed {
                held.push(app_key);
            }
            false
        });
        held
    });
    for key in held {
        send_key(ctx, plain(key));
    }
}

/// Draw the palette while the App shows it.
///
/// # Returns
///
/// `true` while the palette is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(palette) = app.palette.as_ref() else {
        return false;
    };
    let selected = palette.selected;
    let title = palette.scope.title();
    let mut query = palette.query.clone();
    let keys: Arc<BTreeMap<String, String>> =
        ctx.data(|d| d.get_temp(keys_id())).unwrap_or_default();
    let rows = app.palette_results();
    let mut clicked: Option<(usize, bool)> = None;
    let palette_colours = crate::palette::palette_for(app, ctx);
    let width = (ctx.content_rect().width() * 0.6).max(320.0);
    material_modal(
        ctx,
        &palette_colours,
        "codemason_command_palette",
        title,
        width,
        |ui| {
            let field = search_field(ui, &palette_colours, &mut query);
            // The search field takes the typing while the palette is open.
            if !field.has_focus() {
                field.request_focus();
            }
            ui.add_space(crate::metrics::space::SM);
            let list = egui::ScrollArea::vertical()
                .max_height(ctx.content_rect().height() * 0.5)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (index, row) in rows.iter().enumerate() {
                            let key = row
                                .id
                                .strip_prefix("action:")
                                .and_then(|id| keys.get(id).cloned())
                                .or_else(|| row.key.clone());
                            let item = list_item(
                                ui,
                                &palette_colours,
                                &row.title,
                                row.hint.as_deref(),
                                key.as_deref(),
                                index == selected,
                            );
                            if item.double_clicked() {
                                clicked = Some((index, true));
                            } else if item.clicked() {
                                clicked = Some((index, false));
                            }
                        }
                        if rows.is_empty() {
                            material_hint(ui, &palette_colours, NOTHING_MATCHES);
                        }
                    })
                    .response
                });
            list.inner
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, ROWS_LABEL));
            ui.add_space(crate::metrics::space::MD);
            material_hint(ui, &palette_colours, HINT);
        },
    );
    app.set_palette_query(&query);
    if let Some((index, run)) = clicked {
        if let Some(palette) = app.palette.as_mut() {
            palette.selected = index;
        }
        if run {
            send_key(ctx, plain(Key::Enter));
        }
    }
    true
}
