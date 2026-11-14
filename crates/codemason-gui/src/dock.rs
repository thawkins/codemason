//! Dockable pane layout for the GUI window.
//!
//! The window's panes ([`PaneId`]) are arranged by craft-ui's docking model,
//! [`craft_ui::docking::Layout`]. This module builds the default arrangement,
//! shows and hides a pane while remembering where it was, and persists the
//! layout as JSON at `<config_dir>/codemason/gui-layout.json`.
//!
//! # Visibility
//!
//! The App's own panel flags (`sidebar.visible`, `show_log`, ...) are the
//! single source of truth for which panes are shown: [`pane_wanted`] reads
//! them, and [`sync_visibility`] hides or shows each pane through
//! [`toggle_pane`] until the layout matches, so a hidden pane comes back
//! where it was. The shell runs the sync every frame; when the user closes a
//! pane from the dock's own chrome, [`hide_in_app`] clears the App's flag
//! instead and the next sync hides it.
//!
//! The shell renders the layout with craft-ui's `DockArea` and maps each
//! pane to its panel (`crate::panels`).
//!
//! Dependencies: `craft-ui` (feature `serde`) for the layout model,
//! `serde`/`serde_json` for persistence, `codemason-config` for the config
//! directory, `codemason-app-core` for the App's panel flags, and `tracing`
//! for load warnings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use codemason_app_core::app::App;
use craft_ui::docking::{Action, DockError, Layout, Location, Node, Permissions};
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

impl PaneId {
    /// Every pane, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::Messages,
        Self::Prompt,
        Self::Sessions,
        Self::Log,
        Self::Profile,
        Self::Tasks,
        Self::Memory,
        Self::ActiveAgents,
        Self::Teams,
        Self::Queue,
    ];

    /// The pane's name: its tab text and the accessible label of its panel.
    ///
    /// # Returns
    ///
    /// A short title-case name, e.g. `"Sessions"`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Messages => "Messages",
            Self::Prompt => "Prompt",
            Self::Sessions => "Sessions",
            Self::Log => "Log",
            Self::Profile => "Profile",
            Self::Tasks => "Tasks",
            Self::Memory => "Memory",
            Self::ActiveAgents => "Active agents",
            Self::Teams => "Teams",
            Self::Queue => "Queue",
        }
    }

    /// What the dock's chrome may do with the pane.
    ///
    /// The transcript and the prompt are the window's canvas: they can
    /// neither move, close nor take tabs, so craft-ui draws them without a
    /// tab strip. Every other pane takes craft-ui's defaults.
    ///
    /// # Returns
    ///
    /// The pane's docking permissions.
    #[must_use]
    pub fn permissions(self) -> Permissions {
        match self {
            Self::Messages | Self::Prompt => Permissions {
                accepts_splits: true,
                ..Permissions::PROTECTED
            },
            Self::Sessions
            | Self::Log
            | Self::Profile
            | Self::Tasks
            | Self::Memory
            | Self::ActiveAgents
            | Self::Teams
            | Self::Queue => Permissions::default(),
        }
    }
}

/// Whether the App wants `pane` shown, read from the App's own flags.
///
/// The transcript and the prompt are always shown. The input queue is drawn
/// above the prompt (as in the TUI), so its pane stays closed.
///
/// # Arguments
///
/// * `app` - the App whose panel flags are read.
/// * `pane` - the pane in question.
///
/// # Returns
///
/// `true` when the pane should be in the layout.
#[must_use]
pub fn pane_wanted(app: &App, pane: PaneId) -> bool {
    match pane {
        PaneId::Messages | PaneId::Prompt => true,
        PaneId::Sessions => app.sidebar.visible,
        PaneId::Log => app.show_log,
        PaneId::Profile => app.show_profile,
        PaneId::Tasks => app.show_tasks_panel,
        PaneId::Memory => app.show_memory,
        PaneId::ActiveAgents => app.show_agents_window,
        PaneId::Teams => app.show_teams_window,
        PaneId::Queue => false,
    }
}

/// Clear the App's flag for `pane`, as when the user closes the pane from
/// the dock's chrome. The next [`sync_visibility`] then hides it, keeping
/// its place. Closing the sessions sidebar also takes its focus away and
/// saves its preference, as `Alt+S` does when it hides it.
///
/// # Arguments
///
/// * `app` - the App whose flag is cleared.
/// * `pane` - the closed pane; the always-shown panes are left alone.
pub fn hide_in_app(app: &mut App, pane: PaneId) {
    match pane {
        PaneId::Messages | PaneId::Prompt | PaneId::Queue => {}
        PaneId::Sessions => {
            app.sidebar.visible = false;
            app.sidebar.focused = false;
            app.sidebar.save(&app.storage);
        }
        PaneId::Log => app.show_log = false,
        PaneId::Profile => app.show_profile = false,
        PaneId::Tasks => app.show_tasks_panel = false,
        PaneId::Memory => app.show_memory = false,
        PaneId::ActiveAgents => app.show_agents_window = false,
        PaneId::Teams => app.show_teams_window = false,
    }
}

/// Hide or show panes until `layout` holds exactly the panes `wanted`
/// accepts, through [`toggle_pane`] (so a pane shown again goes back where
/// it was hidden). A pane that is shown again becomes the active tab of its
/// group.
///
/// # Arguments
///
/// * `layout` - the layout to change.
/// * `hidden` - the remembered locations of hidden panes.
/// * `wanted` - whether each pane should be shown, e.g. [`pane_wanted`].
///
/// # Returns
///
/// Whether the layout changed.
///
/// # Errors
///
/// Returns the first [`DockError`] craft-ui reports; the panes before it
/// are already synced.
pub fn sync_visibility(
    layout: &mut Layout<PaneId>,
    hidden: &mut HiddenPanes,
    wanted: impl Fn(PaneId) -> bool,
) -> Result<bool, DockError> {
    let mut changed = false;
    for pane in PaneId::ALL {
        let shown = layout.contains(&pane);
        if shown != wanted(pane) {
            toggle_pane(layout, hidden, pane)?;
            if !shown {
                // A pane that comes back is brought to the front of its tab
                // group, as the TUI shows the panel its key turned on.
                layout.apply(Action::Activate { panel: pane })?;
            }
            changed = true;
        }
    }
    Ok(changed)
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
