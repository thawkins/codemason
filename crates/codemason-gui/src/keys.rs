//! Keyboard shortcuts for the GUI window, built on craft-keymap.
//!
//! The GUI binds a small catalog of commands with
//! [`craft_keymap::Keymap`]. A command that fires is not run here: the router
//! sends the [`App`](codemason_app_core::app::App) the [`KeyInput`] of that
//! command's Linux/Windows chord, so app-core's terminal key handler runs it
//! unchanged and the GUI and the TUI cannot drift. Every other key press
//! falls through to `App::handle_key` as its own [`KeyInput`]
//! ([`to_key_input`]).
//!
//! # Command table
//!
//! | Command         | Linux / Windows | macOS         | TUI source                          |
//! |-----------------|-----------------|---------------|-------------------------------------|
//! | `panel.log`     | Alt+L           | Cmd+Shift+L   | `input.rs`: `Alt+L` -> `ToggleLog`  |
//! | `panel.profile` | Alt+P           | Cmd+Shift+P   | `input.rs`: `Alt+P` -> `ToggleProfile` |
//! | `panel.tasks`   | Alt+I           | Cmd+Shift+I   | `input.rs`: `Alt+I` -> `ToggleTasksPanel` |
//! | `panel.memory`  | Alt+M           | Cmd+Shift+M   | `input.rs`: `Alt+M` -> `ToggleMemory` |
//! | `queue.menu`    | Alt+Q           | Cmd+Alt+Q     | `input.rs`: `Alt+Q` -> `OpenQueueMenu` |
//! | `agent.stop`    | Alt+X           | Cmd+Shift+X   | `input.rs`: `Alt+X` -> stop confirmation |
//! | `app.quit`      | Ctrl+D          | Cmd+Q         | `input.rs`: `Ctrl+D` -> `ConfirmQuit` |
//!
//! `input.rs` is `crates/codemason-app-core/src/input.rs`. macOS uses
//! Cmd+Shift with the same letter, except `queue.menu` (Cmd+Shift+Q logs the
//! user out on macOS) and `app.quit` (Cmd+Q is the macOS quit key).
//!
//! # Overrides
//!
//! The user's changes to these bindings are a [`craft_keymap::Overrides`]
//! stored as JSON at `<config_dir>/codemason/gui-keys.json`, beside the
//! dock layout file ([`crate::dock::layout_path`]); see [`keys_path`] and
//! [`load_keymap`].
//!
//! Dependencies: `craft-keymap` (feature `egui`) for the keymap and the egui
//! dispatcher, `egui` for input events, `serde_json` for the overrides file,
//! `codemason-config` for the config directory, and `tracing` for warnings.

use std::path::{Path, PathBuf};

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use craft_keymap::egui::Dispatcher;
use craft_keymap::{Active, Bindable, Binding, CommandRef, Keymap, Overrides, Platform, Scope};

/// File name of the persisted key overrides inside the codemason config
/// directory.
const KEYS_FILE: &str = "gui-keys.json";

/// One bindable command: its id, label, default chord text per platform,
/// and the TUI key press it stands for.
struct Command {
    id: &'static str,
    label: &'static str,
    pc: &'static str,
    mac: &'static str,
    tui: KeyInput,
}

/// A TUI key press of `c` with `mods`.
const fn chord(c: char, mods: Modifiers) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods,
    }
}

/// The GUI command catalog; see the module docs for where each TUI binding
/// lives.
const COMMANDS: &[Command] = &[
    Command {
        id: "panel.log",
        label: "Toggle Log Panel",
        pc: "Alt+L",
        mac: "Cmd+Shift+L",
        tui: chord('l', Modifiers::ALT),
    },
    Command {
        id: "panel.profile",
        label: "Toggle Profile Panel",
        pc: "Alt+P",
        mac: "Cmd+Shift+P",
        tui: chord('p', Modifiers::ALT),
    },
    Command {
        id: "panel.tasks",
        label: "Toggle Tasks Panel",
        pc: "Alt+I",
        mac: "Cmd+Shift+I",
        tui: chord('i', Modifiers::ALT),
    },
    Command {
        id: "panel.memory",
        label: "Toggle Memory Panel",
        pc: "Alt+M",
        mac: "Cmd+Shift+M",
        tui: chord('m', Modifiers::ALT),
    },
    Command {
        id: "queue.menu",
        label: "Queue Menu",
        pc: "Alt+Q",
        mac: "Cmd+Alt+Q",
        tui: chord('q', Modifiers::ALT),
    },
    Command {
        id: "agent.stop",
        label: "Stop Agent",
        pc: "Alt+X",
        mac: "Cmd+Shift+X",
        tui: chord('x', Modifiers::ALT),
    },
    Command {
        id: "app.quit",
        label: "Quit",
        pc: "Ctrl+D",
        mac: "Cmd+Q",
        tui: chord('d', Modifiers::CTRL),
    },
];

