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
//! # Fixed tabs
//!
//! The Context, log, profile, tasks and memory panes are fixed tabs beside
//! the sessions sidebar ([`PaneId::LEFT_TABS`], pinned by
//! [`pin_left_tabs`]): the dock's chrome neither closes, moves nor floats
//! them, and [`pane_wanted`] keeps them present whenever the sidebar is.
//! Their flags pick the group's front tab rather than its membership, so
//! `Alt+L`, `Alt+P`, `Alt+I` and `Alt+M` bring one to the front instead of
//! displacing the others, and the shell turns the panel they cover off when
//! the user picks another tab.
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
use craft_ui::docking::{
    Action, DockError, Layout, Location, Node, PanelState, Permissions, Placement,
};
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
    /// The context-window breakdown: a pinned tab beside the sessions
    /// sidebar (`Alt+C` brings it to the front).
    Context,
    /// The running agents.
    ActiveAgents,
    /// The agent teams.
    Teams,
    /// The queued prompts.
    Queue,
    /// The diff of an edit, opened from the transcript.
    Diff,
}

impl PaneId {
    /// Every pane, in declaration order.
    pub const ALL: [Self; 12] = [
        Self::Messages,
        Self::Prompt,
        Self::Sessions,
        Self::Log,
        Self::Profile,
        Self::Tasks,
        Self::Memory,
        Self::Context,
        Self::ActiveAgents,
        Self::Teams,
        Self::Queue,
        Self::Diff,
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
            Self::Context => "Context",
            Self::ActiveAgents => "Active agents",
            Self::Teams => "Teams",
            Self::Queue => "Queue",
            Self::Diff => "Diff",
        }
    }

    /// What the dock's chrome may do with the pane.
    ///
    /// The transcript and the prompt are the window's canvas: they can
    /// neither move, close nor take tabs, so craft-ui draws them without a
    /// tab strip. The Context, log, profile, tasks and memory panes are fixed
    /// tabs beside the sessions sidebar ([`Self::LEFT_TABS`]): they stay in
    /// their group and cannot be closed, moved or floated, but they take tabs
    /// and splits. Every other pane takes craft-ui's defaults.
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
            Self::Context | Self::Log | Self::Profile | Self::Tasks | Self::Memory => Permissions {
                movable: false,
                floatable: false,
                closable: false,
                accepts_tabs: true,
                accepts_splits: true,
            },
            Self::Sessions | Self::ActiveAgents | Self::Teams | Self::Queue | Self::Diff => {
                Permissions::default()
            }
        }
    }

    /// Whether the pane is a fixed tab of the left column: it sits beside the
    /// sessions sidebar, in [`Self::LEFT_TABS`] order, and the dock's chrome
    /// neither closes, moves nor floats it ([`Self::permissions`]). Its panel
    /// is turned on and off by its toggle chord instead, which brings its tab
    /// to the front (`App::show_log` and its siblings).
    ///
    /// # Returns
    ///
    /// `true` for the panes pinned beside the sidebar.
    #[must_use]
    pub const fn is_left_tab(self) -> bool {
        matches!(
            self,
            Self::Context | Self::Log | Self::Profile | Self::Tasks | Self::Memory
        )
    }

    /// The fixed tabs of the left column, after the sessions sidebar, in tab
    /// order.
    pub const LEFT_TABS: [Self; 5] = [
        Self::Context,
        Self::Log,
        Self::Profile,
        Self::Tasks,
        Self::Memory,
    ];
}

/// Whether the App wants `pane` shown, read from the App's own flags.
///
/// The transcript and the prompt are always shown. The fixed tabs beside the
/// sessions sidebar ([`PaneId::LEFT_TABS`]) go with the sidebar, so hiding the
/// sidebar still collapses the left column: they are shown while
/// `App::sidebar.visible` is, and one of them is shown on its own while its
/// flag says it is in front (`Alt+C`, `Alt+L`, `Alt+P`, `Alt+I`, `Alt+M`), so
/// a panel turned on with the sidebar hidden still appears. Their flags pick
/// the group's front tab rather than its membership, so the five coexist as
/// tabs instead of displacing each other. The input queue is drawn above the
/// prompt (as in the TUI), so its pane stays closed.
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
        PaneId::Context => app.sidebar.visible || app.show_context_panel,
        PaneId::Log => app.sidebar.visible || app.show_log,
        PaneId::Profile => app.sidebar.visible || app.show_profile,
        PaneId::Tasks => app.sidebar.visible || app.show_tasks_panel,
        PaneId::Memory => app.sidebar.visible || app.show_memory,
        PaneId::Sessions => app.sidebar.visible,
        PaneId::ActiveAgents => app.show_agents_window,
        PaneId::Teams => app.show_teams_window,
        PaneId::Queue => false,
        PaneId::Diff => app.diff_pane.shown,
    }
}

