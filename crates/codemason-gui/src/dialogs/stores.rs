//! The plugin-store browser (`App::plugin_store`, `/plugins codex|claude`)
//! and the connector-catalogue browser (`App::connector_store`,
//! `/connectors claude`).
//!
//! app-core runs both panels from the keyboard (`input::handle_key`):
//! Tab/Shift+Tab cycle focus between the result list, the category
//! navigator and the marketplace navigator; Up/Down move the focused pane
//! (a navigator applies its filter at once); Enter over a navigator applies
//! it and focuses the results, Enter over the results installs the
//! highlighted entry; `c` clears the focused navigator; typed characters
//! edit the search query, Backspace removes one and Esc on an empty query
//! closes. The keys reach the App through the router, as in the TUI.
//!
//! This dialog draws one snapshot of either browser:
//! the store, the counts and the query, the three panes with the focused
//! one marked, the load status and the last install result. Clicks do what
//! the TUI's mouse handler does: a navigator row selects that filter and
//! focuses the navigator, a result row moves the result highlight. A double
//! click on a result, or Install, focuses the results and sends Enter.
//! Clear search clears the query and Close closes the panel, through the
//! App's own actions.
//!
//! Dependencies: `codemason-app-core`, `codemason-plugins` (`StoreEntry`),
//! `codemason-connectors` (`CatalogueBrowser`) and `egui`.

use codemason_app_core::app::{
    App, ConnectorBrowseStatus, PluginStoreBrowser, PluginStoreStatus, StorePane,
};
use codemason_app_core::input_event::Key;
use codemason_connectors::{CatalogueBrowser, CataloguePane};

use super::{modal, plain, send_key};

/// Accessible label of the category navigator.
pub const CATEGORIES_LABEL: &str = "Categories";

/// Accessible label of the marketplace navigator.
pub const MARKETPLACES_LABEL: &str = "Marketplaces";

/// Accessible label of the result list.
pub const RESULTS_LABEL: &str = "Results";

/// The button that installs the highlighted result (Enter on the results).
pub const INSTALL: &str = "Install";

/// The button that clears the search query.
pub const CLEAR_SEARCH: &str = "Clear search";

/// The button that closes the panel.
pub const CLOSE: &str = "Close";

/// The plugin store's title, e.g. `Codex Plugin Store`, as the TUI's.
#[must_use]
pub fn plugin_store_title(kind_label: &str) -> String {
    format!("{kind_label} Plugin Store")
}

/// The connector catalogue's title, as the TUI's.
pub const CONNECTORS_TITLE: &str = "Claude Connectors";

/// Which pane holds the keyboard.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Results,
    Categories,
    Marketplaces,
}

/// What either browser shows, read once per frame.
struct StoreSnapshot {
    title: String,
    shown: usize,
    total: usize,
    query: String,
    status: Option<String>,
    focus: Pane,
    categories: Vec<String>,
    category_cursor: usize,
    marketplaces: Vec<String>,
    marketplace_cursor: usize,
    results: Vec<String>,
    result_cursor: usize,
    footer: Option<String>,
}

impl StoreSnapshot {
    /// Snapshot a plugin-store browser.
    fn plugin(browser: &PluginStoreBrowser) -> Self {
        let results = browser
            .filtered
            .iter()
            .filter_map(|&i| browser.all.get(i))
            .map(|entry| {
                let installed = if browser.installed.contains(&entry.id) {
                    " [installed]"
                } else {
                    ""
                };
                format!(
                    "{} {}{installed} - {}",
                    entry.name, entry.version, entry.description
                )
            })
            .collect();
        Self {
            title: plugin_store_title(browser.kind.label()),
            shown: browser.filtered.len(),
            total: browser.all.len(),
            query: browser.query.clone(),
            status: match &browser.status {
                PluginStoreStatus::Loading => Some("Loading the store index...".to_string()),
                PluginStoreStatus::Ready => None,
                PluginStoreStatus::Empty => Some("The store has no entries".to_string()),
                PluginStoreStatus::Failed(detail) => Some(format!("[err] {detail}")),
            },
            focus: match browser.focus {
                StorePane::Results => Pane::Results,
                StorePane::Categories => Pane::Categories,
                StorePane::Marketplaces => Pane::Marketplaces,
            },
            categories: (0..browser.category_row_count())
                .map(|row| browser.category_row_display(row))
                .collect(),
            category_cursor: browser.category_cursor_row(),
            marketplaces: (0..browser.marketplace_row_count())
                .map(|row| browser.marketplace_row_display(row))
                .collect(),
            marketplace_cursor: browser.marketplace_cursor_row(),
            results,
            result_cursor: browser.cursor,
            footer: footer(browser.last_install.as_deref(), browser.skipped),
        }
    }

