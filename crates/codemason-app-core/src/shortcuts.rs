//! The keyboard shortcuts listed in the `?` help panel, and the catalog of
//! rebindable actions.
//!
//! [`KEYBINDINGS`] is the frontend-neutral source of the panel: the terminal
//! UI draws it as a table and the GUI as its "Keyboard shortcuts" dialog, so
//! the two lists cannot drift.
//!
//! [`action_catalog`] lists every action a frontend may bind to a key: its
//! dotted id (the key of the user's overrides), its title and default key
//! chords, and either the terminal key press that runs it through
//! `App::handle_key` or nothing, for an action `App::run_action` or the
//! frontend runs itself.
//!
//! [`LeaderSettings`] is the leader key a frontend's key sequences start
//! with (`<leader> m`), from the `leader` and `leader_timeout_ms` config
//! keys.
//!
//! Depends on [`crate::input_event`] (key presses) and `codemason-config`.

use crate::input_event::{Key, KeyInput, Modifiers};

/// One action a frontend may bind to a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    /// The action's dotted, lowercase id, e.g. `session.new`.
    pub id: &'static str,
    /// Its title, for menus and the command palette.
    pub title: &'static str,
    /// Its default chord on Linux and Windows, if it has one.
    pub default_key: Option<&'static str>,
    /// Its default chord on macOS, if it has one.
    pub mac_key: Option<&'static str>,
    /// The terminal key press that runs it through `App::handle_key`, or
    /// `None` when `App::run_action` or the frontend runs it.
    pub key: Option<KeyInput>,
    /// The key that runs it after the leader key (`<leader> m`), if any.
    pub leader_key: Option<char>,
}

/// A key press of `c` with `mods`.
const fn chord(c: char, mods: Modifiers) -> Option<KeyInput> {
    Some(KeyInput {
        key: Key::Char(c),
        mods,
    })
}

/// An action run by a terminal key press.
const fn keyed(
    id: &'static str,
    title: &'static str,
    pc: &'static str,
    mac: &'static str,
    key: Option<KeyInput>,
) -> ActionSpec {
    ActionSpec {
        id,
        title,
        default_key: Some(pc),
        mac_key: Some(mac),
        key,
        leader_key: None,
    }
}

/// An action with no default chord and no terminal key press, run by
/// `App::run_action`, optionally after the leader key.
const fn keyless(id: &'static str, title: &'static str, leader_key: Option<char>) -> ActionSpec {
    ActionSpec {
        id,
        title,
        default_key: None,
        mac_key: None,
        key: None,
        leader_key,
    }
}

