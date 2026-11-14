//! Project `Properties` dialog, opened from the sidebar's project heading
//! menu (spec SESSIONMENU).
//!
//! Three settings, each falling back to the global configuration when not
//! set: a description shown after the project's name in the sidebar, the
//! execution backend sessions created in the project use, and the provider
//! and model they start on. The dialog edits a copy; `Enter` saves all three
//! through the same [`codemason_agent::project::ProjectManager`] methods as
//! `/project describe`, `/project backend` and `/project model`, and `Esc`
//! discards.
//!
//! Keys: `Tab`/`Shift+Tab` (or `Up`/`Down`) move between fields; in the
//! description the usual editing keys apply; on a choice `Left`/`Right` step
//! through the enabled choices and `Space` opens the full list. A backend
//! that cannot run here (no runtime, no remote configured) is greyed out.

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};

use codemason_agent::project::{MAX_DESCRIPTION_CHARS, parse_model_ref};
use codemason_config::ExecutionBackendKind;

use crate::app::App;

/// Most choices the open list shows at once.
pub(crate) const LIST_ROWS: usize = 10;

/// One entry of a choice field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Text shown for the entry.
    pub label: String,
    /// The stored value; `None` is the `Global (...)` entry.
    pub value: Option<String>,
    /// Whether it can be chosen; a disabled entry is greyed out.
    pub enabled: bool,
}

/// The field that has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The description text.
    Description,
    /// The execution backend choice.
    Backend,
    /// The provider/model choice.
    Model,
}

impl Field {
    /// The next field down (`forward`) or up, wrapping.
    fn step(self, forward: bool) -> Self {
        match (self, forward) {
            (Self::Description, true) | (Self::Model, false) => Self::Backend,
            (Self::Backend, true) | (Self::Description, false) => Self::Model,
            (Self::Model, true) | (Self::Backend, false) => Self::Description,
        }
    }
}

/// The open `Properties` dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProperties {
    /// The project being edited.
    pub project_id: String,
    /// Its name, for the title.
    pub project_name: String,
    /// The description being edited.
    pub description: String,
    /// Caret position in [`Self::description`], in characters.
    pub caret: usize,
    /// Backend choices: `Global (...)` first.
    pub backends: Vec<Choice>,
    /// Index of the chosen backend.
    pub backend: usize,
    /// Model choices: `Global (...)` first.
    pub models: Vec<Choice>,
    /// Index of the chosen model.
    pub model: usize,
    /// The field with the keyboard.
    pub focus: Field,
    /// The highlighted entry of the open choice list, if one is open.
    pub list: Option<usize>,
    /// Why the last save failed, shown at the bottom.
    pub error: Option<String>,
    /// Area painted last frame, for mouse hit-testing.
    pub area: Area,
}

impl ProjectProperties {
    /// The choices and chosen index of the focused field, if it is a choice.
    pub(crate) fn focused_choices(&self) -> Option<(&[Choice], usize)> {
        match self.focus {
            Field::Description => None,
            Field::Backend => Some((&self.backends, self.backend)),
            Field::Model => Some((&self.models, self.model)),
        }
    }

    /// Sets the chosen index of the focused choice field.
    fn choose(&mut self, index: usize) {
        match self.focus {
            Field::Description => {}
            Field::Backend => self.backend = index,
            Field::Model => self.model = index,
        }
    }
}

/// Opens the dialog for project `project_id` with its stored settings.
///
/// # Errors
///
/// Returns a message when the project is not listed in the sidebar.
pub fn open(app: &mut App, project_id: &str) -> Result<(), String> {
    let project = app
        .sidebar
        .groups
        .iter()
        .find(|g| g.project_id == project_id)
        .map(|g| g.project.clone())
        .ok_or_else(|| "that project is no longer listed".to_string())?;
    let (backends, backend) = backend_choices(
        app,
        std::path::Path::new(&project.worktree),
        project.execution_backend.as_deref(),
    );
    let stored_model = match (&project.provider_id, &project.model_id) {
        (Some(provider), Some(model)) => Some(format!("{provider}/{model}")),
        _ => None,
    };
    let (models, model) = model_choices(app, stored_model.as_deref());
    let description = project.description.unwrap_or_default();
    app.sidebar.properties = Some(ProjectProperties {
        project_id: project.id,
        project_name: project.name,
        caret: description.chars().count(),
        description,
        backends,
        backend,
        models,
        model,
        focus: Field::Description,
        list: None,
        error: None,
        area: Area::default(),
    });
    Ok(())
}

