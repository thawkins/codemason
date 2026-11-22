//! What a store dialog shows, read once per frame from either browser.
//!
//! [`StoreSnapshot::plugin`] reads a `PluginStoreBrowser` and
//! [`StoreSnapshot::connector`] a `CatalogueBrowser` into one structured
//! shape: the filter rails as `(label, count)` rows, the shown results as
//! [`RowSnapshot`]s with a plugin- or connector-specific [`Detail`], the
//! list state, the focused pane and the footer's install result. The frame
//! draws the shared parts and each dialog its own rows and details.
//!
//! Dependencies: `codemason-app-core` (the plugin browser),
//! `codemason-connectors` (the catalogue browser) and `codemason-plugins`
//! (`StoreKind`).

use codemason_app_core::app::{
    ConnectorBrowseStatus, PluginStoreBrowser, PluginStoreStatus, StorePane,
};
use codemason_connectors::{CatalogueBrowser, CataloguePane, ConnectorAuthShape};
use codemason_plugins::StoreKind;

use super::{CONNECTORS_TITLE, PLUGIN_STORE_TITLE, REFRESH_FAILED, REFRESHING};

/// One filter-rail row: its label and how many entries it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterRow {
    /// The category or marketplace name (`ALL` for the sentinel row).
    pub label: String,
    /// How many loaded entries selecting the row would show.
    pub count: usize,
}

/// One shown result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSnapshot {
    /// The entry's display name.
    pub name: String,
    /// The muted text after the name: `version - marketplace` for a plugin,
    /// the category for a connector.
    pub subtitle: String,
    /// The entry's description.
    pub description: String,
    /// The entry's tags.
    pub tags: Vec<String>,
    /// Whether the entry is installed already.
    pub installed: bool,
    /// The dialog-specific detail fields.
    pub detail: Detail,
}

/// The fields only one kind of entry has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detail {
    /// A plugin-store entry.
    Plugin(PluginDetail),
    /// A connector-catalogue entry.
    Connector(ConnectorDetail),
}

/// A plugin's detail fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDetail {
    /// The version, possibly blank.
    pub version: String,
    /// The marketplace the entry came from, possibly blank.
    pub marketplace: String,
    /// Where the plugin installs from.
    pub source: String,
    /// The plugin's homepage, if it declares one.
    pub homepage: Option<String>,
}

/// A connector's detail fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorDetail {
    /// The connector's category.
    pub category: String,
    /// The marketplace the entry came from, possibly blank.
    pub marketplace: String,
    /// How the connector signs in.
    pub auth: ConnectorAuthShape,
    /// The OAuth scopes it requests.
    pub scopes: Vec<String>,
    /// The credential's name (never its value), if it declares one.
    pub credential: Option<String>,
    /// The MCP servers it runs.
    pub servers: Vec<ServerSnapshot>,
    /// Where the descriptor came from.
    pub source: String,
    /// The pattern the user's own server address must match, when the
    /// connector is tenant-hosted and has no fixed address.
    pub server_url_pattern: Option<String>,
}

/// One MCP server of a connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSnapshot {
    /// The server's id.
    pub id: String,
    /// Its transport (`stdio`, `http`, ...).
    pub transport: String,
    /// The URL, or the command and its arguments joined by spaces.
    pub target: String,
}

/// What takes the result list's place, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListState {
    /// The index is being fetched.
    Loading,
    /// The index loaded with no entries.
    Empty,
    /// The fetch failed, with its cause.
    Failed(String),
    /// The index is shown (the filters may still match nothing).
    Ready,
}

/// Which pane holds the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The result list.
    Results,
    /// The category rail.
    Categories,
    /// The marketplace rail.
    Marketplaces,
}

/// Everything a store dialog draws in one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSnapshot {
    /// The dialog's title.
    pub title: &'static str,
    /// The plugin store's kind, `None` for the connector catalogue.
    pub plugin_kind: Option<StoreKind>,
    /// The search query.
    pub query: String,
    /// How many entries the filters show.
    pub shown: usize,
    /// How many entries the index holds.
    pub total: usize,
    /// The list state.
    pub state: ListState,
    /// The focused pane.
    pub focus: Pane,
    /// The category rail.
    pub categories: Vec<FilterRow>,
    /// The selected category row.
    pub category_cursor: usize,
    /// The marketplace rail.
    pub marketplaces: Vec<FilterRow>,
    /// The selected marketplace row.
    pub marketplace_cursor: usize,
    /// The shown results.
    pub rows: Vec<RowSnapshot>,
    /// The highlighted result.
    pub cursor: usize,
    /// The refresh note, the last install result and the skipped-entry
    /// count, if any.
    pub footer: Option<String>,
}