/// The actions; see [`action_catalog`].
const ACTIONS: &[ActionSpec] = &[
    keyed(
        "panel.log",
        "Toggle Log Panel",
        "Alt+L",
        "Cmd+Shift+L",
        chord('l', Modifiers::ALT),
    ),
    keyed(
        "panel.profile",
        "Toggle Profile Panel",
        "Alt+P",
        "Cmd+Shift+P",
        chord('p', Modifiers::ALT),
    ),
    keyed(
        "panel.tasks",
        "Toggle Tasks Panel",
        "Alt+I",
        "Cmd+Shift+I",
        chord('i', Modifiers::ALT),
    ),
    keyed(
        "panel.memory",
        "Toggle Memory Panel",
        "Alt+M",
        "Cmd+Shift+M",
        chord('m', Modifiers::ALT),
    ),
    keyed(
        "panel.context",
        "Toggle Context Panel",
        "Alt+C",
        "Cmd+Shift+C",
        chord('c', Modifiers::ALT),
    ),
    keyed(
        "panel.sessions",
        "Toggle Sessions Panel",
        "Alt+S",
        "Cmd+Shift+S",
        chord('s', Modifiers::ALT),
    ),
    keyed(
        "panel.agents",
        "Toggle Agents Window",
        "Alt+A",
        "Cmd+Shift+A",
        chord('a', Modifiers::ALT),
    ),
    keyed(
        "panel.teams",
        "Toggle Teams Window",
        "Alt+T",
        "Cmd+Shift+T",
        chord('t', Modifiers::ALT),
    ),
    keyed(
        "queue.menu",
        "Queue Menu",
        "Alt+Q",
        "Cmd+Alt+Q",
        chord('q', Modifiers::ALT),
    ),
    keyed(
        "mode.autopilot",
        "Toggle Autopilot",
        "Alt+O",
        "Cmd+Shift+O",
        chord('o', Modifiers::ALT),
    ),
    keyed(
        "mode.yolo",
        "Toggle YOLO Mode",
        "Alt+Y",
        "Cmd+Shift+Y",
        chord('y', Modifiers::ALT),
    ),
    keyed(
        "mode.edit_log",
        "Toggle Edit Log",
        "Alt+E",
        "Cmd+Shift+E",
        chord('e', Modifiers::ALT),
    ),
    keyed(
        "mode.gcf",
        "Toggle GCF Encoding",
        "Alt+G",
        "Cmd+Shift+G",
        chord('g', Modifiers::ALT),
    ),
    keyed(
        "attach.image",
        "Paste Image as Attachment",
        "Alt+V",
        "Cmd+Shift+V",
        chord('v', Modifiers::ALT),
    ),
    keyed(
        "session.new",
        "New Session",
        "Ctrl+N",
        "Cmd+N",
        chord('n', Modifiers::CTRL),
    ),
    keyed(
        "teammate.next",
        "Focus Next Teammate",
        "Alt+Down",
        "Cmd+Alt+Down",
        Some(KeyInput {
            key: Key::Down,
            mods: Modifiers::ALT,
        }),
    ),
    keyed(
        "teammate.prev",
        "Focus Previous Teammate",
        "Alt+Up",
        "Cmd+Alt+Up",
        Some(KeyInput {
            key: Key::Up,
            mods: Modifiers::ALT,
        }),
    ),
    keyed(
        "app.quit",
        "Quit",
        "Ctrl+D",
        "Cmd+Q",
        chord('d', Modifiers::CTRL),
    ),
    ActionSpec {
        id: "palette.show",
        title: "Command Palette",
        default_key: Some("Ctrl+P"),
        mac_key: Some("Cmd+P"),
        key: None,
        leader_key: None,
    },
    keyless("picker.model", "Choose Model", Some('m')),
    keyless("picker.agent", "Choose Agent", Some('a')),
    keyless("picker.session", "Choose Session", Some('s')),
    ActionSpec {
        id: "model.cycle_recent",
        title: "Cycle Recent Models",
        default_key: Some("F2"),
        mac_key: Some("F2"),
        key: None,
        leader_key: None,
    },
    keyless("session.undo", "Undo Last Turn", Some('u')),
    keyless("session.redo", "Redo Turn", Some('r')),
    keyless("prompt.editor", "Edit Prompt in External Editor", Some('e')),
    keyless("help.shortcuts", "Show Keyboard Shortcuts", None),
];

/// Every action a frontend may bind to a key, in display order.
///
/// Ids are unique, dotted and lowercase; they are the keys of the user's
/// key overrides, so an id never changes once released.
#[must_use]
pub fn action_catalog() -> &'static [ActionSpec] {
    ACTIONS
}

