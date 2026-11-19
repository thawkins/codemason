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
use egui::{Id, RichText};

use super::{modal, plain, send_key};

/// Title and accessible label of the palette.
pub const PALETTE_TITLE: &str = PaletteScope::All.title();

/// Accessible label of the search field.
pub const SEARCH_LABEL: &str = "Search commands";

/// Accessible label of the list of rows.
pub const ROWS_LABEL: &str = "Commands";

/// The egui memory slot of the action keys the shell publishes.
fn keys_id() -> Id {
    Id::unique("codemason_palette_action_keys")
}

/// Widget id of the search field.
#[must_use]
pub fn search_field_id() -> Id {
    Id::unique("codemason_palette_search")
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
    modal(ctx, "codemason_command_palette", title, |ui| {
        ui.set_width((ctx.content_rect().width() * 0.6).max(320.0));
        let field = ui.add(
            egui::TextEdit::singleline(&mut query)
                .id(search_field_id())
                .hint_text("Type a command, theme, agent or session")
                .desired_width(f32::INFINITY),
        );
        field
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::SearchInput, true, SEARCH_LABEL));
        // The search field takes the typing while the palette is open.
        if !field.has_focus() {
            field.request_focus();
        }
        ui.add_space(crate::metrics::space::XS);
        let list = egui::ScrollArea::vertical()
            .max_height(ctx.content_rect().height() * 0.5)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for (index, row) in rows.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let response = ui.selectable_label(index == selected, &row.title);
                            if response.double_clicked() {
                                clicked = Some((index, true));
                            } else if response.clicked() {
                                clicked = Some((index, false));
                            }
                            if let Some(hint) = &row.hint {
                                ui.weak(hint);
                            }
                            let key = row
                                .id
                                .strip_prefix("action:")
                                .and_then(|id| keys.get(id).cloned())
                                .or_else(|| row.key.clone());
                            if let Some(key) = key {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(RichText::new(key).monospace().weak());
                                    },
                                );
                            }
                        });
                    }
                    if rows.is_empty() {
                        ui.weak("Nothing matches");
                    }
                })
                .response
            });
        list.inner
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, ROWS_LABEL));
        ui.weak("Up/Down move  Enter run  Esc close");
    });
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
