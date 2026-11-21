//! The plugin-store browser (`App::plugin_store`, `/plugins codex|claude`)
//! and the connector-catalogue browser (`App::connector_store`,
//! `/connectors claude`), drawn as one wide three-pane Material dialog.
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
//! Each frame reads one [`snapshot::StoreSnapshot`] of the open browser and
//! [`frame`] draws it: the header (title, the Codex | Claude switcher for
//! plugins, the search field, close), the filter rail, the result list,
//! the details pane and the footer. [`plugin`] draws the plugin-specific
//! row and detail parts. Clicks do what the TUI's mouse handler does: a
//! filter row selects that filter and focuses its pane, a result row moves
//! the highlight; a double click on a result, a row's Install or the
//! details' Install moves the highlight there, focuses the results and
//! sends Enter. Clear search, Close, Refresh, Retry and the switcher call
//! the App's own actions.
//!
//! Dependencies: `codemason-app-core`, `codemason-connectors`,
//! `codemason-plugins` (`StoreKind`) and `egui`.

pub mod frame;
pub mod plugin;
pub mod snapshot;

use codemason_app_core::app::{App, StorePane};
use codemason_app_core::input_event::Key;
use codemason_connectors::CataloguePane;
use codemason_plugins::StoreKind;

use super::{plain, send_key};
use snapshot::StoreSnapshot;

/// Accessible label of the category navigator.
pub const CATEGORIES_LABEL: &str = "Categories";

/// Accessible label of the marketplace navigator.
pub const MARKETPLACES_LABEL: &str = "Marketplaces";

/// Accessible label of the result list.
pub const RESULTS_LABEL: &str = "Results";

/// Accessible label of the details pane.
pub const DETAILS_LABEL: &str = "Details";

/// The button that installs a result (Enter on the results).
pub const INSTALL: &str = "Install";

/// The disabled button an installed result shows instead of Install.
pub const INSTALLED: &str = "Installed";

/// The button that clears the search query.
pub const CLEAR_SEARCH: &str = "Clear search";

/// The button that closes the panel.
pub const CLOSE: &str = "Close";

/// The footer button that re-fetches the index.
pub const REFRESH: &str = "Refresh";

/// The button a failed fetch shows to fetch again.
pub const RETRY: &str = "Retry";

/// The plugin store's title.
pub const PLUGIN_STORE_TITLE: &str = "Plugin Store";

/// The connector catalogue's title.
pub const CONNECTORS_TITLE: &str = "Connectors";

/// The plugin store's kind switcher, in [`StoreKind`] order.
pub const KIND_OPTIONS: [&str; 2] = ["Codex", "Claude"];

/// The search field's tooltip.
pub const SEARCH_TOOLTIP: &str = "Type to search - Esc clears, then closes";

/// The panes' tooltip.
pub const PANES_TOOLTIP: &str = "Tab switches pane - Up/Down move - Enter installs";

/// What a click in the dialog asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// A category row: select it and focus the category pane.
    Category(usize),
    /// A marketplace row: select it and focus the marketplace pane.
    Marketplace(usize),
    /// A result row: move the highlight there; `install` on a double click.
    Result {
        /// The row's index in the shown results.
        index: usize,
        /// Whether to install it too.
        install: bool,
    },
    /// A row's own Install: move the highlight there and install it.
    RowInstall(usize),
    /// The details pane's Install: install the highlighted result.
    Install,
    /// Clear the search query.
    ClearSearch,
    /// Close the dialog.
    Close,
    /// Re-fetch the index (the footer's Refresh).
    Refresh,
    /// Fetch again after a failure.
    Retry,
    /// Show the other plugin store.
    SwitchKind(StoreKind),
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
    let palette = crate::palette::palette_for(app, ctx);
    let Some(click) = frame::draw(
        ctx,
        &palette,
        "codemason_plugin_store",
        &snap,
        &plugin::LOOK,
    ) else {
        return true;
    };
    apply_plugin(ctx, app, click);
    true
}

/// Do what `click` asks of the plugin store.
fn apply_plugin(ctx: &egui::Context, app: &mut App, click: Click) {
    match click {
        Click::ClearSearch => app.plugin_store_clear_search(),
        Click::Close => app.close_plugin_store(),
        Click::Refresh | Click::Retry => app.refresh_open_store(),
        Click::SwitchKind(kind) => app.switch_plugin_store_kind(kind),
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
        Click::RowInstall(index) => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.cursor = index;
                browser.focus = StorePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
        Click::Install => {
            if let Some(browser) = app.plugin_store.as_mut() {
                browser.focus = StorePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
    }
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
    let palette = crate::palette::palette_for(app, ctx);
    let Some(click) = frame::draw(
        ctx,
        &palette,
        "codemason_connector_store",
        &snap,
        &CONNECTOR_LOOK,
    ) else {
        return true;
    };
    apply_connector(ctx, app, click);
    true
}

/// The connector catalogue's copy and parts: a row shows its tags, the
/// details its category, marketplace and source.
const CONNECTOR_LOOK: frame::Look = frame::Look {
    search_placeholder: "Search connectors...",
    loading: "Loading the catalogue...",
    empty: "The catalogue has no entries",
    row_extras: plugin::row_tags,
    details: connector_details,
};

/// Draw a connector's detail fields.
fn connector_details(
    ui: &mut egui::Ui,
    palette: &crate::palette::Palette,
    row: &snapshot::RowSnapshot,
) {
    let detail = match &row.detail {
        snapshot::Detail::Connector(detail) => detail,
        snapshot::Detail::Plugin(_) => return,
    };
    frame::detail_field(ui, palette, "Category", &detail.category);
    frame::detail_field(ui, palette, "Marketplace", &detail.marketplace);
    frame::detail_field(ui, palette, "Source", &detail.source);
}

/// Do what `click` asks of the connector catalogue.
fn apply_connector(ctx: &egui::Context, app: &mut App, click: Click) {
    match click {
        Click::ClearSearch => app.connector_catalogue_clear_search(),
        Click::Close => app.close_connector_catalogue(),
        Click::Refresh | Click::Retry => app.refresh_open_store(),
        // The catalogue has one kind; it shows no switcher.
        Click::SwitchKind(_) => {}
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
        Click::RowInstall(index) => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.cursor = index;
                browser.focus = CataloguePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
        Click::Install => {
            if let Some(browser) = app.connector_store.as_mut() {
                browser.focus = CataloguePane::Results;
                send_key(ctx, plain(Key::Enter));
            }
        }
    }
}
