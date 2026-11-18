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

/// The open palette.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaletteState {
    /// The text typed so far.
    pub query: String,
    /// The highlighted row of [`App::palette_results`].
    pub selected: usize,
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
        self.palette = Some(PaletteState::default());
        self.needs_redraw = true;
    }

    /// The rows the open palette shows: the entries ranked against its
    /// query, best first, at most [`PALETTE_MAX_ROWS`]; empty while it is
    /// closed.
    #[must_use]
    pub fn palette_results(&self) -> Vec<PaletteEntry> {
        let Some(palette) = self.palette.as_ref() else {
            return Vec::new();
        };
        let entries = palette_entries(self);
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
        self.needs_redraw = true;
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
                    Box::pin(self.execute_slash_command(&format!("/{name}"))).await;
                }
            }
            "theme" => {
                if let Some(selection) = self.themes.parse(name) {
                    self.status = format!("theme: {}", selection.display_name());
                    self.theme = selection;
                }
            }
            "agent" => Box::pin(self.execute_slash_command(&format!("/agent {name}"))).await,
            "session" => {
                if let Err(err) = self.switch_to_session(name) {
                    self.status = err;
                }
            }
            _ => {}
        }
        self.needs_redraw = true;
    }
}
