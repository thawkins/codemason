//! Dockable pane layout for the GUI window.
//!
//! The window's panes ([`PaneId`]) are arranged by craft-ui's docking model,
//! [`craft_ui::docking::Layout`]. This module builds the default arrangement,
//! shows and hides a pane while remembering where it was, and persists the
//! layout, with where each hidden pane was, as JSON at
//! `<config_dir>/codemason/gui-layout.json` ([`save_dock`], [`load_dock`]).
//!
//! # Visibility
//!
//! The App's own panel flags (`sidebar.visible`, `show_log`, ...) are the
//! single source of truth for which panes are shown: [`pane_wanted`] reads
//! them, and [`sync_visibility`] hides or shows each pane through
//! [`toggle_pane`] until the layout matches, so a hidden pane comes back
//! where it was. The shell runs the sync every frame; when the user closes a
//! pane from the dock's own chrome, [`close_in_app`] hides it through
//! app-core's own action or toggle chord, and the next sync hides it.
//!
//! The shell renders the layout with craft-ui's `DockArea` and maps each
//! pane to its panel (`crate::panels`).
//!
//! A pane app-core starts sending the keyboard to ([`pane_takes_keys`], e.g.
//! the sessions sidebar after `Alt+S`) is brought to the front of its tab
//! group ([`bring_to_front`]), so keys never go to a covered pane.
//!
//! Dependencies: `craft-ui` (feature `serde`) for the layout model,
//! `serde`/`serde_json` for persistence, `codemason-config` for the config
//! directory, `codemason-app-core` for the App's panel flags, and `tracing`
//! for load warnings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use codemason_app_core::app::App;
use codemason_app_core::input_event::KeyInput;
use craft_ui::docking::{Action, DockError, Layout, Location, Node, PanelState, Permissions};
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

/// Hide `pane` through app-core, as when the user closes it from the
/// dock's chrome, so every side effect of app-core's own hide runs (the
/// pane's text selection and context menu are dropped, the status line
/// says so, the sidebar preference is saved). The next
/// [`sync_visibility`] then hides the pane, keeping its place.
///
/// The sessions sidebar, the agents window and the teams window are hidden
/// by app-core's own actions here (`sessions_sidebar_state::hide`,
/// `App::close_agents_window`, `App::close_teams_window`). The log, profile,
/// tasks and memory panels are hidden by their toggle chord, which the
/// caller sends through `App::handle_key` exactly as the keyboard shortcut
/// does: this function returns it.
///
/// # Arguments
///
/// * `app` - the App whose pane is closed.
/// * `pane` - the closed pane; an always-shown or already hidden pane is
///   left alone.
///
/// # Returns
///
/// The key press to send to `App::handle_key`, if the pane is hidden by
/// its toggle chord.
pub fn close_in_app(app: &mut App, pane: PaneId) -> Option<KeyInput> {
    if !pane_wanted(app, pane) {
        return None;
    }
    let command = match pane {
        PaneId::Messages | PaneId::Prompt | PaneId::Queue => return None,
        PaneId::Sessions => {
            codemason_app_core::sessions_sidebar_state::hide(app);
            return None;
        }
        PaneId::ActiveAgents => {
            app.close_agents_window();
            return None;
        }
        PaneId::Teams => {
            app.close_teams_window();
            return None;
        }
        PaneId::Log => "panel.log",
        PaneId::Profile => "panel.profile",
        PaneId::Tasks => "panel.tasks",
        PaneId::Memory => "panel.memory",
    };
    crate::keys::command_key_input(&craft_keymap::CommandRef::new(command))
}

/// Whether app-core sends the keyboard to `pane` now: the focused sessions
/// sidebar, the shown Memory panel (Up/Down move its cursor), and the
/// shown agents or teams window (while no output view covers it).
///
/// # Arguments
///
/// * `app` - the App whose key routing is read.
/// * `pane` - the pane in question.
///
/// # Returns
///
/// `true` when app-core's key handler acts on the pane.
#[must_use]
pub fn pane_takes_keys(app: &App, pane: PaneId) -> bool {
    match pane {
        PaneId::Sessions => app.sidebar.focused && app.sidebar.visible,
        PaneId::Memory => app.show_memory,
        PaneId::ActiveAgents => app.show_agents_window && app.output_view.is_none(),
        PaneId::Teams => app.show_teams_window && app.output_view.is_none(),
        PaneId::Messages
        | PaneId::Prompt
        | PaneId::Log
        | PaneId::Profile
        | PaneId::Tasks
        | PaneId::Queue => false,
    }
}