/// The backend choices for the project at `dir` (its own configuration
/// decides the global kind and what can run) and the index of `stored`
/// (`Global` when unset).
fn backend_choices(app: &App, dir: &std::path::Path, stored: Option<&str>) -> (Vec<Choice>, usize) {
    let global = app
        .session_processor
        .load_config_for(dir)
        .effective_execution_backend();
    let mut choices = vec![Choice {
        label: format!("Global ({})", global.as_str()),
        value: None,
        enabled: true,
    }];
    choices.extend(
        app.project_backend_choices(dir)
            .into_iter()
            .map(|(kind, why)| Choice {
                label: match &why {
                    Some(why) => format!("{} ({why})", kind.as_str()),
                    None => kind.as_str().to_string(),
                },
                value: Some(kind.as_str().to_string()),
                enabled: why.is_none(),
            }),
    );
    let chosen = stored
        .and_then(ExecutionBackendKind::from_label)
        .and_then(|kind| {
            choices
                .iter()
                .position(|c| c.value.as_deref() == Some(kind.as_str()))
        })
        .unwrap_or(0);
    (choices, chosen)
}

/// The model choices (every model of every configured provider) and the
/// index of `stored` (`Global` when unset). A stored model no provider
/// lists any more is kept as a choice so saving does not drop it.
fn model_choices(app: &App, stored: Option<&str>) -> (Vec<Choice>, usize) {
    let global = match app.storage.get_setting("selected_model") {
        Ok(Some(model)) if !model.is_empty() => model,
        Ok(_) => "none".to_string(),
        Err(e) => {
            tracing::warn!(error = %e, "global model not read");
            "none".to_string()
        }
    };
    let mut choices = vec![Choice {
        label: format!("Global ({global})"),
        value: None,
        enabled: true,
    }];
    for provider in App::get_configured_providers_for_router(&app.storage) {
        choices.extend(app.models_for_provider(&provider.id).into_iter().map(|m| {
            let value = format!("{}/{}", provider.id, m.id);
            Choice {
                label: value.clone(),
                value: Some(value),
                enabled: true,
            }
        }));
    }
    let chosen = match stored {
        None => 0,
        Some(stored) => match choices
            .iter()
            .position(|c| c.value.as_deref() == Some(stored))
        {
            Some(at) => at,
            None => {
                choices.insert(
                    1,
                    Choice {
                        label: stored.to_string(),
                        value: Some(stored.to_string()),
                        enabled: true,
                    },
                );
                1
            }
        },
    };
    (choices, chosen)
}

/// Saves the dialog's settings and closes it; on failure it stays open
/// with the reason.
fn save(app: &mut App) {
    let Some(dialog) = app.sidebar.properties.clone() else {
        return;
    };
    let manager = app.project_manager();
    let id = dialog.project_id.as_str();
    let backend = dialog.backends[dialog.backend]
        .value
        .as_deref()
        .and_then(ExecutionBackendKind::from_label);
    let model = dialog.models[dialog.model].value.clone();
    let result = (|| -> Result<(), String> {
        let model = model
            .as_deref()
            .map(parse_model_ref)
            .transpose()
            .map_err(|e| e.to_string())?;
        manager
            .set_description(id, Some(&dialog.description))
            .map_err(|e| e.to_string())?;
        manager
            .set_backend(id, backend)
            .map_err(|e| e.to_string())?;
        manager
            .set_base_model(id, model.as_ref())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            app.sidebar.properties = None;
            app.sidebar.dirty = true;
            app.status = format!(
                "{}: backend {}, model {}",
                dialog.project_name,
                dialog.backends[dialog.backend].label,
                dialog.models[dialog.model].label
            );
        }
        Err(e) => {
            if let Some(open) = app.sidebar.properties.as_mut() {
                open.error = Some(e);
            }
        }
    }
}

/// Handles a key while the dialog is open.
///
/// # Returns
///
/// `true` when the dialog is open (it consumes every key), `false` otherwise.
pub fn handle_key(app: &mut App, key: KeyInput) -> bool {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return false;
    };
    if let Some(cursor) = dialog.list {
        list_key(dialog, cursor, key);
        return true;
    }
    match key.key {
        Key::Esc => app.sidebar.properties = None,
        Key::Enter => save(app),
        Key::Tab | Key::Down => dialog.focus = dialog.focus.step(true),
        Key::BackTab | Key::Up => dialog.focus = dialog.focus.step(false),
        _ => match dialog.focus {
            Field::Description => edit_key(dialog, key),
            Field::Backend | Field::Model => choice_key(dialog, key),
        },
    }
    true
}