/// The panel a fixed tab of the left column shows, read from the App's flags:
/// the one whose `show_*` flag is set, or `None` when they are all off (the
/// sidebar is in front). app-core sets at most one of them at a time.
///
/// # Arguments
///
/// * `app` - the App whose panel flags are read.
///
/// # Returns
///
/// The flagged pane, in [`PaneId::LEFT_TABS`] order.
#[must_use]
pub fn shown_left_tab(app: &App) -> Option<PaneId> {
    PaneId::LEFT_TABS
        .into_iter()
        .find(|pane| pane_is_flag_set(app, *pane))
}

/// Whether `pane`'s `show_*` flag is set, i.e. app-core wants its panel
/// visible in the left column.
///
/// # Arguments
///
/// * `app` - the App whose panel flags are read.
/// * `pane` - the fixed tab in question.
///
/// # Returns
///
/// `false` for a pane that is not a fixed tab of the left column.
#[must_use]
pub fn pane_is_flag_set(app: &App, pane: PaneId) -> bool {
    match pane {
        PaneId::Context => app.show_context_panel,
        PaneId::Log => app.show_log,
        PaneId::Profile => app.show_profile,
        PaneId::Tasks => app.show_tasks_panel,
        PaneId::Memory => app.show_memory,
        PaneId::Messages
        | PaneId::Prompt
        | PaneId::Sessions
        | PaneId::ActiveAgents
        | PaneId::Teams
        | PaneId::Queue
        | PaneId::Diff => false,
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
/// does: this function returns it. The Diff pane has no toggle chord (the
/// pane's own `Close diff` button hides it as well), so it is hidden by its
/// flag here. The pinned Context tab cannot be closed, so it is left
/// alone.
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
        PaneId::Messages | PaneId::Prompt | PaneId::Queue | PaneId::Context => return None,
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
        PaneId::Diff => {
            app.diff_pane.shown = false;
            return None;
        }
        PaneId::Log | PaneId::Profile | PaneId::Tasks | PaneId::Memory => panel_command(pane)?,
    };
    crate::keys::command_key_input(&craft_keymap::CommandRef::new(command))
}

/// The keymap command that toggles `pane`, for the panels app-core shows and
/// hides by a chord: the log, profile, tasks and memory panels.
///
/// # Arguments
///
/// * `pane` - the pane in question.
///
/// # Returns
///
/// The command id (e.g. `panel.log`), or `None` for a pane with no toggle
/// command.
#[must_use]
pub const fn panel_command(pane: PaneId) -> Option<&'static str> {
    match pane {
        PaneId::Context => Some("panel.context"),
        PaneId::Log => Some("panel.log"),
        PaneId::Profile => Some("panel.profile"),
        PaneId::Tasks => Some("panel.tasks"),
        PaneId::Memory => Some("panel.memory"),
        PaneId::Messages
        | PaneId::Prompt
        | PaneId::Sessions
        | PaneId::ActiveAgents
        | PaneId::Teams
        | PaneId::Queue
        | PaneId::Diff => None,
    }
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
        | PaneId::Context
        | PaneId::Queue
        | PaneId::Diff => false,
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
/// The sessions sidebar and the five fixed tabs beside it
/// ([`PaneId::LEFT_TABS`]) form the left column, sized wide enough for six
/// tabs at craft-ui's minimum width; the transcript sits above the prompt in
/// the centre; the agents, teams, queue and diff panes share the right
/// column. Every [`PaneId`] is placed; [`sync_visibility`] then hides the ones
/// the App does not want, keeping their place.
///
/// # Returns
///
/// A layout that passes [`Layout::validate`].
pub fn default_layout() -> Layout<PaneId> {
    let mut left_tabs = vec![PaneId::Sessions];
    left_tabs.extend(PaneId::LEFT_TABS);
    let left = tabs(left_tabs);
    let centre = split(
        SplitAxis::Vertical,
        SplitSize::FixedSecond(120.0),
        tabs(vec![PaneId::Messages]),
        tabs(vec![PaneId::Prompt]),
    );
    let right = tabs(vec![
        PaneId::ActiveAgents,
        PaneId::Teams,
        PaneId::Queue,
        PaneId::Diff,
    ]);
    let main = split(SplitAxis::Horizontal, SplitSize::Ratio(0.75), centre, right);
    Layout {
        root: Some(split(
            SplitAxis::Horizontal,
            SplitSize::Ratio(0.28),
            left,
            main,
        )),
        floating: Vec::new(),
    }
}