/// Build the keymap with the default bindings for `platform`.
///
/// # Arguments
///
/// * `platform` - the platform whose default chords to use: Alt chords on
///   Linux and Windows, Cmd chords on macOS.
///
/// # Returns
///
/// A keymap over the command catalog with no user overrides.
pub fn default_keymap(platform: Platform) -> Keymap {
    let catalog = COMMANDS
        .iter()
        .map(|command| {
            let text = if platform.is_mac() {
                command.mac
            } else {
                command.pc
            };
            let bindable = Bindable::new(command.id, command.label, Scope::app());
            match Binding::parse(text) {
                Ok(binding) => bindable.with_default(binding),
                Err(err) => {
                    // The table is static; a bad entry loses its default only.
                    tracing::warn!(command = command.id, text, ?err, "bad default chord");
                    bindable
                }
            }
        })
        .collect();
    Keymap::new(catalog, craft_keymap::Scopes::new(), platform)
}

/// The TUI key press a GUI command stands for: its Linux/Windows chord.
///
/// # Arguments
///
/// * `command` - a command from [`default_keymap`]'s catalog.
///
/// # Returns
///
/// The [`KeyInput`] to send to `App::handle_key`, or `None` for a command
/// the catalog does not hold.
pub fn command_key_input(command: &CommandRef) -> Option<KeyInput> {
    COMMANDS
        .iter()
        .find(|c| *command == CommandRef::new(c.id))
        .map(|c| c.tui)
}

/// Convert an egui event into the frontend-neutral [`KeyInput`].
///
/// Key presses and repeats convert; releases and other events do not. egui's
/// `Copy` and `Cut` events, which egui-winit sends in place of the Ctrl+C
/// and Ctrl+X presses, convert to those presses so the TUI's Ctrl+C (arm
/// quit) and Ctrl+X (cancel the agent) still reach the App. Letters are
/// lower case unless Shift is held; Shift+Tab is [`Key::BackTab`]; a key
/// with no neutral equivalent is [`Key::Other`].
///
/// # Arguments
///
/// * `ev` - an event from egui's input.
///
/// # Returns
///
/// The key press, or `None` when `ev` is not a key press.
pub fn to_key_input(ev: &egui::Event) -> Option<KeyInput> {
    match ev {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let mods = Modifiers {
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
                cmd: modifiers.mac_cmd,
            };
            Some(KeyInput {
                key: neutral_key(*key, mods.shift),
                mods,
            })
        }
        egui::Event::Copy => Some(chord('c', Modifiers::CTRL)),
        egui::Event::Cut => Some(chord('x', Modifiers::CTRL)),
        _ => None,
    }
}

/// The neutral key for an egui key, given whether Shift is held.
fn neutral_key(key: egui::Key, shift: bool) -> Key {
    match key {
        egui::Key::Escape => Key::Esc,
        egui::Key::Enter => Key::Enter,
        egui::Key::Tab if shift => Key::BackTab,
        egui::Key::Tab => Key::Tab,
        egui::Key::Backspace => Key::Backspace,
        egui::Key::Delete => Key::Delete,
        egui::Key::ArrowUp => Key::Up,
        egui::Key::ArrowDown => Key::Down,
        egui::Key::ArrowLeft => Key::Left,
        egui::Key::ArrowRight => Key::Right,
        egui::Key::Home => Key::Home,
        egui::Key::End => Key::End,
        egui::Key::PageUp => Key::PageUp,
        egui::Key::PageDown => Key::PageDown,
        egui::Key::Space => Key::Char(' '),
        egui::Key::Minus => Key::Char('-'),
        egui::Key::Quote => Key::Char('\''),
        other => named_key(other, shift),
    }
}

/// Letters, digits, ASCII symbols and function keys, read from egui's key
/// names; anything else is [`Key::Other`].
fn named_key(key: egui::Key, shift: bool) -> Key {
    let mut chars = key.symbol_or_name().chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_graphic()
    {
        return Key::Char(if shift { c } else { c.to_ascii_lowercase() });
    }
    key.name()
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .map_or(Key::Other, Key::F)
}

