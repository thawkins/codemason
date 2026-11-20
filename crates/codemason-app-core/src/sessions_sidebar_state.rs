//! Left-hand session sidebar state and behaviour (spec MULTISESSION,
//! milestone M5; spec `projects`). Frontend-neutral: painting lives in the
//! terminal frontend's `sessions_sidebar` module.
//!
//! Lists every stored session, a `Pinned` section first and then one section
//! per project (the launch project first, the rest by most recent activity),
//! under a `+ New session` row and a `/ Search` row. A project heading shows
//! the project's name, a chevron (`v` open, `>` collapsed) and its remote
//! host kind; a project with no sessions is listed too. Each session row
//! shows the title, a relative age, the branch marker with the count of
//! pull/merge requests for the session's branch, and an attention marker at
//! the right edge; the focused session is highlighted.
//!
//! Data from storage (groups, titles, pins, branches, request counts,
//! hidden projects) is cached in [`SidebarState`] and rebuilt
//! only when the state is marked dirty by a session or project event or a
//! command (NFR-03). Attention markers come from the open views in memory,
//! so they are read live each frame.
//!
//! Keys: `Alt+S` shows/focuses/hides the sidebar, `Ctrl+N` opens a session,
//! `Ctrl+Up`/`Ctrl+Down` move to the previous/next session, `Alt+R` returns
//! to the most recently focused one. While the sidebar has focus: arrows and
//! `PageUp`/`PageDown` move, `Enter` acts on the row, `Left`/`Right` collapse
//! and expand a section, `p` pins, `a` archives, `F2` renames, `/` filters,
//! `u` marks unread/read, `f` forks, `D` deletes (after a confirmation),
//! `Esc` clears the filter or returns focus to the input. On a project
//! heading `n` starts a session in that project and `b` prefills the command
//! that starts one in its own worktree at a branch. `m`, `Shift+F10` or a
//! right-click on a session row opens its context menu, and on a project
//! heading the project's menu (`session_menu`). `v` or
//! `Ctrl`+click adds a session to the selection (or takes it out); `m` on a
//! selected session opens a menu acting on the whole selection, and `Esc`
//! clears it. Projects keep a manual order once one is moved up or down.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crate::geometry::Area;
use crate::input_event::{Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind};
use codemason_config::ExecutionBackendKind;
use codemason_storage::{ProjectSessions, SessionCheckout, SidebarSection, Storage, project_root};
use serde::{Deserialize, Serialize};

use crate::app::{App, Attention};

/// Settings key holding the persisted sidebar layout.
const SIDEBAR_KEY: &str = "tui.sessions.sidebar";
/// Default sidebar width in columns, border included.
pub const DEFAULT_WIDTH: u16 = 32;
/// Narrowest the sidebar can be dragged to.
pub const MIN_WIDTH: u16 = 20;
/// Section key of the `Pinned` pseudo-project; NUL keeps it apart from ids.
#[doc(hidden)]
pub const PINNED_KEY: &str = "\0pinned";
/// ASCII marker shown before a session's request count; it stands for a
/// branch.
pub const BRANCH_MARK: char = 'Y';

/// Sidebar layout and cached session data.
#[derive(Debug)]
pub struct SidebarState {
    /// User preference: show the sidebar when the terminal is wide enough.
    pub visible: bool,
    /// Whether the sidebar has keyboard focus.
    pub focused: bool,
    /// Width in columns, border included.
    pub width: u16,
    /// Cursor row, an index into the rows from [`build_rows`].
    pub cursor: usize,
    /// First row shown.
    pub scroll: usize,
    /// Filter text; empty means no filter (FR-09).
    pub filter: String,
    /// Whether keys are currently typed into the filter.
    pub filter_editing: bool,
    /// Section keys (project ids, or the pinned key) that are collapsed.
    pub collapsed: BTreeSet<String>,
    /// Whether the cached storage data must be reloaded before the next use.
    pub dirty: bool,
    /// Cached non-archived top-level sessions grouped by project.
    pub groups: Vec<ProjectSessions>,
    /// Cached ids of pinned sessions.
    pub pinned: HashSet<String>,
    /// Cached ids of sessions marked unread in storage (SESSIONMENU FR-08).
    pub unread: HashSet<String>,
    /// Cached bound sessions, each with whether Auto-fix is paused waiting
    /// on the user (SESSIONMENU FR-22).
    pub bound: HashMap<String, bool>,
    /// Cached number of pull/merge requests each session has had (created
    /// or bound); sessions with none are absent.
    pub request_totals: HashMap<String, usize>,
    /// Cached execution backend of each session that has its own; the rest
    /// use [`SidebarState::default_backend`].
    pub backends: HashMap<String, ExecutionBackendKind>,
    /// Cached execution backend of sessions without their own: the global
    /// one, from the config.
    pub default_backend: ExecutionBackendKind,
    /// Cached user-defined sections, in display order (SESSIONMENU FR-11).
    pub sections: Vec<SidebarSection>,
    /// Cached section id of each session placed in a section.
    pub session_section: HashMap<String, String>,
    /// Cached ids of projects not listed: projects with no sessions whose
    /// directory no longer exists, or is a parent folder (not a git
    /// repository, holding another project). The launch project is listed
    /// regardless.
    pub hidden: HashSet<String>,
    /// Cached branch, worktree and request count of each session that has a
    /// branch (spec `projects`).
    pub checkouts: HashMap<String, SessionCheckout>,
    /// Area painted last frame; zero-sized when hidden.
    pub area: Area,
    /// Whether the right border is being dragged to resize.
    pub dragging: bool,
    /// The session whose row the left button went down on; releasing it
    /// over the `Pinned` heading pins that session.
    pub drag_session: Option<String>,
    /// The open session context menu, if any (spec SESSIONMENU).
    pub menu: Option<crate::session_menu::SessionMenu>,
    /// The open project `Properties` dialog, if any.
    pub properties: Option<crate::project_properties::ProjectProperties>,
    /// Ids of the sessions in the multi-selection (not persisted).
    pub selected: BTreeSet<String>,
    /// Manual project order (project ids, top first), set by the project
    /// menu's `Move up`/`Move down`; empty means the default order.
    pub project_order: Vec<String>,
    /// Test hook: when set, `Open in` records what it would start here
    /// instead of starting it.
    #[doc(hidden)]
    pub launch_log: Option<Vec<crate::open_in::LaunchSpec>>,
    /// The stored title of the focused session when the sidebar does not
    /// list it (archived, child or non-interactive), as `(id, title)`; read
    /// once per focused session and dropped on every refresh.
    pub focused_title: Option<(String, String)>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            visible: true,
            focused: false,
            width: DEFAULT_WIDTH,
            cursor: 0,
            scroll: 0,
            filter: String::new(),
            filter_editing: false,
            collapsed: BTreeSet::new(),
            dirty: true,
            groups: Vec::new(),
            pinned: HashSet::new(),
            unread: HashSet::new(),
            bound: HashMap::new(),
            request_totals: HashMap::new(),
            backends: HashMap::new(),
            default_backend: ExecutionBackendKind::Local,
            sections: Vec::new(),
            session_section: HashMap::new(),
            hidden: HashSet::new(),
            checkouts: HashMap::new(),
            area: Area::default(),
            dragging: false,
            drag_session: None,
            menu: None,
            properties: None,
            selected: BTreeSet::new(),
            project_order: Vec::new(),
            launch_log: None,
            focused_title: None,
        }
    }
}