/// Bring `pane` to the front of its tab group when it is in `layout` and
/// not already there.
///
/// # Errors
///
/// Returns the [`DockError`] craft-ui reports for the activation.
pub fn bring_to_front(layout: &mut Layout<PaneId>, pane: PaneId) -> Result<bool, DockError> {
    if !layout.contains(&pane) || layout.panel_state(&pane) == PanelState::Front {
        return Ok(false);
    }
    layout.apply(Action::Activate { panel: pane })?;
    Ok(true)
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
/// Showing restores it at that location while the panes it was next to are
/// still shown. Otherwise (nothing recorded, or its neighbours hidden too)
/// the pane goes where [`default_layout`] puts it among the panes shown
/// now, and only when that fails at craft-ui's default place.
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
    let remembered = hidden
        .remove(&pane)
        .filter(|location| place_still_exists(layout, location));
    if let Some(location) = remembered {
        match layout.restore(pane, &location) {
            Ok(()) => return Ok(()),
            Err(err) => tracing::debug!(?pane, %err, "saved pane location unusable"),
        }
    }
    if let Some(location) = designed_location(layout, pane) {
        match layout.restore(pane, &location) {
            Ok(()) => return Ok(()),
            Err(err) => tracing::debug!(?pane, %err, "designed pane location unusable"),
        }
    }
    open_default(layout, pane)
}

/// Whether the place `location` describes is still in `layout`: its
/// anchor and every pane of the sibling subtree it was split from are
/// shown. A place next to panes that have since been hidden would put the
/// pane beside only part of what it was next to.
fn place_still_exists(layout: &Layout<PaneId>, location: &Location<PaneId>) -> bool {
    location
        .anchor
        .as_ref()
        .is_none_or(|anchor| layout.contains(anchor))
        && location.sibling.iter().all(|pane| layout.contains(pane))
}

/// Where [`default_layout`] puts `pane` among the panes `layout` shows now:
/// the default layout with every other pane `layout` does not show closed,
/// then `pane`'s place in it.
fn designed_location(layout: &Layout<PaneId>, pane: PaneId) -> Option<Location<PaneId>> {
    let mut designed = default_layout();
    for other in PaneId::ALL {
        if other != pane && !layout.contains(&other) && designed.contains(&other) {
            designed.apply(Action::Close { panel: other }).ok()?;
        }
    }
    designed.location(&pane).ok()
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

/// The layout file: the layout and where each hidden pane was.
#[derive(Serialize)]
struct DockFile<'a> {
    layout: &'a Layout<PaneId>,
    hidden: &'a HiddenPanes,
}

/// A layout file as read: the current format, or the older one holding the
/// bare layout only (written before hidden panes were saved).
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredDock {
    /// `{ "layout": ..., "hidden": ... }`.
    Full {
        layout: Layout<PaneId>,
        #[serde(default)]
        hidden: HiddenPanes,
    },
    /// The bare layout.
    Bare(Layout<PaneId>),
}

/// Read the layout and the hidden panes' places persisted at `path` (see
/// [`save_dock`]); a file holding only a layout (the older format, or
/// [`save_layout`]) loads with no hidden places.
///
/// # Arguments
///
/// * `path` - the JSON layout file.
///
/// # Returns
///
/// The stored layout and hidden places, or [`default_layout`] with none
/// when the file is missing, cannot be parsed, or its layout fails
/// [`Layout::validate`]. An unreadable or invalid file is reported with
/// `tracing::warn!`. A hidden place recorded for a pane the layout shows is
/// dropped.
pub fn load_dock(path: &Path) -> (Layout<PaneId>, HiddenPanes) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (default_layout(), HiddenPanes::new());
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cannot read GUI layout; using default");
            return (default_layout(), HiddenPanes::new());
        }
    };
    let parsed = serde_json::from_str::<StoredDock>(&text)
        .map_err(|err| err.to_string())
        .map(|stored| match stored {
            StoredDock::Full { layout, hidden } => (layout, hidden),
            StoredDock::Bare(layout) => (layout, HiddenPanes::new()),
        })
        .and_then(|(layout, hidden)| {
            layout
                .validate()
                .map(|()| (layout, hidden))
                .map_err(|err| err.to_string())
        });
    match parsed {
        Ok((layout, mut hidden)) => {
            hidden.retain(|pane, _| !layout.contains(pane));
            (layout, hidden)
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "invalid GUI layout; using default");
            (default_layout(), HiddenPanes::new())
        }
    }
}

/// Read the layout persisted at `path`, without the hidden panes' places
/// (see [`load_dock`]).
///
/// # Arguments
///
/// * `path` - the JSON layout file.
///
/// # Returns
///
/// The stored layout, or [`default_layout`] when the file is missing, cannot
/// be parsed, or fails [`Layout::validate`].
pub fn load_layout(path: &Path) -> Layout<PaneId> {
    load_dock(path).0
}

/// Write `layout` and the hidden panes' places to `path` as pretty-printed
/// JSON (`{ "layout": ..., "hidden": ... }`), creating the parent directory
/// if needed, so a pane hidden at exit reopens where it was.
///
/// # Arguments
///
/// * `path` - the JSON layout file to write.
/// * `layout` - the layout to persist.
/// * `hidden` - where each hidden pane was docked.
///
/// # Errors
///
/// Returns an error when the directory cannot be created, the state cannot
/// be serialized, or the file cannot be written.
pub fn save_dock(path: &Path, layout: &Layout<PaneId>, hidden: &HiddenPanes) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let json = serde_json::to_string_pretty(&DockFile { layout, hidden })
        .context("serializing GUI layout")?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}

/// Write `layout` alone to `path` as pretty-printed JSON (the older file
/// format, with no hidden places; [`save_dock`] writes both), creating the
/// parent directory if needed.
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