/// Pin the fixed tabs of the left column into the sessions sidebar's tab
/// group, in [`PaneId::LEFT_TABS`] order after the sidebar's tab.
///
/// A layout saved before these panes became fixed tabs may have them
/// elsewhere (the log and tasks panes were in the right column, the profile
/// and memory panes after another pane) or hidden; this moves each into the
/// group that holds the sessions sidebar, in order, and forgets where it was
/// hidden. A layout without the sessions sidebar (hidden) is left alone, so
/// the panes keep their hidden places; the shell calls this again each frame,
/// so they are pinned as soon as the sidebar is shown.
///
/// # Arguments
///
/// * `layout` - the layout to change.
/// * `hidden` - the remembered locations of hidden panes.
///
/// # Returns
///
/// Whether the layout changed.
///
/// # Errors
///
/// Returns the [`DockError`] craft-ui reports for the first move.
pub fn pin_left_tabs(
    layout: &mut Layout<PaneId>,
    hidden: &mut HiddenPanes,
) -> Result<bool, DockError> {
    if !layout.contains(&PaneId::Sessions) {
        return Ok(false);
    }
    // Moving or opening a pane makes it its group's active tab, so remember
    // which of the group's panes was showing and put it back afterwards: a
    // migration must not drop the user onto the pane that moved last.
    let front_before = std::iter::once(PaneId::Sessions)
        .chain(PaneId::LEFT_TABS)
        .find(|pane| layout.panel_state(pane) == PanelState::Front);
    let mut changed = false;
    // Each pane goes before the tab that follows its predecessor, so the
    // group ends up in `LEFT_TABS` order whatever order they arrive in.
    let mut after = PaneId::Sessions;
    for pane in PaneId::LEFT_TABS {
        hidden.remove(&pane);
        let next = match layout.location(&after).map(|at| at.placement) {
            Ok(Placement::Tab { before }) => before,
            Ok(_) | Err(_) => break,
        };
        if layout.contains(&pane) && next == Some(pane) {
            after = pane;
            continue;
        }
        let action = if layout.contains(&pane) {
            Action::Move {
                panel: pane,
                anchor: after,
                placement: Placement::Tab { before: next },
            }
        } else {
            Action::OpenAt {
                panel: pane,
                anchor: after,
                placement: Placement::Tab { before: next },
            }
        };
        layout.apply(action)?;
        changed = true;
        after = pane;
    }
    // The pane that was showing keeps the front; joining the group must not
    // cover a pane the user was reading.
    if let Some(front) = front_before.filter(|pane| layout.contains(pane)) {
        layout.apply(Action::Activate { panel: front })?;
    }
    Ok(changed)
}

/// Size the split holding the prompt pane so the pane is `height` points
/// tall, measured up from the split's bottom edge: the prompt grows and
/// shrinks at its top only.
///
/// # Arguments
///
/// * `layout` - the layout to change.
/// * `height` - the prompt pane's wanted height, in points.
///
/// # Returns
///
/// `true` when the split's size changed.
pub fn fit_prompt(layout: &mut Layout<PaneId>, height: f32) -> bool {
    fn fit(node: &mut Node<PaneId>, height: f32) -> bool {
        let Node::Split {
            axis,
            size,
            first,
            second,
        } = node
        else {
            return false;
        };
        let holds_prompt =
            matches!(&**second, Node::Tabs { panels, .. } if panels == &[PaneId::Prompt]);
        if *axis == SplitAxis::Vertical && holds_prompt {
            let fitted = SplitSize::FixedSecond(height);
            let same = matches!(size, SplitSize::FixedSecond(now) if (*now - height).abs() <= 0.5);
            if !same {
                *size = fitted;
            }
            return !same;
        }
        fit(first, height) || fit(second, height)
    }
    if !height.is_finite() || height <= 0.0 {
        return false;
    }
    layout.root.as_mut().is_some_and(|root| fit(root, height))
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
        Ok((mut layout, mut hidden)) => {
            hidden.retain(|pane, _| !layout.contains(pane));
            if let Err(err) = pin_left_tabs(&mut layout, &mut hidden) {
                tracing::warn!(%err, "cannot pin the left column's fixed tabs; using default");
                return (default_layout(), HiddenPanes::new());
            }
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