/// The persisted part of [`SidebarState`].
#[derive(Debug, Serialize, Deserialize)]
struct Persisted {
    visible: bool,
    width: u16,
    collapsed: Vec<String>,
    /// Manual project order; absent in layouts saved before it existed.
    #[serde(default)]
    project_order: Vec<String>,
}

impl SidebarState {
    /// Loads the persisted layout, falling back to defaults.
    ///
    /// # Arguments
    ///
    /// * `storage` - the settings store.
    ///
    /// # Returns
    ///
    /// The sidebar state with the saved visibility, width and collapsed
    /// sections applied; defaults when nothing was saved or it is unreadable.
    pub fn load(storage: &Storage) -> Self {
        let mut state = Self::default();
        let saved = match storage.get_setting(SIDEBAR_KEY) {
            Ok(Some(json)) => json,
            Ok(None) => return state,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read sidebar settings");
                return state;
            }
        };
        match serde_json::from_str::<Persisted>(&saved) {
            Ok(p) => {
                state.visible = p.visible;
                state.width = p.width.max(MIN_WIDTH);
                state.collapsed = p.collapsed.into_iter().collect();
                state.project_order = p.project_order;
            }
            Err(e) => tracing::warn!(error = %e, "ignoring malformed sidebar settings"),
        }
        state
    }

    /// Saves visibility, width, collapsed sections and the project order.
    ///
    /// # Arguments
    ///
    /// * `storage` - the settings store. A failure is logged only.
    pub fn save(&self, storage: &Storage) {
        let persisted = Persisted {
            visible: self.visible,
            width: self.width,
            collapsed: self.collapsed.iter().cloned().collect(),
            project_order: self.project_order.clone(),
        };
        match serde_json::to_string(&persisted) {
            Ok(json) => {
                if let Err(e) = storage.set_setting(SIDEBAR_KEY, &json) {
                    tracing::warn!(error = %e, "failed to save sidebar settings");
                }
            }
            Err(e) => tracing::warn!(error = %e, "failed to encode sidebar settings"),
        }
    }
}

/// One painted row of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
    /// The `+ New session` action.
    NewSession,
    /// A blank line above the `Pinned` heading and each project heading.
    /// It cannot be selected: the cursor skips it and a click does nothing.
    Spacer,
    /// The `/ Search` action: starts the filter.
    Search,
    /// A section heading.
    Header {
        /// Project id, or the pinned key.
        key: String,
        /// Text shown for the section.
        label: String,
        /// Sessions in the section (after filtering).
        count: usize,
        /// Whether the section's sessions are hidden.
        collapsed: bool,
        /// The project's remote host kind (`github`, `gitlab`), if any.
        remote: Option<String>,
        /// The project's description (its `Properties`), shown dim in
        /// parentheses after the name.
        description: Option<String>,
    },
    /// A session.
    Session {
        /// Full session id.
        id: String,
        /// Text shown for the session.
        title: String,
        /// RFC 3339 last-activity time.
        updated_at: String,
        /// The session's branch, if it has one.
        branch: Option<String>,
        /// Whether the session owns its git worktree.
        owns_worktree: bool,
        /// Pull/merge requests whose source is `branch`; `None` when unknown.
        requests: Option<i64>,
    },
}

/// Reloads the cached storage data when it is marked dirty.
pub fn refresh_if_dirty(app: &mut App) {
    if !app.sidebar.dirty {
        return;
    }
    match app.storage.bound_sessions() {
        Ok(bound) => app.sidebar.bound = bound,
        Err(e) => tracing::warn!(error = %e, "session sidebar: bound sessions not read"),
    }
    match app.storage.session_request_totals() {
        Ok(totals) => app.sidebar.request_totals = totals,
        Err(e) => tracing::warn!(error = %e, "session sidebar: request totals not read"),
    }
    match app.storage.session_backends() {
        Ok(backends) => {
            app.sidebar.backends = backends
                .into_iter()
                .filter_map(|(id, kind)| ExecutionBackendKind::from_label(&kind).map(|k| (id, k)))
                .collect();
        }
        Err(e) => tracing::warn!(error = %e, "session sidebar: session backends not read"),
    }
    match codemason_agent::Config::load_for(&app.cwd_path) {
        Ok(config) => app.sidebar.default_backend = config.effective_execution_backend(),
        Err(e) => tracing::warn!(error = %e, "session sidebar: config not read for the backend"),
    }
    match (
        app.storage.list_sessions_grouped(),
        app.storage.pinned_session_ids(),
        app.storage.session_checkouts(),
        app.storage.unread_session_ids(),
        app.storage.list_sections(),
        app.storage.session_sections(),
    ) {
        (Ok(groups), Ok(pinned), Ok(checkouts), Ok(unread), Ok(sections), Ok(session_section)) => {
            app.sidebar.hidden = groups
                .iter()
                .filter(|g| {
                    let dir = Path::new(&g.project.worktree);
                    // A parent folder: holds another project, is no repo itself.
                    let container = || {
                        groups.iter().any(|other| {
                            let inner = Path::new(&other.project.worktree);
                            inner != dir && inner.starts_with(dir)
                        }) && !dir.join(".git").exists()
                    };
                    // A project with sessions stays listed, so they can
                    // still be reached, archived and deleted.
                    g.sessions.is_empty() && (container() || !dir.exists())
                })
                .map(|g| g.project_id.clone())
                .collect();
            // A session that is gone (archived, deleted) leaves the selection.
            if !app.sidebar.selected.is_empty() {
                let live: HashSet<&str> = groups
                    .iter()
                    .flat_map(|g| g.sessions.iter().map(|s| s.id.as_str()))
                    .collect();
                app.sidebar.selected.retain(|id| live.contains(id.as_str()));
            }
            app.sidebar.groups = groups;
            app.sidebar.pinned = pinned;
            app.sidebar.checkouts = checkouts;
            app.sidebar.unread = unread;
            app.sidebar.sections = sections;
            app.sidebar.session_section = session_section;
            // A rename or other change re-reads the unlisted focused title.
            app.sidebar.focused_title = None;
            app.sidebar.dirty = false;
        }
        (Err(e), ..)
        | (_, Err(e), ..)
        | (_, _, Err(e), ..)
        | (.., Err(e), _, _)
        | (.., Err(e), _)
        | (.., Err(e)) => {
            // Keep the stale rows and retry on the next frame.
            tracing::warn!(error = %e, "session sidebar refresh failed");
        }
    }
}