/// A key in the description field.
fn edit_key(dialog: &mut ProjectProperties, key: KeyInput) {
    let len = dialog.description.chars().count();
    let byte = |text: &str, at: usize| text.char_indices().nth(at).map_or(text.len(), |(i, _)| i);
    match key.key {
        Key::Char(c) if !key.mods.contains(Modifiers::CTRL) => {
            if len < MAX_DESCRIPTION_CHARS {
                let at = byte(&dialog.description, dialog.caret);
                dialog.description.insert(at, c);
                dialog.caret += 1;
            }
        }
        Key::Backspace if dialog.caret > 0 => {
            dialog.caret -= 1;
            let at = byte(&dialog.description, dialog.caret);
            dialog.description.remove(at);
        }
        Key::Delete if dialog.caret < len => {
            let at = byte(&dialog.description, dialog.caret);
            dialog.description.remove(at);
        }
        Key::Left => dialog.caret = dialog.caret.saturating_sub(1),
        Key::Right => dialog.caret = (dialog.caret + 1).min(len),
        Key::Home => dialog.caret = 0,
        Key::End => dialog.caret = len,
        _ => {}
    }
}

/// A key on a choice field: `Left`/`Right` step to the previous or next
/// enabled choice, `Space` opens the list.
fn choice_key(dialog: &mut ProjectProperties, key: KeyInput) {
    let Some((choices, chosen)) = dialog.focused_choices() else {
        return;
    };
    let step = |forward: bool| {
        let n = choices.len();
        (1..n)
            .map(|k| {
                if forward {
                    (chosen + k) % n
                } else {
                    (chosen + n - k) % n
                }
            })
            .find(|&i| choices[i].enabled)
    };
    match key.key {
        Key::Left => {
            if let Some(i) = step(false) {
                dialog.choose(i);
            }
        }
        Key::Right => {
            if let Some(i) = step(true) {
                dialog.choose(i);
            }
        }
        Key::Char(' ') => dialog.list = Some(chosen),
        _ => {}
    }
}

/// A key while a choice list is open: move, `Enter` picks an enabled
/// entry, `Esc` closes the list.
fn list_key(dialog: &mut ProjectProperties, cursor: usize, key: KeyInput) {
    let Some((choices, _)) = dialog.focused_choices() else {
        dialog.list = None;
        return;
    };
    let last = choices.len().saturating_sub(1);
    let enabled = choices.get(cursor).is_some_and(|c| c.enabled);
    dialog.list = match key.key {
        Key::Up => Some(cursor.saturating_sub(1)),
        Key::Down => Some((cursor + 1).min(last)),
        Key::PageUp => Some(cursor.saturating_sub(LIST_ROWS)),
        Key::PageDown => Some((cursor + LIST_ROWS).min(last)),
        Key::Home => Some(0),
        Key::End => Some(last),
        Key::Enter if enabled => {
            dialog.choose(cursor);
            None
        }
        Key::Esc => None,
        _ => Some(cursor),
    };
}

/// Handles pasted `text` while the dialog is open: into the description at
/// the caret (line breaks become spaces, cut at the length limit), and
/// ignored on a choice field.
///
/// # Returns
///
/// `true` when the dialog is open (it takes every paste), `false` otherwise.
pub fn handle_paste(app: &mut App, text: &str) -> bool {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return false;
    };
    if dialog.focus == Field::Description && dialog.list.is_none() {
        let room = MAX_DESCRIPTION_CHARS.saturating_sub(dialog.description.chars().count());
        let pasted: String = text
            .chars()
            .filter(|&c| c != '\r')
            .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
            .take(room)
            .collect();
        let at = dialog
            .description
            .char_indices()
            .nth(dialog.caret)
            .map_or(dialog.description.len(), |(i, _)| i);
        dialog.description.insert_str(at, &pasted);
        dialog.caret += pasted.chars().count();
    }
    true
}

/// Handles a mouse event while the dialog is open: it consumes every event,
/// and a click outside it closes it.
///
/// # Returns
///
/// `true` when the dialog is open, `false` otherwise.
pub fn handle_mouse(app: &mut App, event: MouseInput) -> bool {
    let Some(dialog) = app.sidebar.properties.as_ref() else {
        return false;
    };
    let area = dialog.area;
    let inside = event.column >= area.x
        && event.column < area.x + area.width
        && event.row >= area.y
        && event.row < area.y + area.height;
    if matches!(event.kind, MouseKind::Down(MouseButton::Left)) && !inside {
        app.sidebar.properties = None;
    }
    true
}
