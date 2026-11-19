//! The command palette: one searchable list of everything the user can run.
//!
//! [`palette_entries`] lists the actions of the shortcut catalog
//! (`action:<id>`), the slash commands (`slash:<trigger>`), the themes
//! (`theme:<name>`), the agents (`agent:<name>`) and the sessions
//! (`session:<id>`). While the palette is open ([`App::palette`]) app-core
//! owns its keys like any dialog: Up and Down move the highlight over
//! [`App::palette_results`] (the entries ranked against the query with
//! [`crate::fuzzy::fuzzy_rank`], at most [`PALETTE_MAX_ROWS`]), Enter runs
//! the highlighted entry and closes the palette, Esc closes it. The
//! frontend edits the query in place and draws the rows.
//!
//! Running an entry: an action runs as its terminal key press, or through
//! [`App::run_action`]; one neither knows is left for the frontend
//! ([`App::take_frontend_action`]). A slash command that takes arguments is
//! put in the prompt for the user to finish; one that takes none runs. A
//! theme, agent or session is switched to.
//!
//! Depends on [`crate::fuzzy`], [`crate::shortcuts`] and the App.

use crate::app::App;
use crate::fuzzy::fuzzy_rank;
use crate::input_event::{Key, KeyInput};
use crate::shortcuts::action_catalog;

/// Most rows the palette shows.
pub const PALETTE_MAX_ROWS: usize = 50;

/// One row of the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    /// What the row runs: `<kind>:<name>`, the kind one of `action`,
    /// `slash`, `theme`, `agent` and `session`.
    pub id: String,
    /// The row's text, which the query is matched against.
    pub title: String,
    /// A second, dimmer text: a description or a project.
    pub hint: Option<String>,
    /// The default key of an action, as text.
    pub key: Option<String>,
}

/// Which entries the palette lists: all of them, or one kind (the agent
/// and session pickers).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PaletteScope {
    /// Every entry: the command palette.
    #[default]
    All,
    /// Only `agent:` entries: the agent picker.
    Agent,
    /// Only `session:` entries: the session picker.
    Session,
}

impl PaletteScope {
    /// The id prefix the scope keeps, or `None` for every entry.
    #[must_use]
    pub const fn prefix(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Agent => Some("agent:"),
            Self::Session => Some("session:"),
        }
    }

    /// The dialog title for the scope.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::All => "Command palette",
            Self::Agent => "Choose agent",
            Self::Session => "Choose session",
        }
    }
}

/// The open palette.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaletteState {
    /// The text typed so far.
    pub query: String,
    /// The highlighted row of [`App::palette_results`].
    pub selected: usize,
    /// Which entries are listed.
    pub scope: PaletteScope,
}

/// Everything the palette lists, in kind order: actions, slash commands,
/// themes, agents, sessions.
#[must_use]
pub fn palette_entries(app: &App) -> Vec<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = action_catalog()
        .iter()
        .map(|action| PaletteEntry {
            id: format!("action:{}", action.id),
            title: action.title.to_string(),
            hint: None,
            key: action.default_key.map(str::to_string),
        })
        .collect();
    entries.extend(crate::app::SLASH_COMMANDS.iter().map(|cmd| PaletteEntry {
        id: format!("slash:{}", cmd.trigger),
        title: format!("/{}", cmd.trigger),
        hint: Some(cmd.description.to_string()),
        key: None,
    }));
    entries.extend(
        std::iter::once("system")
            .chain(app.themes.names())
            .map(|name| PaletteEntry {
                id: format!("theme:{name}"),
                title: format!("Theme: {name}"),
                hint: None,
                key: None,
            }),
    );
    entries.extend(app.cycleable_agents.iter().map(|agent| PaletteEntry {
        id: format!("agent:{}", agent.name),
        title: format!("Agent: {}", agent.name),
        hint: Some(agent.description.clone()).filter(|d| !d.is_empty()),
        key: None,
    }));
    for group in &app.sidebar.groups {
        entries.extend(group.sessions.iter().map(|session| PaletteEntry {
            id: format!("session:{}", session.id),
            title: format!("Session: {}", session.title),
            hint: Some(group.project.name.clone()),
            key: None,
        }));
    }
    entries
}

impl App {
    /// Open the command palette with an empty query.
    pub fn open_palette(&mut self) {
        self.open_palette_scoped(PaletteScope::All);
    }

