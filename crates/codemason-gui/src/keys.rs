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
/// Key presses and repeats convert; releases and other events do not
/// (egui's `Copy` and `Cut` events need the platform and the frame's
/// modifiers, so [`KeyRouter::route`] converts those). Letters are lower case
/// unless Shift is held; Shift+Tab is [`Key::BackTab`]; a key with no neutral
/// equivalent is [`Key::Other`].
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

/// The letters the TUI binds with Alt: the `Key::Char(..)` arms guarded by
/// Alt in `handle_key` in `crates/codemason-app-core/src/input.rs`, plus
/// Alt+S (toggle the sessions sidebar) and Alt+R (return to the recent
/// session) in `sessions_sidebar_state.rs`, which `handle_key` always calls.
/// A test scans every app-core source file and keeps the two in step.
pub const TUI_ALT_LETTERS: &[char] = &[
    'a', 'c', 'e', 'g', 'i', 'l', 'm', 'o', 'p', 'q', 'r', 's', 't', 'v', 'x', 'y',
];

/// Whether `input` is an Alt chord the TUI handles: Alt with one of
/// [`TUI_ALT_LETTERS`], or Alt with Up, Down (teammate focus) or Enter
/// (newline), and neither Ctrl nor Cmd.
pub fn is_tui_alt_chord(input: KeyInput) -> bool {
    if !input.mods.alt || input.mods.ctrl || input.mods.cmd {
        return false;
    }
    match input.key {
        Key::Char(c) => TUI_ALT_LETTERS.contains(&c),
        Key::Up | Key::Down | Key::Enter => true,
        Key::Esc
        | Key::Tab
        | Key::BackTab
        | Key::Backspace
        | Key::Delete
        | Key::Left
        | Key::Right
        | Key::Home
        | Key::End
        | Key::PageUp
        | Key::PageDown
        | Key::F(_)
        | Key::Other => false,
    }
}

/// The TUI key press an egui `Copy` or `Cut` event stands for.
///
/// egui-winit sends these events in place of the key press for Ctrl+C and
/// Ctrl+X, but also for macOS Cmd+C/Cmd+X and PC Shift+Delete, Ctrl+Insert
/// and Ctrl+Shift+Delete. Only a real Ctrl+C or Ctrl+X off macOS converts:
/// the platform is not macOS and the frame holds Ctrl without Shift or Alt.
fn clipboard_key_input(
    event: &egui::Event,
    platform: Platform,
    mods: &egui::Modifiers,
) -> Option<KeyInput> {
    if platform.is_mac() || !mods.ctrl || mods.shift || mods.alt {
        return None;
    }
    match event {
        egui::Event::Copy => Some(chord('c', Modifiers::CTRL)),
        egui::Event::Cut => Some(chord('x', Modifiers::CTRL)),
        _ => None,
    }
}

/// Whether an unbound key press that typed no text reaches the App while
/// the prompt TextEdit has the keyboard. The TextEdit owns editing keys, so
/// only keys it ignores go through: Esc, function keys and page scrolling.
/// TUI Alt chords are handled before this ([`is_tui_alt_chord`]).
fn forwards_while_typing(input: KeyInput) -> bool {
    match input.key {
        Key::Esc | Key::F(_) | Key::PageUp | Key::PageDown => true,
        Key::Char(_)
        | Key::Enter
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

/// Where the keyboard focus is when a frame's keys are routed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The prompt TextEdit does not have the keyboard.
    Elsewhere,
    /// The prompt TextEdit has the keyboard.
    Prompt {
        /// Whether the prompt has a non-empty text selection.
        has_selection: bool,
    },
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
    /// typed, so no widget sees it. Other events stay in `input`, except the
    /// text of a TUI Alt chord forwarded from the prompt (below).
    ///
    /// While the prompt has the keyboard ([`Focus::Prompt`]), typed text is
    /// never sent to the App, so it is never inserted twice. The rules:
    ///
    /// - A TUI Alt chord ([`is_tui_alt_chord`]) is forwarded and the text it
    ///   typed is dropped, so its letter never reaches the prompt (the TUI
    ///   never types these letters). On macOS an Alt (Option) press that
    ///   typed a character is left to the prompt instead, as a terminal does
    ///   unless Option is set to act as Meta.
    /// - Ctrl+C and Ctrl+X (egui `Copy` and `Cut` events off macOS, see
    ///   below) are left to the prompt when it has a non-empty selection, so
    ///   they only copy or cut it, matching the TUI's "copy if a selection is
    ///   active". Without a selection they are forwarded: Ctrl+C arms quit
    ///   and Ctrl+X cancels a running agent, as in the TUI.
    /// - Other presses are forwarded only when they typed no text and the
    ///   prompt ignores them: Esc, function keys, PageUp and PageDown.
    ///
    /// egui `Copy`/`Cut` events convert to Ctrl+C/Ctrl+X only off macOS and
    /// only when the frame holds Ctrl without Shift or Alt; macOS Cmd+C/Cmd+X
    /// and PC Shift+Delete or Ctrl+Shift+Delete never reach the App as Ctrl
    /// chords, wherever the focus is.
    ///
    /// # Arguments
    ///
    /// * `input` - the frame's input, before any widget reads it.
    /// * `focus` - whether the prompt TextEdit has the keyboard, and whether
    ///   it has a selection.
    ///
    /// # Returns
    ///
    /// The key presses for the App.
    pub fn route(&mut self, input: &mut egui::InputState, focus: Focus) -> Vec<KeyInput> {
        let platform = self.keymap.platform();
        let (typing, has_selection) = match focus {
            Focus::Elsewhere => (false, false),
            Focus::Prompt { has_selection } => (true, has_selection),
        };
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
        let mut drop = vec![false; events.len()];
        for (i, event) in events.iter().enumerate() {
            if let Some(key) = clipboard_key_input(event, platform, &input.modifiers) {
                // A prompt selection takes Ctrl+C and Ctrl+X for itself.
                if !(typing && has_selection) {
                    keys.push(key);
                }
                continue;
            }
            let Some(key) = to_key_input(event) else {
                continue;
            };
            let typed_text = matches!(events.get(i + 1), Some(egui::Event::Text(_)));
            if typing && is_tui_alt_chord(key) {
                if typed_text {
                    if platform.is_mac() {
                        continue;
                    }
                    drop[i + 1] = true;
                }
                keys.push(key);
                continue;
            }
            if typing && (typed_text || !forwards_while_typing(key)) {
                continue;
            }
            keys.push(key);
        }
        let mut index = 0;
        input.events.retain(|_| {
            let keep = !drop[index];
            index += 1;
            keep
        });
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
