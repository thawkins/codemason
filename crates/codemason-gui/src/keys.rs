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
    'a', 'c', 'e', 'g', 'i', 'l', 'm', 'o', 'p', 'q', 'r', 's', 't', 'v', 'y',
];

/// Whether `input` is an Alt chord the TUI handles: Alt with one of
/// [`TUI_ALT_LETTERS`], or Alt with Up, Down (teammate focus) or Enter
/// (newline), and neither Ctrl nor Cmd.
pub fn is_tui_alt_chord(input: KeyInput) -> bool {
    if !is_plain_alt(input) {
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

/// Whether `text` is the `Event::Text` that the press `input` typed.
///
/// egui-winit (`State::on_keyboard_input`) emits the press, then a `Text`
/// with winit's `KeyEvent::text` when it is printable and neither Ctrl nor
/// Cmd is held. With Alt (no Ctrl) on Linux and Windows that text is the
/// key's base character, upper case with Shift, so it equals the
/// [`Key::Char`] that [`to_key_input`] reports, ignoring ASCII case.
/// egui-winit drops control characters (`is_printable_char`), so Enter
/// ("\r"), Tab, Backspace, arrows and other named keys never type a `Text`:
/// a `Text` after one of them belongs to another press.
pub(crate) fn is_key_text(input: KeyInput, text: &str) -> bool {
    match input.key {
        Key::Char(c) => {
            let mut buf = [0u8; 4];
            text.eq_ignore_ascii_case(c.encode_utf8(&mut buf))
        }
        Key::Enter
        | Key::Esc
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
        | Key::PageUp
        | Key::PageDown
        | Key::F(_)
        | Key::Other => false,
    }
}

/// The characters a text-carrying event holds: a typed `Text` or an IME
/// commit. Pastes are not typing (the TUI receives them as a paste, never as
/// keys), so they are not included.
fn typed_text_of(event: &egui::Event) -> Option<&str> {
    match event {
        egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
            Some(text.as_str())
        }
        _ => None,
    }
}

/// Whether `input` holds Alt without Ctrl or Cmd.
fn is_plain_alt(input: KeyInput) -> bool {
    input.mods.alt && !input.mods.ctrl && !input.mods.cmd
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

/// Whether `input` is a press the prompt editor handles: a printable key
/// (no Ctrl, Alt or Cmd), or Backspace, Delete, the arrows, Home or End
/// without Alt or Cmd. When one arrives while the prompt is unfocused, the
/// prompt takes the focus so the editor applies it (by grapheme) instead of
/// the App's char-based editing; other keys (Esc, Enter, Tab, function keys,
/// chords) still go to the App.
///
/// # Arguments
///
/// * `input` - the key press.
pub fn is_prompt_edit_key(input: KeyInput) -> bool {
    let mods = input.mods;
    if mods.alt || mods.cmd {
        return false;
    }
    match input.key {
        Key::Char(_) => !mods.ctrl,
        Key::Backspace
        | Key::Delete
        | Key::Left
        | Key::Right
        | Key::Up
        | Key::Down
        | Key::Home
        | Key::End => true,
        Key::Enter
        | Key::Esc
        | Key::Tab
        | Key::BackTab
        | Key::PageUp
        | Key::PageDown
        | Key::F(_)
        | Key::Other => false,
    }
}

/// Whether `input` is a press a focused text field applies itself: a
/// printable key, Backspace, Delete, Left, Right, Home or End, as
/// [`is_prompt_edit_key`] without Up and Down (which move a dialog's list).
fn is_field_edit_key(input: KeyInput) -> bool {
    is_prompt_edit_key(input) && !matches!(input.key, Key::Up | Key::Down)
}

/// Whether `input` is a key the App handles for the focused prompt: it
/// reads or edits the App's input buffer, so it is forwarded and the prompt
/// TextEdit never sees it.
///
/// - Enter with any modifiers: plain (or Ctrl) Enter submits through the
///   App's submit path; Shift+Enter and Alt+Enter insert a newline, as in
///   `input.rs` (`Key::Enter if SHIFT || ALT`).
/// - While a completion menu (`@` mention or `/` command) is open: Up, Down,
///   Tab, Shift+Tab and Esc move, accept or close it, as the TUI's file and
///   slash menus do.
/// - Up without Shift or Alt when [`Focus::Prompt`]'s `up_to_app` is set,
///   and Down likewise with `down_to_app`: the App browses the input history
///   (or moves the Memory panel cursor), as the TUI's `HistoryUp` and
///   `HistoryDown` do on the first and last logical line.
/// - Delete without modifiers when [`Focus::Prompt`]'s `delete_to_app` is
///   set (the Memory panel is shown and the prompt is empty): the App asks
///   to delete the selected memory, as the TUI's `Key::Delete` arm does.
/// - `?` without Ctrl, Alt or Cmd when [`Focus::Prompt`]'s
///   `question_opens_shortcuts` is set (the prompt is empty): the App opens
///   the keyboard shortcuts panel, as the TUI's `Key::Char('?') if
///   input.is_empty()` arm does. The text it typed is dropped with it.
///
/// # Arguments
///
/// * `input` - the key press.
/// * `focus` - where the keyboard is; [`Focus::Elsewhere`] has no prompt
///   keys.
pub fn is_prompt_app_key(input: KeyInput, focus: Focus) -> bool {
    let Focus::Prompt {
        completion_open,
        up_to_app,
        down_to_app,
        delete_to_app,
        question_opens_shortcuts,
        ..
    } = focus
    else {
        return false;
    };
    let plain = !input.mods.shift && !input.mods.alt;
    match input.key {
        Key::Enter => true,
        Key::Char('?') => {
            question_opens_shortcuts && !input.mods.ctrl && !input.mods.alt && !input.mods.cmd
        }
        Key::Up => completion_open || (up_to_app && plain),
        Key::Down => completion_open || (down_to_app && plain),
        Key::Delete => delete_to_app && plain && !input.mods.ctrl && !input.mods.cmd,
        Key::Tab | Key::BackTab | Key::Esc => completion_open,
        Key::Char(_)
        | Key::Backspace
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
    /// No widget has the keyboard and no modal is open: every press goes to
    /// the App and stays in the input for egui.
    Elsewhere,
    /// Another text field than the prompt has the keyboard and no modal is
    /// open. It keeps its text and editing keys, and Ctrl+C/Ctrl+X while it
    /// has a selection, as the prompt does; Esc, function keys,
    /// PageUp/PageDown, the TUI Alt chords and Ctrl+C/Ctrl+X without a
    /// selection still reach the App.
    Widget {
        /// Whether the field has a non-empty text selection.
        has_selection: bool,
    },
    /// An app-core modal (a dialog, a menu, the shortcuts panel, a running
    /// compaction) owns the keyboard, as in the TUI: every press goes to the
    /// App and is removed from egui's input with the text it typed, so no
    /// dialog button is also pressed by Enter or Space, and Tab does not move
    /// egui's focus. The exception is a dialog's own text field (a filter):
    /// while one has the keyboard it keeps its text and editing keys, and
    /// Ctrl+C/Ctrl+X while it has a selection. With no such field focused,
    /// text no press typed (AltGr, dead keys, IME commits) goes to the App
    /// as its characters; pastes stay for the dialog's paste hook.
    Modal {
        /// Whether a dialog's text field (not the prompt) has the keyboard.
        field_focused: bool,
        /// Whether that field has a non-empty text selection.
        field_selection: bool,
    },
    /// The prompt TextEdit has the keyboard.
    Prompt {
        /// Whether the prompt has a non-empty text selection.
        has_selection: bool,
        /// Whether the App has an `@` mention or `/` command completion menu
        /// open (see [`is_prompt_app_key`]).
        completion_open: bool,
        /// Whether Up goes to the App: the caret is on the first logical
        /// line, the history is being browsed, or the Memory panel is shown.
        up_to_app: bool,
        /// Whether Down goes to the App: the caret is on the last logical
        /// line, the history is being browsed, or the Memory panel is shown.
        down_to_app: bool,
        /// Whether Delete goes to the App to ask to delete the selected
        /// memory: the Memory panel is shown and the prompt is empty.
        delete_to_app: bool,
        /// Whether `?` goes to the App to open the shortcuts panel: the
        /// prompt is empty and the panel is enabled.
        question_opens_shortcuts: bool,
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
    /// never sent to the App, so it is never inserted twice. The prompt edits
    /// the App's own input buffer, so the two cannot drift. The rules:
    ///
    /// - A key the App handles for the prompt ([`is_prompt_app_key`]: Enter,
    ///   the completion-menu keys while a menu is open, Up or Down at the
    ///   first or last line for the history, and `?` on an empty prompt) is
    ///   forwarded and removed from `input` with the text it typed, so the
    ///   App submits, inserts the newline, moves the menu or opens the
    ///   shortcuts panel exactly once and the TextEdit does not act on it too.
    /// - A TUI Alt chord ([`is_tui_alt_chord`]) is forwarded and the text it
    ///   typed (`is_key_text`: the next `Text`, equal to the key's
    ///   character ignoring ASCII case) is dropped, so its letter never reaches the prompt (the TUI
    ///   never types these letters). On macOS an Alt (Option) press that
    ///   typed a character is left to the prompt instead, as a terminal does
    ///   unless Option is set to act as Meta.
    /// - Off macOS, any other Alt press without Ctrl that typed text has its
    ///   text dropped and is not forwarded (a `Text` that is not the press's
    ///   own is never dropped): app-core ignores an unbound
    ///   Alt+character, so the prompt does not type it either. Ctrl+Alt
    ///   (AltGr on Windows) and Linux AltGr (reported without Alt) still type.
    /// - Ctrl+C and Ctrl+X (egui `Copy` and `Cut` events off macOS, see
    ///   below) are left to the prompt when it has a non-empty selection, so
    ///   they only copy or cut it, matching the TUI's "copy if a selection is
    ///   active". Without a selection they are forwarded: Ctrl+C arms quit
    ///   and Ctrl+X cancels a running agent, as in the TUI.
    /// - Other presses are forwarded only when they typed no text and the
    ///   prompt ignores them: Esc, function keys, PageUp and PageDown.
    ///
    /// Under [`Focus::Widget`] the same typing rules apply, with no prompt
    /// keys. Under [`Focus::Modal`]
    /// every press is forwarded and removed from `input` with the text it
    /// typed, except the typing and editing keys of a focused dialog text
    /// field, which stay with the field and are not forwarded. With no
    /// dialog field focused, the App's key-driven fields (the `/loop` form,
    /// the properties description, the provider fields) get every typed
    /// character: a `Text` or IME commit that no press of the frame typed is
    /// forwarded as one [`Key::Char`] per character, and a character press
    /// whose own text is another character (AltGr, Option) is not forwarded,
    /// so its text is, once. Pastes stay in `input` for the dialog's own paste
    /// hook.
    ///
    /// egui `Copy`/`Cut` events convert to Ctrl+C/Ctrl+X only off macOS and
    /// only when the frame holds Ctrl without Shift or Alt; macOS Cmd+C/Cmd+X
    /// and PC Shift+Delete or Ctrl+Shift+Delete never reach the App as Ctrl
    /// chords, wherever the focus is.
    ///
    /// # Arguments
    ///
    /// * `input` - the frame's input, before any widget reads it.
    /// * `focus` - where the keyboard is (see [`Focus`]).
    ///
    /// # Returns
    ///
    /// The key presses for the App.
    pub fn route(&mut self, input: &mut egui::InputState, focus: Focus) -> Vec<KeyInput> {
        let platform = self.keymap.platform();
        let (typing, has_selection) = match focus {
            Focus::Elsewhere | Focus::Modal { .. } => (false, false),
            Focus::Prompt { has_selection, .. } | Focus::Widget { has_selection } => {
                (true, has_selection)
            }
        };
        let field_selection = matches!(
            focus,
            Focus::Modal {
                field_selection: true,
                ..
            }
        );
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
                // A selection in the prompt, another text field or a
                // dialog's text field takes Ctrl+C and Ctrl+X for itself.
                let keeps_clipboard = (typing && has_selection) || field_selection;
                if !keeps_clipboard {
                    keys.push(key);
                }
                continue;
            }
            if let Focus::Modal {
                field_focused: false,
                ..
            } = focus
                && !drop[i]
                && let Some(text) = typed_text_of(event)
            {
                // Text no press of this frame typed (a dead key's result, an
                // AltGr or Option character, a letter with no egui key, an
                // IME commit) reaches the App as the characters it holds, as
                // the TUI receives them; the router removes it.
                drop[i] = true;
                keys.extend(text.chars().filter(|c| !c.is_control()).map(|c| KeyInput {
                    key: Key::Char(c),
                    mods: Modifiers::NONE,
                }));
                continue;
            }
            let Some(key) = to_key_input(event) else {
                continue;
            };
            let next_text = match events.get(i + 1) {
                Some(egui::Event::Text(text)) => Some(text.as_str()),
                _ => None,
            };
            // Only the Text this press typed is paired with it; an unrelated
            // Text that follows in the same frame (fast typing, an IME
            // commit) is never dropped and never makes the press "typed".
            let typed_text = next_text.is_some_and(|text| is_key_text(key, text));
            if let Focus::Modal { field_focused, .. } = focus {
                // A dialog's text field keeps its typing and editing keys.
                if field_focused && is_field_edit_key(key) {
                    continue;
                }
                drop[i] = true;
                // A character key whose press typed another character (AltGr
                // or Option composing, a layout where egui reports the
                // physical key) is not forwarded: the text that follows it is,
                // as its own characters. A Ctrl or Cmd chord types nothing
                // (egui-winit sends no Text with Ctrl or Cmd), so a Text after
                // one is another press's and the chord is forwarded. Ctrl+Alt
                // is Windows AltGr, which types, so it is not such a chord.
                let command_chord = key.mods.cmd || (key.mods.ctrl && !key.mods.alt);
                if !field_focused
                    && !command_chord
                    && next_text.is_some()
                    && !typed_text
                    && matches!(key.key, Key::Char(_))
                {
                    continue;
                }
                if typed_text {
                    drop[i + 1] = true;
                }
                keys.push(key);
                continue;
            }
            if is_prompt_app_key(key, focus) {
                drop[i] = true;
                if typed_text {
                    drop[i + 1] = true;
                }
                keys.push(key);
                continue;
            }
            if typing && is_tui_alt_chord(key) {
                // macOS Option composes its own character (e.g. Option+A
                // types an a-ring), so any following Text is left to the
                // prompt there, as before.
                if platform.is_mac() && next_text.is_some() {
                    continue;
                }
                if typed_text {
                    drop[i + 1] = true;
                }
                keys.push(key);
                continue;
            }
            // App-core ignores an unbound Alt+character, so off macOS the
            // prompt does not type it either. It is not forwarded (app-core
            // would drop it). Ctrl+Alt (AltGr on Windows) and Linux AltGr
            // (reported without Alt) still type.
            if typing && typed_text && !platform.is_mac() && is_plain_alt(key) {
                drop[i + 1] = true;
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