    /// Open the palette listing only the entries of `scope`: the agent and
    /// session pickers.
    pub fn open_palette_scoped(&mut self, scope: PaletteScope) {
        self.palette = Some(PaletteState {
            scope,
            ..PaletteState::default()
        });
    }

    /// The rows the open palette shows: the entries ranked against its
    /// query, best first, at most [`PALETTE_MAX_ROWS`]; empty while it is
    /// closed.
    #[must_use]
    pub fn palette_results(&self) -> Vec<PaletteEntry> {
        let Some(palette) = self.palette.as_ref() else {
            return Vec::new();
        };
        let mut entries = palette_entries(self);
        if let Some(prefix) = palette.scope.prefix() {
            entries.retain(|e| e.id.starts_with(prefix));
        }
        fuzzy_rank(&palette.query, &entries, |e| e.title.as_str())
            .into_iter()
            .take(PALETTE_MAX_ROWS)
            .cloned()
            .collect()
    }

    /// Set the open palette's query (the frontend's search field), keeping
    /// the highlight on the first row.
    pub fn set_palette_query(&mut self, query: &str) {
        if let Some(palette) = self.palette.as_mut()
            && palette.query != query
        {
            palette.query = query.to_string();
            palette.selected = 0;
        }
    }

    /// The action a palette row asked the frontend to run, if any.
    pub fn take_frontend_action(&mut self) -> Option<String> {
        self.frontend_action.take()
    }

    /// Serve a key while the palette is open: Up/Down move the highlight,
    /// Enter runs the highlighted row and closes the palette, Esc closes it.
    /// Other keys are left to the frontend's search field.
    pub(crate) async fn handle_palette_key(&mut self, key: KeyInput) {
        let count = self.palette_results().len();
        let Some(palette) = self.palette.as_mut() else {
            return;
        };
        match key.key {
            Key::Esc => self.palette = None,
            Key::Up => palette.selected = palette.selected.saturating_sub(1),
            Key::Down if palette.selected + 1 < count => palette.selected += 1,
            Key::Enter => {
                let selected = palette.selected;
                let chosen = self.palette_results().into_iter().nth(selected);
                self.palette = None;
                if let Some(entry) = chosen {
                    Box::pin(self.run_palette_entry(&entry.id)).await;
                }
            }
            _ => {}
        }
    }

    /// Run a slash command chosen in the palette as a typed one runs: queued
    /// for the next turn boundary while a turn runs (refused with a status
    /// message when the queue is full), else at once. Unlike a typed command,
    /// the prompt holds the user's draft, not the command, so the draft is
    /// kept when the command leaves the prompt empty.
    async fn run_palette_slash(&mut self, command: String) {
        let draft = self.session.input.clone();
        let cursor = self.session.input_cursor;
        if self.is_input_blocked() {
            // INTENTIONAL: a full queue sets the status line; the draft is restored below.
            let _ = self.enqueue_input(command, Vec::new());
        } else {
            Box::pin(self.execute_slash_command(&command)).await;
        }
        if self.session.input.is_empty() && !draft.is_empty() {
            self.session.input = draft;
            self.set_cursor_char_index_clamped(cursor);
        }
    }

    /// Run the palette entry `id` (see the module docs).
    pub async fn run_palette_entry(&mut self, id: &str) {
        let Some((kind, name)) = id.split_once(':') else {
            return;
        };
        match kind {
            "action" => match crate::shortcuts::action(name) {
                Some(action) => match action.key {
                    Some(key) => {
                        let _ = Box::pin(self.handle_key(key)).await; // INTENTIONAL: the outcome is the App's own
                    }
                    None if Box::pin(self.run_action(name)).await => {}
                    None => self.frontend_action = Some(name.to_string()),
                },
                None => {}
            },
            "slash" => {
                if self.get_parameter_hint(name).is_some() {
                    // It takes arguments: let the user finish it.
                    self.session.input = format!("/{name} ");
                    let end = self.session.input.chars().count();
                    self.input_edited(end);
                } else {
                    Box::pin(self.run_palette_slash(format!("/{name}"))).await;
                }
            }
            "theme" => {
                if let Some(selection) = self.themes.parse(name) {
                    self.status = format!("theme: {}", selection.display_name());
                    self.theme = selection;
                }
            }
            "agent" => Box::pin(self.run_palette_slash(format!("/agent {name}"))).await,
            "session" => {
                if let Err(err) = self.switch_to_session(name) {
                    self.status = err;
                }
            }
            _ => {}
        }
    }
}