/// Whether an unbound key press reaches the App while the prompt TextEdit
/// has the keyboard. The TextEdit owns typed text and editing keys, so only
/// keys it ignores go through: Esc, function keys, page scrolling, and the
/// TUI's Alt+letter commands when they typed no text.
fn forwards_while_typing(input: KeyInput, typed_text: bool) -> bool {
    if typed_text {
        return false;
    }
    match input.key {
        Key::Esc | Key::F(_) | Key::PageUp | Key::PageDown => true,
        Key::Char(_) => input.mods.alt && !input.mods.ctrl && !input.mods.cmd,
        Key::Enter
        | Key::Tab
        | Key::BackTab
        | Key::Backspace
        | Key::Delete
        | Key::Up
        | Key::Down
        | Key::Left
        | Key::Right
        | Key::Home
        | Key::End
        | Key::Other => false,
    }
}

/// Routes a frame's key events: keymap commands first, then unbound keys.
#[derive(Debug)]
pub struct KeyRouter {
    keymap: Keymap,
    dispatcher: Dispatcher,
}

impl KeyRouter {
    /// Create a router over `keymap`.
    ///
    /// # Arguments
    ///
    /// * `keymap` - the bindings to dispatch, e.g. from [`load_keymap`].
    ///
    /// # Returns
    ///
    /// A router with a fresh craft-keymap dispatcher.
    #[must_use]
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            dispatcher: Dispatcher::new(),
        }
    }

    /// Turn this frame's key events into the key presses to send to
    /// `App::handle_key`, in order: first the commands the keymap ran (as
    /// their TUI chords, see [`command_key_input`]), then the unbound
    /// presses ([`to_key_input`]).
    ///
    /// A press that runs a command is removed from `input`, with any text it
    /// typed, so no widget sees it. Unbound events stay in `input`. While
    /// `typing`, an unbound press is forwarded only when the prompt TextEdit
    /// ignores it (Esc, function keys, PageUp/PageDown, an Alt+letter that
    /// typed no text), so typed text is never inserted twice.
    ///
    /// # Arguments
    ///
    /// * `input` - the frame's input, before any widget reads it.
    /// * `typing` - whether the prompt TextEdit has keyboard focus.
    ///
    /// # Returns
    ///
    /// The key presses for the App.
    pub fn route(&mut self, input: &mut egui::InputState, typing: bool) -> Vec<KeyInput> {
        let active = Active {
            typing,
            ..Active::NONE
        };
        let hits = self.dispatcher.dispatch(input, &self.keymap, &active);
        let mut keys: Vec<KeyInput> = hits
            .iter()
            .filter_map(|hit| command_key_input(&hit.command))
            .collect();
        let events = &input.events;
        for (i, event) in events.iter().enumerate() {
            let Some(key) = to_key_input(event) else {
                continue;
            };
            let typed_text = matches!(events.get(i + 1), Some(egui::Event::Text(_)));
            if typing && !forwards_while_typing(key, typed_text) {
                continue;
            }
            keys.push(key);
        }
        keys
    }
}

/// Resolve where the key overrides are persisted:
/// `<config_dir>/codemason/gui-keys.json`, beside the dock layout.
///
/// # Returns
///
/// `None` when the platform config directory cannot be determined.
pub fn keys_path() -> Option<PathBuf> {
    codemason_config::config::Config::global_config_dir().map(|dir| dir.join(KEYS_FILE))
}

/// Build the keymap for `platform` with the user overrides stored at
/// `path`.
///
/// # Arguments
///
/// * `platform` - the platform whose default chords to use.
/// * `path` - the JSON overrides file.
///
/// # Returns
///
/// The keymap with the stored overrides applied, or [`default_keymap`]
/// when the file is missing or cannot be read or parsed. An unreadable or
/// invalid file, and override entries the keymap cannot use, are reported
/// with `tracing::warn!`.
pub fn load_keymap(platform: Platform, path: &Path) -> Keymap {
    let keymap = default_keymap(platform);
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return keymap,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "cannot read GUI keys; using defaults");
            return keymap;
        }
    };
    match serde_json::from_str::<Overrides>(&text) {
        Ok(overrides) => {
            let keymap = keymap.with_overrides(overrides);
            let problems = keymap.problems();
            if !problems.is_empty() {
                tracing::warn!(path = %path.display(), ?problems, "GUI key overrides ignored");
            }
            keymap
        }
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "invalid GUI keys; using defaults");
            keymap
        }
    }
}