/// Section labels: each project's name, or `parent/name` (the parent being
/// the directory that holds the project's worktree) when two projects share
/// a name.
fn project_labels(groups: &[ProjectSessions]) -> HashMap<String, String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for g in groups {
        *counts.entry(g.project.name.as_str()).or_default() += 1;
    }
    groups
        .iter()
        .map(|g| {
            let name = g.project.name.as_str();
            let label = if counts.get(name).copied().unwrap_or(0) > 1 {
                let parent = Path::new(&g.project.worktree)
                    .parent()
                    .and_then(Path::file_name)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{parent}/{name}")
            } else {
                name.to_string()
            };
            (g.project_id.clone(), label)
        })
        .collect()
}

/// Display title for a session: its stored title, or `untitled <id tail>`.
fn display_title(id: &str, title: &str) -> String {
    if title.is_empty() {
        let tail: String = id
            .chars()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("untitled {tail}")
    } else {
        title.to_string()
    }
}

/// The display title of the focused session (its stored title, or
/// `untitled <id tail>`).
///
/// Read from the sidebar's cached rows; a session the sidebar does not list
/// (archived, child or non-interactive, e.g. one resumed at startup) is read
/// from storage once and kept in [`SidebarState::focused_title`] until the
/// focus moves or the sidebar refreshes. So a frontend may call this every
/// frame; call [`refresh_if_dirty`] first for a title renamed since the last
/// refresh.
///
/// # Returns
///
/// `None` when no session is focused.
pub fn current_title(app: &mut App) -> Option<String> {
    let id = app.session.session_id.clone()?;
    let listed = app
        .sidebar
        .groups
        .iter()
        .flat_map(|group| group.sessions.iter())
        .find(|session| session.id == id)
        .map(|session| display_title(&session.id, &session.title));
    if listed.is_some() {
        return listed;
    }
    if app
        .sidebar
        .focused_title
        .as_ref()
        .is_none_or(|(cached, _)| *cached != id)
    {
        let title = match app.storage.get_session(&id) {
            Ok(row) => row.map(|row| row.title).unwrap_or_default(),
            Err(err) => {
                tracing::warn!(error = %err, "focused session title not read");
                String::new()
            }
        };
        app.sidebar.focused_title = Some((id.clone(), title));
    }
    app.sidebar
        .focused_title
        .as_ref()
        .map(|(id, title)| display_title(id, title))
}

/// Builds the rows the sidebar shows, honouring pins, collapse and filter.
///
/// # Arguments
///
/// * `app` - for the cached data, sidebar state and launch directory.
///
/// # Returns
///
/// The rows in display order, starting with [`SidebarRow::NewSession`].
pub fn build_rows(app: &App) -> Vec<SidebarRow> {
    let filter = app.sidebar.filter.to_lowercase();
    let mut rows = vec![SidebarRow::NewSession, SidebarRow::Search];
    for section in sections(app) {
        push_section(&mut rows, app, &section, &filter);
    }
    rows
}

/// Collapse key of a user-defined section; NUL keeps it apart from paths.
pub fn user_section_key(section_id: &str) -> String {
    format!("\0section:{section_id}")
}

/// One sidebar section before filtering and collapse.
struct SectionData<'a> {
    /// Collapse key: a project root, the pinned key or a user section key.
    key: String,
    /// Heading text.
    label: String,
    /// Sessions, each with its project label (matched by the filter).
    sessions: Vec<(&'a codemason_storage::SessionRow, String)>,
    /// Forge kind of the project's remote (`github`, `gitlab`), if any.
    remote: Option<String>,
    /// The project's description, if any.
    description: Option<String>,
    /// Whether the heading is shown when the section has no sessions.
    show_empty: bool,
    /// Whether a blank line goes above the heading (`Pinned` and projects).
    spaced: bool,
}

