//! Dockable pane layout for the GUI window.
//!
//! The window's panes ([`PaneId`]) are arranged by craft-ui's docking model,
//! [`craft_ui::docking::Layout`]. This module builds the default arrangement,
//! shows and hides a pane while remembering where it was, and persists the
//! layout as JSON at `<config_dir>/codemason/gui-layout.json`.
//!
//! Rendering each pane is left to the pane tasks; this module is only the
//! layout model.
//!
//! Dependencies: `craft-ui` (feature `serde`) for the layout model,
//! `serde`/`serde_json` for persistence, `codemason-config` for the config
//! directory, and `tracing` for load warnings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use craft_ui::docking::{Action, DockError, Layout, Location, Node};
use craft_ui::layout::{SplitAxis, SplitSize};
use serde::{Deserialize, Serialize};

/// File name of the persisted layout inside the codemason config directory.
const LAYOUT_FILE: &str = "gui-layout.json";

/// Identity of one dockable pane in the GUI window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PaneId {
    /// The conversation transcript.
    Messages,
    /// The prompt editor.
    Prompt,
    /// The session list.
    Sessions,
    /// The tracing log.
    Log,
    /// The active agent profile.
    Profile,
    /// The task list.
    Tasks,
    /// The memory browser.
    Memory,
    /// The running agents.
    ActiveAgents,
    /// The agent teams.
    Teams,
    /// The queued prompts.
    Queue,
}

/// Where each hidden pane was docked, keyed by pane, so showing it again
/// puts it back in the same place.
pub type HiddenPanes = BTreeMap<PaneId, Location<PaneId>>;

/// One tab group holding `panels`, with the first one active.
fn tabs(panels: Vec<PaneId>) -> Node<PaneId> {
    Node::Tabs { panels, active: 0 }
}

/// A split of `first` and `second` along `axis`, sized by `size`.
fn split(
    axis: SplitAxis,
    size: SplitSize,
    first: Node<PaneId>,
    second: Node<PaneId>,
) -> Node<PaneId> {
    Node::Split {
        axis,
        size,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Build the layout a first launch (or an unreadable layout file) uses.
///
/// Sessions, profile and memory sit in a left column; the transcript sits
/// above the prompt in the centre; agents, teams, tasks and the queue sit
/// above the log in a right column. Every [`PaneId`] is shown.
///
/// # Returns
///
/// A layout that passes [`Layout::validate`].
pub fn default_layout() -> Layout<PaneId> {
    let left = tabs(vec![PaneId::Sessions, PaneId::Profile, PaneId::Memory]);
    let centre = split(
        SplitAxis::Vertical,
        SplitSize::FixedSecond(120.0),
        tabs(vec![PaneId::Messages]),
        tabs(vec![PaneId::Prompt]),
    );
    let right = split(
        SplitAxis::Vertical,
        SplitSize::Ratio(0.6),
        tabs(vec![
            PaneId::ActiveAgents,
            PaneId::Teams,
            PaneId::Tasks,
            PaneId::Queue,
        ]),
        tabs(vec![PaneId::Log]),
    );
    let main = split(SplitAxis::Horizontal, SplitSize::Ratio(0.75), centre, right);
    Layout {
        root: Some(split(
            SplitAxis::Horizontal,
            SplitSize::Ratio(0.2),
            left,
            main,
        )),
        floating: Vec::new(),
    }
}

/// Hide `pane` if it is in `layout`, otherwise show it again.
///
/// Hiding records the pane's [`Location`] in `hidden` before closing it.
/// Showing restores it at that location; when none was recorded, or its
/// anchor is gone, the pane is opened at craft-ui's default place.
///
/// # Arguments
///
/// * `layout` - the layout to change.
/// * `hidden` - the remembered locations of hidden panes.
/// * `pane` - the pane to toggle.
///
/// # Errors
///
/// Returns the [`DockError`] craft-ui reports when the layout is invalid or
/// the change is rejected; `layout` is then unchanged.
pub fn toggle_pane(
    layout: &mut Layout<PaneId>,
    hidden: &mut HiddenPanes,
    pane: PaneId,
) -> Result<(), DockError> {
    if layout.contains(&pane) {
        let location = layout.location(&pane)?;
        layout.apply(Action::Close { panel: pane })?;
        hidden.insert(pane, location);
        return Ok(());
    }
    let restored = hidden
        .remove(&pane)
        .map(|location| layout.restore(pane, &location));
    match restored {
        Some(Ok(())) => Ok(()),
        Some(Err(err)) => {
            tracing::debug!(?pane, %err, "saved pane location unusable; opening at default");
            open_default(layout, pane)
        }
        None => open_default(layout, pane),
    }
}

/// Open `pane` at craft-ui's default place for a panel with no anchor.
fn open_default(layout: &mut Layout<PaneId>, pane: PaneId) -> Result<(), DockError> {
    layout
        .apply(Action::Open {
            panel: pane,
            anchor: None,
        })
        .map(|_closed| ())
}

/// Resolve where the layout is persisted:
/// `<config_dir>/codemason/gui-layout.json`.
///
/// # Returns
///
/// `None` when the platform config directory cannot be determined.
pub fn layout_path() -> Option<PathBuf> {
    codemason_config::config::Config::global_config_dir().map(|dir| dir.join(LAYOUT_FILE))
}

/// Read the layout persisted at `path`.
///
/// # Arguments
///
/// * `path` - the JSON layout file.
///
/// # Returns
///
/// The stored layout, or [`default_layout`] when the file is missing, cannot
/// be parsed, or fails [`Layout::validate`]. An unreadable or invalid file is
/// reported with `tracing::warn!`.
pub fn load_layout(path: &Path) -> Layout<PaneId> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return default_layout(),
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cannot read GUI layout; using default");
            return default_layout();
        }
    };
    let parsed = serde_json::from_str::<Layout<PaneId>>(&text)
        .map_err(|err| err.to_string())
        .and_then(|layout| {
            layout
                .validate()
                .map(|()| layout)
                .map_err(|err| err.to_string())
        });
    match parsed {
        Ok(layout) => layout,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "invalid GUI layout; using default");
            default_layout()
        }
    }
}

/// Write `layout` to `path` as pretty-printed JSON, creating the parent
/// directory if needed.
///
/// # Arguments
///
/// * `path` - the JSON layout file to write.
/// * `layout` - the layout to persist.
///
/// # Errors
///
/// Returns an error when the directory cannot be created, the layout cannot
/// be serialized, or the file cannot be written.
pub fn save_layout(path: &Path, layout: &Layout<PaneId>) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(layout).context("serializing GUI layout")?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}