impl StoreSnapshot {
    /// Snapshot a plugin-store browser.
    ///
    /// # Arguments
    ///
    /// * `browser` - the open plugin store.
    ///
    /// # Returns
    ///
    /// The snapshot; a row's subtitle is `"{version} - {marketplace}"`, the
    /// version left out when it is blank.
    #[must_use]
    pub fn plugin(browser: &PluginStoreBrowser) -> Self {
        let rows = browser
            .filtered
            .iter()
            .filter_map(|&i| browser.all.get(i))
            .map(|entry| RowSnapshot {
                name: entry.name.clone(),
                subtitle: plugin_subtitle(&entry.version, &entry.marketplace),
                description: entry.description.clone(),
                tags: entry.tags.clone(),
                installed: browser.installed.contains(&entry.id),
                detail: Detail::Plugin(PluginDetail {
                    version: entry.version.clone(),
                    marketplace: entry.marketplace.clone(),
                    source: entry.source.clone(),
                    homepage: entry.homepage.clone(),
                }),
            })
            .collect();
        Self {
            title: PLUGIN_STORE_TITLE,
            plugin_kind: Some(browser.kind),
            query: browser.query.clone(),
            shown: browser.filtered.len(),
            total: browser.all.len(),
            state: match &browser.status {
                PluginStoreStatus::Loading => ListState::Loading,
                PluginStoreStatus::Empty => ListState::Empty,
                PluginStoreStatus::Failed(cause) => ListState::Failed(cause.clone()),
                PluginStoreStatus::Ready => ListState::Ready,
            },
            focus: match browser.focus {
                StorePane::Results => Pane::Results,
                StorePane::Categories => Pane::Categories,
                StorePane::Marketplaces => Pane::Marketplaces,
            },
            categories: (0..browser.category_row_count())
                .map(|row| {
                    let label = browser.category_row_label(row);
                    FilterRow {
                        label: label.to_string(),
                        count: browser.category_entry_count(label),
                    }
                })
                .collect(),
            category_cursor: browser.category_cursor_row(),
            marketplaces: (0..browser.marketplace_row_count())
                .map(|row| {
                    let label = browser.marketplace_row_label(row);
                    FilterRow {
                        label: label.to_string(),
                        count: browser.marketplace_entry_count(label),
                    }
                })
                .collect(),
            marketplace_cursor: browser.marketplace_cursor_row(),
            rows,
            cursor: browser.cursor,
            footer: footer(
                refresh_note(browser.refreshing, browser.refresh_error.as_deref()),
                browser.last_install.as_deref(),
                browser.skipped,
            ),
        }
    }

    /// Snapshot a connector-catalogue browser.
    ///
    /// # Arguments
    ///
    /// * `browser` - the open connector catalogue.
    ///
    /// # Returns
    ///
    /// The snapshot; a row's subtitle is the connector's category.
    #[must_use]
    pub fn connector(browser: &CatalogueBrowser) -> Self {
        let rows = browser
            .filtered
            .iter()
            .filter_map(|&i| browser.all.get(i))
            .map(|entry| RowSnapshot {
                name: entry.name.clone(),
                subtitle: entry.category.clone(),
                description: entry.description.clone(),
                tags: entry.tags.clone(),
                installed: browser.is_installed(entry.id.as_str()),
                detail: Detail::Connector(ConnectorDetail {
                    server_url_pattern: entry.server_url_pattern().map(str::to_string),
                    category: entry.category.clone(),
                    marketplace: entry.marketplace.clone(),
                    auth: entry.auth,
                    scopes: entry.auth_scope.clone(),
                    credential: entry.credential.clone(),
                    servers: entry
                        .servers
                        .iter()
                        .map(|server| ServerSnapshot {
                            id: server.id.clone(),
                            transport: server.transport.clone(),
                            target: server.url.clone().unwrap_or_else(|| {
                                server
                                    .command
                                    .iter()
                                    .chain(server.args.iter())
                                    .map(String::as_str)
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            }),
                        })
                        .collect(),
                    source: entry.source.clone(),
                }),
            })
            .collect();
        Self {
            title: CONNECTORS_TITLE,
            plugin_kind: None,
            query: browser.query.clone(),
            shown: browser.filtered.len(),
            total: browser.all.len(),
            state: match &browser.status {
                ConnectorBrowseStatus::Loading => ListState::Loading,
                ConnectorBrowseStatus::Empty => ListState::Empty,
                ConnectorBrowseStatus::Failed(cause) => ListState::Failed(cause.clone()),
                ConnectorBrowseStatus::Ready => ListState::Ready,
            },
            focus: match browser.focus {
                CataloguePane::Results => Pane::Results,
                CataloguePane::Categories => Pane::Categories,
                CataloguePane::Marketplaces => Pane::Marketplaces,
            },
            categories: (0..browser.category_row_count())
                .map(|row| {
                    let label = browser.category_row_label(row);
                    FilterRow {
                        label: label.to_string(),
                        count: browser.category_entry_count(label),
                    }
                })
                .collect(),
            category_cursor: browser.category_cursor_row(),
            marketplaces: (0..browser.marketplace_row_count())
                .map(|row| {
                    let label = browser.marketplace_row_label(row);
                    FilterRow {
                        label: label.to_string(),
                        count: browser.marketplace_entry_count(label),
                    }
                })
                .collect(),
            marketplace_cursor: browser.marketplace_cursor_row(),
            rows,
            cursor: browser.cursor,
            footer: footer(
                refresh_note(browser.refreshing, browser.refresh_error.as_deref()),
                browser.last_install.as_deref(),
                browser.skipped,
            ),
        }
    }
}

/// A plugin row's subtitle: `"{version} - {marketplace}"`, either part left
/// out when it is blank.
fn plugin_subtitle(version: &str, marketplace: &str) -> String {
    match (version.trim().is_empty(), marketplace.trim().is_empty()) {
        (false, false) => format!("{version} - {marketplace}"),
        (false, true) => version.to_string(),
        (true, _) => marketplace.to_string(),
    }
}

/// The footer note a forced re-fetch shows: [`REFRESHING`] while it runs,
/// otherwise the cause of a re-fetch that failed over the shown index.
fn refresh_note(refreshing: bool, refresh_error: Option<&str>) -> Option<String> {
    if refreshing {
        Some(REFRESHING.to_string())
    } else {
        refresh_error.map(|cause| format!("{REFRESH_FAILED}: {cause}"))
    }
}

/// The footer's right-hand text: the refresh note, the last install result and
/// any skipped entries, joined by `" - "`.
fn footer(note: Option<String>, last_install: Option<&str>, skipped: usize) -> Option<String> {
    let skipped = (skipped > 0).then(|| format!("{skipped} malformed entries skipped"));
    let parts: Vec<String> = [note, last_install.map(str::to_string), skipped]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| parts.join(" - "))
}