/// Every section in display order: `Pinned`, the user sections by name,
/// then the projects (the launch project first, the rest by recency).
/// A pinned session is listed only under `Pinned`, and a session in a user
/// section only there.
fn sections(app: &App) -> Vec<SectionData<'_>> {
    let state = &app.sidebar;
    let labels = project_labels(&state.groups);
    let launch = project_root(&app.cwd_path.display().to_string());
    let all = || {
        state.groups.iter().flat_map(|g| {
            let label = labels.get(&g.project_id).cloned().unwrap_or_default();
            g.sessions.iter().map(move |s| (s, label.clone()))
        })
    };
    let newest_first = |list: &mut Vec<(&codemason_storage::SessionRow, String)>| {
        list.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
    };
    let mut out = Vec::new();

    let mut pinned: Vec<_> = all()
        .filter(|(s, _)| state.pinned.contains(&s.id))
        .collect();
    newest_first(&mut pinned);
    out.push(SectionData {
        key: PINNED_KEY.to_string(),
        label: "Pinned".to_string(),
        sessions: pinned,
        remote: None,
        description: None,
        // Always listed, so a session can be dragged onto it to pin it.
        show_empty: true,
        spaced: true,
    });

    let placed = |id: &str| state.session_section.get(id);
    for section in &state.sections {
        let mut sessions: Vec<_> = all()
            .filter(|(s, _)| !state.pinned.contains(&s.id) && placed(&s.id) == Some(&section.id))
            .collect();
        newest_first(&mut sessions);
        out.push(SectionData {
            key: user_section_key(&section.id),
            label: section.name.clone(),
            sessions,
            remote: None,
            description: None,
            show_empty: false,
            spaced: false,
        });
    }

    // A hidden project is not listed, unless codemason was launched in it.
    let mut ordered: Vec<&ProjectSessions> = state
        .groups
        .iter()
        .filter(|g| !state.hidden.contains(&g.project_id) || g.project.worktree == launch)
        .collect();
    // Stable sort: the launch project first, the rest keep recency order.
    ordered.sort_by_key(|g| g.project.worktree != launch);
    // A manual order wins. A launch project not in it still leads; other
    // projects not in it follow in the order above.
    if !state.project_order.is_empty() {
        let place: HashMap<&str, usize> = state
            .project_order
            .iter()
            .enumerate()
            .map(|(at, id)| (id.as_str(), at))
            .collect();
        ordered.sort_by_key(|g| match place.get(g.project_id.as_str()) {
            Some(&at) => (1, at),
            None if g.project.worktree == launch => (0, 0),
            None => (2, 0),
        });
    }
    for group in ordered {
        let label = labels.get(&group.project_id).cloned().unwrap_or_default();
        let sessions = group
            .sessions
            .iter()
            .filter(|s| !state.pinned.contains(&s.id) && placed(&s.id).is_none())
            .map(|s| (s, label.clone()))
            .collect();
        out.push(SectionData {
            key: group.project_id.clone(),
            label,
            sessions,
            remote: group.project.remote.as_ref().map(|r| r.kind.clone()),
            description: group.project.description.clone(),
            // A project with no sessions is still listed, so a session
            // can be started in it (spec `projects`).
            show_empty: true,
            spaced: true,
        });
    }
    out
}

/// Whether session `s` of the project labelled `project` passes the
/// lower-cased `filter`; an empty filter passes everything.
fn matches_filter(s: &codemason_storage::SessionRow, project: &str, filter: &str) -> bool {
    filter.is_empty()
        || s.title.to_lowercase().contains(filter)
        || project.to_lowercase().contains(filter)
}

/// Appends one section (heading plus visible sessions) to `rows`.
fn push_section(rows: &mut Vec<SidebarRow>, app: &App, section: &SectionData<'_>, filter: &str) {
    let matching: Vec<&codemason_storage::SessionRow> = section
        .sessions
        .iter()
        .filter(|(s, project)| matches_filter(s, project, filter))
        .map(|(s, _)| *s)
        .collect();
    // While filtering only sections with a match are shown.
    // `Pinned` stays even then: it is the drop target for drag-to-pin.
    let always = section.key == PINNED_KEY;
    if matching.is_empty() && !always && !(section.show_empty && filter.is_empty()) {
        return;
    }
    // A filter shows every match, collapsed or not.
    let collapsed = filter.is_empty() && app.sidebar.collapsed.contains(&section.key);
    if section.spaced {
        rows.push(SidebarRow::Spacer);
    }
    rows.push(SidebarRow::Header {
        key: section.key.clone(),
        label: section.label.clone(),
        count: matching.len(),
        collapsed,
        remote: section.remote.clone(),
        description: section.description.clone(),
    });
    if !collapsed {
        rows.extend(matching.into_iter().map(|s| {
            let checkout = app.sidebar.checkouts.get(&s.id);
            SidebarRow::Session {
                id: s.id.clone(),
                title: display_title(&s.id, &s.title),
                updated_at: s.updated_at.clone(),
                branch: checkout.and_then(|c| c.branch.clone()),
                owns_worktree: checkout.is_some_and(|c| c.worktree.is_some()),
                requests: checkout.and_then(|c| c.pr_count),
            }
        }));
    }
}

/// Compact age of an RFC 3339 timestamp: `now`, `5m`, `3h`, `4d` or `2w`.
///
/// # Arguments
///
/// * `timestamp` - RFC 3339 time.
/// * `now` - the reference time.
///
/// # Returns
///
/// The age, or an empty string when `timestamp` does not parse.
pub fn relative_age(timestamp: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    let Ok(then) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return String::new();
    };
    let secs = (now - then.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0);
    match secs {
        0..60 => "now".to_string(),
        60..3_600 => format!("{}m", secs / 60),
        3_600..86_400 => format!("{}h", secs / 3_600),
        86_400..604_800 => format!("{}d", secs / 86_400),
        _ => format!("{}w", secs / 604_800),
    }
}

/// The execution backend session `id` runs on: its own, else the global one.
///
/// # Arguments
///
/// * `app` - the App whose cached sidebar data is read.
/// * `id` - the session id.
///
/// # Returns
///
/// The backend kind.
pub fn backend_of(app: &App, id: &str) -> ExecutionBackendKind {
    app.sidebar
        .backends
        .get(id)
        .copied()
        .unwrap_or(app.sidebar.default_backend)
}

/// Attention of the session `id`: its open view's state, or `Unread` when
/// an otherwise idle session is marked unread in storage.
#[doc(hidden)]
pub fn attention_of(app: &App, id: &str) -> Attention {
    let live = if app.session.session_id.as_deref() == Some(id) {
        app.session.attention()
    } else {
        app.background
            .iter()
            .find(|v| v.session_id.as_deref() == Some(id))
            .map_or(Attention::Idle, |v| v.attention())
    };
    if live == Attention::Idle && app.sidebar.unread.contains(id) {
        Attention::Unread
    } else {
        live
    }
}

/// Truncates `text` to `max` characters, marking a cut with `..`.
pub fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let keep = max.saturating_sub(2);
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(&"..".chars().take(max - keep).collect::<String>());
    out
}

/// Session ids in sidebar order (pins first), ignoring collapse and filter.
fn ordered_session_ids(app: &App) -> Vec<String> {
    sections(app)
        .into_iter()
        .flat_map(|section| section.sessions)
        .map(|(s, _)| s.id.clone())
        .collect()
}

/// Reports a failed session action on the status line.
pub(crate) fn report(app: &mut App, result: Result<(), String>) {
    if let Err(message) = result {
        app.status = format!("session: {message}");
    }
    app.sidebar.dirty = true;
}

