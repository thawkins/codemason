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
//! Depends on [`crate::input_event`] (key presses).

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
    },
    ActionSpec {
        id: "help.shortcuts",
        title: "Show Keyboard Shortcuts",
        default_key: None,
        mac_key: None,
        key: None,
    },
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