/// The action with `id`, if the catalog has one.
#[must_use]
pub fn action(id: &str) -> Option<&'static ActionSpec> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// All documented keybindings, in display order: (keys column, description
/// column). The terminal UI and the GUI both draw their `?` panel from this
/// one list.
pub const KEYBINDINGS: &[(&str, &str)] = &[
    // -- Typing ----------------------------------------------------------
    ("@", "Mention a file - opens file picker"),
    ("/", "Slash command - opens command menu"),
    ("?", "Show this keybindings help panel"),
    (
        "Shift+Enter / Alt+Enter",
        "Insert a newline (multiline input)",
    ),
    ("Left/Right", "Move cursor within the input line"),
    ("Shift+Left/Right", "Extend/shrink keyboard selection"),
    ("Ctrl+Left/Right", "Move cursor by word"),
    ("Ctrl+Shift+Left/Right", "Extend/shrink selection by word"),
    ("Home/End", "Jump to start/end of input"),
    ("Ctrl+Home/End", "Jump to input start/end"),
    ("Ctrl+E", "Jump to end of input (terminal style)"),
    ("Ctrl+B / Ctrl+F", "Move cursor left/right (terminal style)"),
    ("Ctrl+A", "Select all input text"),
    ("Ctrl+C", "Copy selection (or quit if no selection)"),
    ("Ctrl+X", "Cut selection to clipboard"),
    ("Ctrl+V", "Paste text from clipboard"),
    ("Delete", "Delete character under cursor"),
    ("Ctrl+W", "Delete previous word"),
    ("Ctrl+K", "Delete to end of line"),
    ("Alt+V", "Paste image from clipboard as attachment"),
    ("Alt+L", "Toggle log panel visibility"),
    ("Alt+P", "Toggle profiler panel visibility"),
    ("Alt+I", "Toggle Tasks panel visibility"),
    ("Alt+O", "Toggle autopilot mode (autonomous operation)"),
    ("Alt+C", "Toggle context panel visibility"),
    ("Alt+Y", "Toggle YOLO mode (bypass safety checks)"),
    ("Alt+E", "Toggle edit-log (per-edit operation logging)"),
    ("Alt+G", "Toggle GCF tool-result encoding"),
    (
        "Alt+A",
        "Toggle the Agents window (same toggle as the Agents button)",
    ),
    (
        "Alt+T",
        "Toggle the Teams window (same toggle as the Teams button)",
    ),
    // -- Sending ---------------------------------------------------------
    ("Enter", "Send message / confirm"),
    ("Ctrl+C, Ctrl+D", "Quit application (guarded sequence)"),
    // -- Navigation ------------------------------------------------------
    ("Shift+^ / PageUp", "Scroll messages up"),
    ("Shift+v / PageDown", "Scroll messages down"),
    ("^ / v", "Browse input history"),
    ("Ctrl+PageUp", "Scroll log panel up"),
    ("Ctrl+PageDown", "Scroll log panel down"),
    ("PageUp / PageDown", "Scroll opened output overlay"),
    ("Ctrl+PageUp/PageDown", "Output overlay: jump start/end"),
    // -- Agent ------------------------------------------------------------
    ("Tab", "Cycle to next agent"),
    ("Esc / Ctrl+X", "Cancel running agent (while processing)"),
    // -- Teams ------------------------------------------------------------
    ("Alt+v", "Focus next teammate"),
    ("Alt+^", "Focus previous teammate (or clear focus)"),
    // -- Dialogs ----------------------------------------------------------
    ("Esc", "Close any open dialog or menu"),
    ("y / a / n", "Allow / Always / Deny permission request"),
];

/// The default leader key.
pub const DEFAULT_LEADER: &str = "Ctrl+G";

/// The default wait for the key after the leader, in milliseconds.
pub const DEFAULT_LEADER_TIMEOUT_MS: u64 = 2_000;

/// The leader key settings: the key (`None` when leader sequences are off)
/// and how long to wait for the next key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderSettings {
    /// The leader chord as text, e.g. `"Ctrl+G"`; `None` turns it off.
    pub key: Option<String>,
    /// How long to wait for the key after the leader, in milliseconds.
    pub timeout_ms: u64,
}

impl Default for LeaderSettings {
    fn default() -> Self {
        Self {
            key: Some(DEFAULT_LEADER.to_string()),
            timeout_ms: DEFAULT_LEADER_TIMEOUT_MS,
        }
    }
}

impl LeaderSettings {
    /// The settings the `leader` and `leader_timeout_ms` config keys ask for:
    /// unset means the defaults, `"none"` (or an empty text) turns the leader
    /// off.
    #[must_use]
    pub fn from_config(config: &codemason_config::Config) -> Self {
        let key = match config.leader.as_deref().map(str::trim) {
            None => Some(DEFAULT_LEADER.to_string()),
            Some(text) if text.is_empty() || text.eq_ignore_ascii_case("none") => None,
            Some(text) => Some(text.to_string()),
        };
        Self {
            key,
            timeout_ms: config
                .leader_timeout_ms
                .unwrap_or(DEFAULT_LEADER_TIMEOUT_MS),
        }
    }
}

/// The leader settings the launch configuration asks for, set once by the
/// binary.
static LEADER: std::sync::OnceLock<LeaderSettings> = std::sync::OnceLock::new();

/// Record the launch configuration's leader settings. The first call wins.
pub fn set_leader_startup(settings: LeaderSettings) {
    // INTENTIONAL: a second call keeps the first settings.
    let _ = LEADER.set(settings);
}

/// The leader settings the window starts with: the ones given to
/// [`set_leader_startup`], else the defaults.
#[must_use]
pub fn leader_startup() -> LeaderSettings {
    LEADER.get().cloned().unwrap_or_default()
}