    /// Snapshot a connector-catalogue browser.
    fn connector(browser: &CatalogueBrowser) -> Self {
        let results = browser
            .filtered
            .iter()
            .filter_map(|&i| browser.all.get(i))
            .map(|entry| {
                let installed = if browser.is_installed(entry.id.as_str()) {
                    " [installed]"
                } else {
                    ""
                };
                format!("{}{installed} - {}", entry.name, entry.description)
            })
            .collect();
        Self {
            title: CONNECTORS_TITLE.to_string(),
            shown: browser.filtered.len(),
            total: browser.all.len(),
            query: browser.query.clone(),
            status: match &browser.status {
                ConnectorBrowseStatus::Loading => Some("Loading the catalogue...".to_string()),
                ConnectorBrowseStatus::Ready => None,
                ConnectorBrowseStatus::Empty => Some("The catalogue has no entries".to_string()),
                ConnectorBrowseStatus::Failed(detail) => Some(format!("[err] {detail}")),
            },
            focus: match browser.focus {
                CataloguePane::Results => Pane::Results,
                CataloguePane::Categories => Pane::Categories,
                CataloguePane::Marketplaces => Pane::Marketplaces,
            },
            categories: (0..browser.category_row_count())
                .map(|row| browser.category_row_display(row))
                .collect(),
            category_cursor: browser.category_cursor_row(),
            marketplaces: (0..browser.marketplace_row_count())
                .map(|row| browser.marketplace_row_display(row))
                .collect(),
            marketplace_cursor: browser.marketplace_cursor_row(),
            results,
            result_cursor: browser.cursor,
            footer: footer(browser.last_install.as_deref(), browser.skipped),
        }
    }
}

/// The footer line: the last install result and any skipped entries.
fn footer(last_install: Option<&str>, skipped: usize) -> Option<String> {
    let skipped = (skipped > 0).then(|| format!("{skipped} malformed entries skipped"));
    match (last_install, skipped) {
        (Some(install), Some(skipped)) => Some(format!("{install} * {skipped}")),
        (Some(install), None) => Some(install.to_string()),
        (None, skipped) => skipped,
    }
}

/// What a click in the panel asks for.
enum Click {
    Category(usize),
    Marketplace(usize),
    Result { index: usize, install: bool },
    Install,
    ClearSearch,
    Close,
}

/// Draw one pane as a labelled list; returns a clicked row and whether it
/// was double-clicked.
fn pane(
    ui: &mut egui::Ui,
    label: &str,
    focused: bool,
    rows: &[String],
    cursor: usize,
) -> Option<(usize, bool)> {
    let mut clicked = None;
    let marker = if focused { "> " } else { "  " };
    ui.strong(format!("{marker}{label}"));
    let list = egui::ScrollArea::vertical()
        .id_salt(label)
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                for (i, row) in rows.iter().enumerate() {
                    let response = ui.selectable_label(i == cursor, row.as_str());
                    if response.double_clicked() {
                        clicked = Some((i, true));
                    } else if response.clicked() {
                        clicked = Some((i, false));
                    }
                }
                if rows.is_empty() {
                    ui.weak("(none)");
                }
            })
            .response
        });
    list.inner
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, label));
    clicked
}