/// Focuses the session `step` places away in sidebar order (FR-12),
/// skipping sessions that cannot be opened from this directory.
fn cycle_session(app: &mut App, forward: bool) {
    refresh_if_dirty(app);
    let ids = ordered_session_ids(app);
    if ids.is_empty() {
        return;
    }
    let current = app
        .session
        .session_id
        .as_deref()
        .and_then(|id| ids.iter().position(|s| s == id));
    let len = ids.len();
    let start = current.unwrap_or(if forward { len - 1 } else { 0 });
    let mut last_error = None;
    for step in 1..=len {
        let index = if forward {
            (start + step) % len
        } else {
            (start + len - step % len) % len
        };
        match app.switch_to_session(&ids[index]) {
            Ok(()) => {
                app.sidebar.dirty = true;
                return;
            }
            Err(e) => last_error = Some(e),
        }
    }
    if let Some(e) = last_error {
        app.status = format!("session: {e}");
    }
}

/// Shows and focuses, focuses, or hides the sidebar (`Alt+S`, FR-10).
pub fn toggle(app: &mut App) {
    let state = &mut app.sidebar;
    if !state.visible {
        state.visible = true;
        state.focused = true;
    } else if !state.focused {
        state.focused = true;
    } else {
        hide(app);
        return;
    }
    app.sidebar.save(&app.storage);
}

/// Hides the sidebar and takes the keyboard from it, saving the preference,
/// as the third `Alt+S` press does. Frontends with their own close control
/// (the GUI's dock tab) call this.
pub fn hide(app: &mut App) {
    app.sidebar.visible = false;
    app.sidebar.focused = false;
    app.sidebar.save(&app.storage);
}

/// Takes the keyboard from the sidebar, leaving it shown, as a mouse press
/// outside it does. Frontends that move the keyboard elsewhere (the GUI,
/// when a pane toggled on covers the sidebar's tab) call this.
pub fn unfocus(app: &mut App) {
    app.sidebar.focused = false;
}

/// Acts on the row at `index`: open a session, toggle a section, or switch.
fn activate_row(app: &mut App, index: usize) {
    let rows = build_rows(app);
    match rows.get(index) {
        Some(SidebarRow::NewSession) => {
            let result = app.open_new_session();
            report(app, result);
            app.sidebar.focused = false;
        }
        Some(SidebarRow::Search) => start_filter(app),
        Some(SidebarRow::Spacer) => {}
        Some(SidebarRow::Header { key, .. }) => toggle_section(app, key, None),
        Some(SidebarRow::Session { id, .. }) => {
            let result = app.switch_to_session(id);
            report(app, result);
            app.sidebar.focused = false;
        }
        None => {}
    }
}

/// Drops session `id` on the row painted at screen row `row`: on the
/// `Pinned` heading it pins the session (an already pinned one is left as
/// is); anywhere else nothing happens.
fn drop_session(app: &mut App, id: &str, area: Area, row: u16) {
    let top = area.y + 1;
    if row < top || row >= area.y + area.height.saturating_sub(1) {
        return;
    }
    let index = app.sidebar.scroll + usize::from(row - top);
    let on_pinned = matches!(
        build_rows(app).get(index),
        Some(SidebarRow::Header { key, .. }) if key == PINNED_KEY
    );
    if on_pinned && !app.sidebar.pinned.contains(id) {
        let result = app.toggle_session_pin(id);
        report(app, result);
    }
}

/// The nearest selectable row to `index`, moving `forward` (down) or back
/// past [`SidebarRow::Spacer`] rows, and the other way when that runs off
/// the end.
#[doc(hidden)]
pub fn selectable(rows: &[SidebarRow], index: usize, forward: bool) -> usize {
    let is_spacer = |i: usize| matches!(rows.get(i), Some(SidebarRow::Spacer));
    if !is_spacer(index) {
        return index;
    }
    let ahead = if forward {
        (index + 1..rows.len()).find(|&i| !is_spacer(i))
    } else {
        (0..index).rev().find(|&i| !is_spacer(i))
    };
    let behind = || {
        if forward {
            (0..index).rev().find(|&i| !is_spacer(i))
        } else {
            (index + 1..rows.len()).find(|&i| !is_spacer(i))
        }
    };
    ahead.or_else(behind).unwrap_or(index)
}

/// Starts typing a new filter (the `/` key and the `/ Search` row).
fn start_filter(app: &mut App) {
    app.sidebar.filter_editing = true;
    app.sidebar.filter.clear();
}

/// The project whose heading is under the cursor, if the cursor is on a
/// project heading (not the pinned one).
fn cursor_project(app: &App) -> Option<codemason_storage::ProjectRow> {
    match build_rows(app).get(app.sidebar.cursor) {
        Some(SidebarRow::Header { key, .. }) => app
            .sidebar
            .groups
            .iter()
            .find(|g| g.project_id == *key)
            .map(|g| g.project.clone()),
        _ => None,
    }
}

/// The `/session new` command that starts a session in `project` on a new
/// branch with a random name ([`crate::branch_names`]). The project is
/// named by its name (quoted when it has spaces) when that name picks it
/// out alone, else by its id.
pub(crate) fn branch_session_command(app: &App, project: &codemason_storage::ProjectRow) -> String {
    // The stored name, which may be newer than the cached sidebar row.
    let name = match app.storage.get_project(&project.id) {
        Ok(Some(stored)) => stored.name,
        Ok(None) => project.name.clone(),
        Err(e) => {
            tracing::warn!(error = %e, "could not read project");
            project.name.clone()
        }
    };
    // The name must resolve back to this project: not shared with another
    // project, and not another project's id (ids are matched first).
    let unique = matches!(
        app.storage.find_project(&name),
        Ok(Some(found)) if found.id == project.id
    );
    // A name that cannot be written as one `--project` value falls back to
    // the id: shared with another project, empty, flag-like, or holding
    // both quote characters.
    let key = if !unique
        || name.is_empty()
        || name.starts_with("--")
        || (name.contains('"') && name.contains('\''))
    {
        project.id.clone()
    } else if name.contains('"') {
        format!("'{name}'")
    } else if name.contains(char::is_whitespace) || name.contains('\'') {
        format!("\"{name}\"")
    } else {
        name
    };
    let branches = branch_names_in(Path::new(&project.worktree));
    let branch = crate::branch_names::unused_branch_name(|name| branches.contains(name));
    format!("/session new --project {key} --branch {branch}")
}

/// Names of the local and remote-tracking branches of the repository at
/// `repo` (`main`, and `feature` for `origin/feature`); empty when git
/// cannot list them.
fn branch_names_in(repo: &Path) -> HashSet<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                line.strip_prefix("refs/heads/").or_else(|| {
                    line.strip_prefix("refs/remotes/")
                        .and_then(|rest| rest.split_once('/'))
                        .map(|(_, branch)| branch)
                })
            })
            .map(str::to_string)
            .collect(),
        Ok(out) => {
            tracing::debug!(status = %out.status, "branches not listed");
            HashSet::new()
        }
        Err(e) => {
            tracing::debug!(error = %e, "branches not listed");
            HashSet::new()
        }
    }
}

/// Collapses, expands (`Some`) or toggles (`None`) a section and saves it.
fn toggle_section(app: &mut App, key: &str, collapse: Option<bool>) {
    let collapsed = &mut app.sidebar.collapsed;
    let want = collapse.unwrap_or(!collapsed.contains(key));
    if want {
        collapsed.insert(key.to_string());
    } else {
        collapsed.remove(key);
    }
    app.sidebar.save(&app.storage);
}

/// Focuses session `id` and prefills `/session rename <title>` (`F2`).
pub(crate) fn begin_rename(app: &mut App, id: &str) {
    let title = match app.storage.get_session(id) {
        Ok(row) => row.map(|r| r.title),
        Err(e) => {
            tracing::warn!(error = %e, "could not read session title");
            None
        }
    };
    let text = format!("/session rename {}", title.unwrap_or_default());
    begin_prefill(app, id, &text);
}

/// Focuses session `id`, puts `text` in the input with the cursor at its
/// end, and returns focus to the input. Nothing is prefilled when the
/// session cannot be focused.
pub(crate) fn begin_prefill(app: &mut App, id: &str, text: &str) {
    let result = app.switch_to_session(id);
    let ok = result.is_ok();
    report(app, result);
    if ok {
        prefill_input(app, text);
    }
}

/// Puts `text` in the input with the cursor at its end and returns focus
/// to the input.
pub(crate) fn prefill_input(app: &mut App, text: &str) {
    app.session.input_cursor = text.chars().count();
    app.session.input = text.to_string();
    app.sidebar.focused = false;
}

/// Starts a session in `project` and returns focus to the input (`n` on a
/// project heading, and its menu's `New session`).
///
/// # Errors
///
/// Returns a message when the session cannot be started.
pub(crate) fn new_session_in(
    app: &mut App,
    project: codemason_storage::ProjectRow,
) -> Result<(), String> {
    app.open_new_session_in_project(&project.into(), None)?;
    app.sidebar.focused = false;
    Ok(())
}

/// Opens the context menu for the row under the cursor: a session's, the
/// selection's (on a selected session) or a project heading's.
fn open_menu_at_cursor(app: &mut App) {
    let state = &app.sidebar;
    let line = state.cursor.saturating_sub(state.scroll);
    let row_y = state.area.y + 1 + u16::try_from(line).unwrap_or(u16::MAX);
    open_menu_for_row(app, app.sidebar.cursor, row_y);
}

/// Opens the context menu for row `index`, painted at screen row `row_y`.
/// Rows other than sessions and project headings have no menu.
///
/// # Returns
///
/// Whether a menu was opened.
fn open_menu_for_row(app: &mut App, index: usize, row_y: u16) -> bool {
    match build_rows(app).get(index) {
        Some(SidebarRow::Session { id, .. }) => {
            let id = id.clone();
            if app.sidebar.selected.len() > 1 && app.sidebar.selected.contains(&id) {
                crate::session_menu::open_selection(app, row_y);
            } else {
                crate::session_menu::open(app, &id, row_y);
            }
            true
        }
        Some(SidebarRow::Header { key, .. }) if is_project_key(app, key) => {
            let key = key.clone();
            crate::session_menu::open_project(app, &key, row_y);
            true
        }
        Some(
            SidebarRow::Header { .. }
            | SidebarRow::NewSession
            | SidebarRow::Spacer
            | SidebarRow::Search,
        )
        | None => false,
    }
}

/// Whether section `key` is a listed project (not `Pinned`, not a user
/// section).
fn is_project_key(app: &App, key: &str) -> bool {
    app.sidebar.groups.iter().any(|g| g.project_id == key)
}

/// Adds session `id` to the selection, or takes it out.
fn toggle_selected(app: &mut App, id: &str) {
    let selected = &mut app.sidebar.selected;
    if !selected.remove(id) {
        selected.insert(id.to_string());
    }
}