/// Draw a snapshot; returns what was clicked.
fn draw(ctx: &egui::Context, id: &str, snap: &StoreSnapshot) -> Option<Click> {
    let mut click = None;
    modal(ctx, id, &snap.title, |ui| {
        let query = if snap.query.is_empty() {
            "-"
        } else {
            snap.query.as_str()
        };
        ui.label(format!(
            "{} of {} * search: {query}",
            snap.shown, snap.total
        ));
        if let Some(status) = &snap.status {
            ui.colored_label(ui.visuals().warn_fg_color, status);
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width(220.0);
                if let Some((row, _)) = pane(
                    ui,
                    CATEGORIES_LABEL,
                    snap.focus == Pane::Categories,
                    &snap.categories,
                    snap.category_cursor,
                ) {
                    click = Some(Click::Category(row));
                }
                if let Some((row, _)) = pane(
                    ui,
                    MARKETPLACES_LABEL,
                    snap.focus == Pane::Marketplaces,
                    &snap.marketplaces,
                    snap.marketplace_cursor,
                ) {
                    click = Some(Click::Marketplace(row));
                }
            });
            ui.vertical(|ui| {
                ui.set_min_width(360.0);
                if let Some((index, install)) = pane(
                    ui,
                    RESULTS_LABEL,
                    snap.focus == Pane::Results,
                    &snap.results,
                    snap.result_cursor,
                ) {
                    click = Some(Click::Result { index, install });
                }
            });
        });
        if let Some(footer) = &snap.footer {
            ui.weak(footer);
        }
        ui.add_space(crate::metrics::space::SM);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!snap.results.is_empty(), egui::Button::new(INSTALL))
                .clicked()
            {
                click = Some(Click::Install);
            }
            if ui.button(CLEAR_SEARCH).clicked() {
                click = Some(Click::ClearSearch);
            }
            if ui.button(CLOSE).clicked() {
                click = Some(Click::Close);
            }
        });
        ui.weak(
            "Tab pane * Up/Down move * Enter apply/install * c clear filter * \
             type to search * Esc close",
        );
    });
    click
}

/// Draw the plugin-store browser while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `plugin_store` is drawn.
///
/// # Returns
///
/// `true` while the panel is open.
pub fn show_plugin_store(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(browser) = app.plugin_store.as_ref() else {
        return false;
    };
    let snap = StoreSnapshot::plugin(browser);
    let Some(click) = draw(ctx, "codemason_plugin_store", &snap) else {
        return true;
    };
    match click {
        Click::ClearSearch => app.plugin_store_clear_search(),
        Click::Close => app.close_plugin_store(),
        Click::Category(row) => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.category_select_row(row);
                browser.focus = StorePane::Categories;
            }
        }
        Click::Marketplace(row) => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.marketplace_select_row(row);
                browser.focus = StorePane::Marketplaces;
            }
        }
        Click::Result { index, install } => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.cursor = index;
                if install {
                    browser.focus = StorePane::Results;
                    send_key(ctx, plain(Key::Enter));
                }
            }
        }
        Click::Install => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.focus = StorePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
    }
    app.needs_redraw = true;
    true
}

/// Draw the connector-catalogue browser while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `connector_store` is drawn.
///
/// # Returns
///
/// `true` while the panel is open.
pub fn show_connector_store(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(browser) = app.connector_store.as_ref() else {
        return false;
    };
    let snap = StoreSnapshot::connector(browser);
    let Some(click) = draw(ctx, "codemason_connector_store", &snap) else {
        return true;
    };
    match click {
        Click::ClearSearch => app.connector_catalogue_clear_search(),
        Click::Close => app.close_connector_catalogue(),
        Click::Category(row) => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.category_select_row(row);
                browser.focus = CataloguePane::Categories;
            }
        }
        Click::Marketplace(row) => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.marketplace_select_row(row);
                browser.focus = CataloguePane::Marketplaces;
            }
        }
        Click::Result { index, install } => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.cursor = index;
                if install {
                    browser.focus = CataloguePane::Results;
                    send_key(ctx, plain(Key::Enter));
                }
            }
        }
        Click::Install => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.focus = CataloguePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
    }
    app.needs_redraw = true;
    true
}