/// The sessions listed under project `project_id`'s heading (not pinned,
/// not in a user section, matching the filter when one is set), newest
/// first. Collapsed sessions are included.
pub(crate) fn project_session_ids(app: &App, project_id: &str) -> Vec<String> {
    let filter = app.sidebar.filter.to_lowercase();
    sections(app)
        .into_iter()
        .find(|section| section.key == project_id)
        .map(|section| {
            section
                .sessions
                .into_iter()
                .filter(|(s, project)| matches_filter(s, project, &filter))
                .map(|(s, _)| s.id.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The selected sessions the sidebar lists: a session hidden by the filter
/// is left out, so a selection action never reaches a session out of sight.
pub(crate) fn selected_ids(app: &App) -> Vec<String> {
    let filter = app.sidebar.filter.to_lowercase();
    sections(app)
        .into_iter()
        .flat_map(|section| section.sessions)
        .filter(|(s, project)| {
            app.sidebar.selected.contains(&s.id) && matches_filter(s, project, &filter)
        })
        .map(|(s, _)| s.id.clone())
        .collect()
}

/// Project ids in display order, and which of them have a heading drawn
/// now (the filter hides projects with no match).
fn project_ids_in_order(app: &App) -> (Vec<String>, HashSet<String>) {
    let shown: HashSet<String> = build_rows(app)
        .into_iter()
        .filter_map(|row| match row {
            SidebarRow::Header { key, .. } => Some(key),
            _ => None,
        })
        .collect();
    let order = sections(app)
        .into_iter()
        .filter(|section| is_project_key(app, &section.key))
        .map(|section| section.key)
        .collect();
    (order, shown)
}

/// The place project `project_id` moves to: the next drawn project above
/// (`up`) or below it, as `(from, to)` indexes into `order`.
fn project_move(
    order: &[String],
    shown: &HashSet<String>,
    project_id: &str,
    up: bool,
) -> Option<(usize, usize)> {
    let at = order.iter().position(|id| id == project_id)?;
    let to = if up {
        order[..at].iter().rposition(|id| shown.contains(id))?
    } else {
        at + 1 + order[at + 1..].iter().position(|id| shown.contains(id))?
    };
    Some((at, to))
}

/// Where project `project_id` can move: `(can move up, can move down)`.
pub(crate) fn project_moves(app: &App, project_id: &str) -> (bool, bool) {
    let (order, shown) = project_ids_in_order(app);
    (
        project_move(&order, &shown, project_id, true).is_some(),
        project_move(&order, &shown, project_id, false).is_some(),
    )
}

/// Moves project `project_id` past the next drawn project up or down and
/// saves the order. The cursor follows the heading.
pub(crate) fn move_project(app: &mut App, project_id: &str, up: bool) {
    let (mut order, shown) = project_ids_in_order(app);
    let Some((at, to)) = project_move(&order, &shown, project_id, up) else {
        return;
    };
    let moved = order.remove(at);
    order.insert(to, moved);
    app.sidebar.project_order = order;
    app.sidebar.save(&app.storage);
    if let Some(index) = build_rows(app)
        .iter()
        .position(|row| matches!(row, SidebarRow::Header { key, .. } if key == project_id))
    {
        app.sidebar.cursor = index;
    }
}

/// Every section key shown in the sidebar.
pub(crate) fn section_keys(app: &App) -> Vec<String> {
    sections(app)
        .into_iter()
        .map(|section| section.key)
        .collect()
}

/// Collapses every section (`true`) or expands every section (`false`).
pub(crate) fn set_all_collapsed(app: &mut App, collapse: bool) {
    if collapse {
        let keys = section_keys(app);
        app.sidebar.collapsed.extend(keys);
    } else {
        app.sidebar.collapsed.clear();
    }
    app.sidebar.save(&app.storage);
}

/// The session id under the cursor, if the cursor is on a session row.
fn cursor_session(app: &App) -> Option<String> {
    match build_rows(app).get(app.sidebar.cursor) {
        Some(SidebarRow::Session { id, .. }) => Some(id.clone()),
        _ => None,
    }
}

/// Handles a key in chat mode.
///
/// # Arguments
///
/// * `app` - the application.
/// * `key` - the key event (after every modal dialog had its chance).
///
/// # Returns
///
/// `true` when the sidebar consumed the key.
pub fn handle_key(app: &mut App, key: KeyInput) -> bool {
    if crate::project_properties::handle_key(app, key) {
        return true;
    }
    if crate::session_menu::handle_key(app, key) {
        return true;
    }
    let ctrl = key.mods.contains(Modifiers::CTRL);
    let alt = key.mods.contains(Modifiers::ALT);
    match key.key {
        Key::Char('s') if alt => {
            toggle(app);
            return true;
        }
        Key::Char('n') if ctrl => {
            let result = app.open_new_session();
            report(app, result);
            return true;
        }
        Key::Up if ctrl => {
            cycle_session(app, false);
            return true;
        }
        Key::Down if ctrl => {
            cycle_session(app, true);
            return true;
        }
        Key::Char('r') if alt => {
            let recent = app
                .background
                .iter()
                .next()
                .and_then(|v| v.session_id.clone());
            match recent {
                Some(id) => {
                    let result = app.switch_to_session(&id);
                    report(app, result);
                }
                None => app.status = "session: no other open session".to_string(),
            }
            return true;
        }
        _ => {}
    }
    if !app.sidebar.focused || !app.sidebar.visible {
        return false;
    }
    // An Alt+letter chord the arms above do not bind (Alt+L, Alt+P, Alt+M,
    // ...) is a global shortcut, not a sidebar letter: it falls through to
    // the input handler, so Alt+P toggles the profile panel instead of
    // pinning the cursor session. Ctrl+Alt is left alone (AltGr).
    if alt && !ctrl && matches!(key.key, Key::Char(_)) {
        return false;
    }
    if app.sidebar.filter_editing {
        match key.key {
            Key::Char(c) if !ctrl && !alt => app.sidebar.filter.push(c),
            Key::Backspace => {
                app.sidebar.filter.pop();
            }
            Key::Enter | Key::Esc => app.sidebar.filter_editing = false,
            _ => {}
        }
        app.sidebar.cursor = 0;
        return true;
    }
    let page = usize::from(app.sidebar.area.height.saturating_sub(2)).max(1);
    let rows = build_rows(app);
    let last = rows.len().saturating_sub(1);
    let state = &mut app.sidebar;
    match key.key {
        Key::Up => state.cursor = selectable(&rows, state.cursor.saturating_sub(1), false),
        Key::Down => state.cursor = selectable(&rows, (state.cursor + 1).min(last), true),
        Key::PageUp => {
            state.cursor = selectable(&rows, state.cursor.saturating_sub(page), false);
        }
        Key::PageDown => {
            state.cursor = selectable(&rows, (state.cursor + page).min(last), true);
        }
        Key::Home => state.cursor = 0,
        Key::End => state.cursor = selectable(&rows, last, false),
        Key::Enter | Key::Char(' ') => activate_row(app, app.sidebar.cursor),
        Key::Left | Key::Right => {
            if let Some(SidebarRow::Header { key: section, .. }) =
                build_rows(app).get(app.sidebar.cursor)
            {
                let section = section.clone();
                toggle_section(app, &section, Some(key.key == Key::Left));
            }
        }
        Key::Char('/') => start_filter(app),
        // On a project heading: `n` starts a session in that project, `b`
        // prefills the command that starts one on its own branch.
        Key::Char('n') => {
            if let Some(project) = cursor_project(app) {
                let result = new_session_in(app, project);
                report(app, result);
            }
        }
        Key::Char('b') => {
            if let Some(project) = cursor_project(app) {
                let text = branch_session_command(app, &project);
                prefill_input(app, &text);
            }
        }
        Key::Char('p') => {
            if let Some(id) = cursor_session(app) {
                let result = app.toggle_session_pin(&id);
                report(app, result);
            }
        }
        Key::Char('a') => {
            if let Some(id) = cursor_session(app) {
                let result = app.archive_session_by_id(&id);
                report(app, result);
            }
        }
        Key::F(2) => {
            if let Some(id) = cursor_session(app) {
                begin_rename(app, &id);
            }
        }
        Key::Char('m') => open_menu_at_cursor(app),
        Key::Char('v') => {
            if let Some(id) = cursor_session(app) {
                toggle_selected(app, &id);
            }
        }
        Key::Char('u') => {
            if let Some(id) = cursor_session(app) {
                let result = app.toggle_session_unread(&id);
                report(app, result);
            }
        }
        Key::Char('f') => {
            if let Some(id) = cursor_session(app) {
                let result = app.fork_session_by_id(&id);
                report(app, result);
                app.sidebar.focused = false;
            }
        }
        Key::Char('D') => {
            if let Some(id) = cursor_session(app) {
                let result = app.request_delete_session(&id);
                report(app, result);
            }
        }
        Key::F(10) if key.mods.contains(Modifiers::SHIFT) => {
            open_menu_at_cursor(app);
        }
        Key::Esc if !state.selected.is_empty() => state.selected.clear(),
        Key::Esc if !state.filter.is_empty() => {
            state.filter.clear();
            state.cursor = 0;
        }
        Key::Esc | Key::Tab => state.focused = false,
        _ => {}
    }
    true
}

/// Acts on a click on row `index` (an index into [`build_rows`]), as a
/// left press on that row does: the sidebar takes the focus, a plain click
/// clears the multi-selection and acts on the row (opens a session, starts
/// the filter, toggles a section), and with `add_to_selection` (`Ctrl`+click)
/// a session row joins or leaves the selection instead. A spacer row, or an
/// index past the last row, only takes the focus.
///
/// Frontends that hit-test rows themselves (the GUI) call this; the
/// terminal's [`handle_mouse`] maps a screen row to `index` and calls it.
///
/// # Arguments
///
/// * `app` - the application.
/// * `index` - the clicked row.
/// * `add_to_selection` - whether the click toggles a session's selection.
pub fn click_row(app: &mut App, index: usize, add_to_selection: bool) {
    app.sidebar.focused = true;
    let rows = build_rows(app);
    // `Ctrl`+click adds the session to the selection or takes it out; a
    // plain click clears the selection.
    if add_to_selection && let Some(SidebarRow::Session { id, .. }) = rows.get(index) {
        let id = id.clone();
        app.sidebar.cursor = index;
        toggle_selected(app, &id);
        return;
    }
    app.sidebar.selected.clear();
    if index < rows.len() && !matches!(rows.get(index), Some(SidebarRow::Spacer)) {
        app.sidebar.cursor = index;
        activate_row(app, index);
    }
}

/// Opens the context menu of row `index` (an index into [`build_rows`]), as
/// a right press on that row does: the session menu on a session row (the
/// selection's menu on a session of a multi-selection), the project menu on
/// a project heading. The sidebar takes the focus, and the cursor moves to
/// the row when a menu opens.
///
/// # Arguments
///
/// * `app` - the application.
/// * `index` - the row whose menu to open.
///
/// # Returns
///
/// Whether a menu was opened; other rows have none.
pub fn open_row_menu(app: &mut App, index: usize) -> bool {
    app.sidebar.focused = true;
    let line = index.saturating_sub(app.sidebar.scroll);
    let row_y = app
        .sidebar
        .area
        .y
        .saturating_add(1)
        .saturating_add(u16::try_from(line).unwrap_or(u16::MAX));
    let opened = open_menu_for_row(app, index, row_y);
    if opened {
        app.sidebar.cursor = index;
    }
    opened
}

/// Handles a mouse event.
///
/// # Arguments
///
/// * `app` - the application.
/// * `event` - the mouse event (after modal overlays had their chance).
///
/// # Returns
///
/// `true` when the sidebar consumed the event.
pub fn handle_mouse(app: &mut App, event: MouseInput) -> bool {
    if crate::project_properties::handle_mouse(app, event) {
        return true;
    }
    if crate::session_menu::handle_mouse(app, event) {
        return true;
    }
    let area = app.sidebar.area;
    if app.sidebar.dragging {
        match event.kind {
            MouseKind::Drag(MouseButton::Left) => {
                app.sidebar.width = (event.column.saturating_sub(area.x) + 1).max(MIN_WIDTH);
            }
            MouseKind::Up(MouseButton::Left) => {
                app.sidebar.dragging = false;
                app.sidebar.save(&app.storage);
            }
            _ => {}
        }
        return true;
    }
    let inside = area.width > 0 && area.contains((event.column, event.row));
    if !inside {
        match event.kind {
            MouseKind::Down(_) => unfocus(app),
            // A drag that ends outside the sidebar drops nothing.
            MouseKind::Up(MouseButton::Left) => app.sidebar.drag_session = None,
            _ => {}
        }
        return false;
    }
    let border_x = area.x + area.width - 1;
    match event.kind {
        MouseKind::Down(MouseButton::Left) if event.column == border_x => {
            app.sidebar.dragging = true;
        }
        MouseKind::Down(MouseButton::Left) => {
            app.sidebar.focused = true;
            let top = area.y + 1;
            if event.row >= top && event.row < area.y + area.height - 1 {
                let index = app.sidebar.scroll + usize::from(event.row - top);
                let ctrl = event.mods.contains(Modifiers::CTRL);
                // A plain press on a session row may start dragging it.
                if !ctrl && let Some(SidebarRow::Session { id, .. }) = build_rows(app).get(index) {
                    app.sidebar.drag_session = Some(id.clone());
                }
                click_row(app, index, ctrl);
            }
        }
        MouseKind::Up(MouseButton::Left) => {
            if let Some(id) = app.sidebar.drag_session.take() {
                drop_session(app, &id, area, event.row);
            }
        }
        MouseKind::Down(MouseButton::Right) => {
            app.sidebar.focused = true;
            let top = area.y + 1;
            if event.row >= top && event.row < area.y + area.height - 1 {
                let index = app.sidebar.scroll + usize::from(event.row - top);
                if open_menu_for_row(app, index, event.row) {
                    app.sidebar.cursor = index;
                }
            }
        }
        MouseKind::ScrollUp => {
            let rows = build_rows(app);
            app.sidebar.scroll = app.sidebar.scroll.saturating_sub(1);
            app.sidebar.cursor = selectable(&rows, app.sidebar.cursor.saturating_sub(1), false);
        }
        MouseKind::ScrollDown => {
            let rows = build_rows(app);
            let last = rows.len().saturating_sub(1);
            app.sidebar.cursor = selectable(&rows, (app.sidebar.cursor + 1).min(last), true);
        }
        _ => {}
    }
    true
}
