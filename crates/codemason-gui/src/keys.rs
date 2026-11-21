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
//! # Commands
//!
//! The commands are app-core's action catalog
//! ([`codemason_app_core::shortcuts::action_catalog`]): every action has a
//! dotted id, a title and default chords for Linux/Windows and macOS. An
//! action with a terminal key press (`ActionSpec::key`) runs as that press
//! through `App::handle_key`; one without is queued for the shell
//! ([`KeyRouter::take_actions`]), which runs it through `App::run_action` or
//! itself. macOS uses Cmd+Shift with the same letter as the Alt chord,
//! except `queue.menu` (Cmd+Shift+Q logs the user out on macOS) and
//! `app.quit` (Cmd+Q is the macOS quit key).
//!
//! # Overrides
//!
//! The user's changes to these bindings are a [`craft_keymap::Overrides`]
//! stored as JSON at `<config_dir>/codemason/gui-keys.json`, beside the
//! dock layout file ([`crate::dock::layout_path`]), keyed by action id; see
//! [`keys_path`] and [`load_keymap`]. A user binding on a chord another
//! action holds by default takes the chord from it.
//!
//! Dependencies: `craft-keymap` (feature `egui`) for the keymap and the egui
//! dispatcher, `egui` for input events, `serde_json` for the overrides file,
//! `codemason-config` for the config directory, and `tracing` for warnings.

use std::path::{Path, PathBuf};

use codemason_app_core::app::{KeyWindow, window_owns_key};
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::shortcuts::action_catalog;
use craft_keymap::egui::Dispatcher;
use craft_keymap::{Active, Bindable, Binding, CommandRef, Keymap, Overrides, Platform, Scope};

/// File name of the persisted key overrides inside the codemason config
/// directory.
const KEYS_FILE: &str = "gui-keys.json";

/// A key press of `c` with `mods`.
const fn chord(c: char, mods: Modifiers) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods,
    }
}

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
    let catalog = action_catalog()
        .iter()
        .map(|action| {
            let bindable = Bindable::new(action.id, action.title, Scope::app());
            let text = if platform.is_mac() {
                action.mac_key
            } else {
                action.default_key
            };
            match text.map(Binding::parse) {
                Some(Ok(binding)) => bindable.with_default(binding),
                Some(Err(err)) => {
                    // The table is static; a bad entry loses its default only.
                    tracing::warn!(command = action.id, ?text, ?err, "bad default chord");
                    bindable
                }
                None => bindable,
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
/// the catalog does not hold or one with no terminal key press (the shell
/// runs those, see [`KeyRouter::take_actions`]).
pub fn command_key_input(command: &CommandRef) -> Option<KeyInput> {
    action_catalog()
        .iter()
        .find(|a| *command == CommandRef::new(a.id))
        .and_then(|a| a.key)
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

/// Whether `input` holds none of Ctrl, Alt or Cmd (Shift alone is allowed).
fn is_unmodified_by_command(input: KeyInput) -> bool {
    !input.mods.ctrl && !input.mods.alt && !input.mods.cmd
}

/// Whether `input` moves egui's keyboard focus or presses the focused
/// widget: Tab, Enter or Space with no modifier other than Shift.
fn is_widget_key(input: KeyInput) -> bool {
    is_unmodified_by_command(input)
        && matches!(
            input.key,
            Key::Tab | Key::BackTab | Key::Enter | Key::Char(' ')
        )
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
/// - A key the shown Agents or Teams window takes itself
///   (`window_owns_key`: Up/Down, PageUp/PageDown, Space, Delete, Enter,
///   Esc, Shift+K or `k`) when [`Focus::Prompt`]'s `window` is set.
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
        window,
        question_opens_shortcuts,
        ..
    } = focus
    else {
        return false;
    };
    if window.is_some_and(|window| window_owns_key(window, input)) {
        return true;
    }
    let plain = !input.mods.shift && !input.mods.alt;
    match input.key {
        Key::Enter => true,
        Key::Char('?') => question_opens_shortcuts && is_unmodified_by_command(input),
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
        /// Whether the modal is drawn with egui widgets the keyboard drives
        /// (the theme editor): Tab, Shift+Tab, Enter and Space stay in egui's
        /// input to move the focus and press the focused button, and are not
        /// forwarded. Esc and every other key still go to the App.
        widget_keys: bool,
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
        /// The Agents or Teams window that takes its own navigation keys
        /// (`App::key_window`): those keys go to the App, and the prompt keeps
        /// every other key, its typing, IME and pastes, as the TUI's input
        /// editor keeps them while the popup is open (spec `agentui` FR-055).
        window: Option<KeyWindow>,
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
    /// Whether the transcript holds a text selection this frame (see
    /// [`KeyRouter::set_label_selection`]).
    label_selection: bool,
    /// Actions with no terminal key press that ran since the last
    /// [`KeyRouter::take_actions`], by id.
    actions: Vec<String>,
    /// The leader chord, if leader sequences are on.
    leader: Option<(egui::Key, egui::Modifiers)>,
    /// How long the leader waits for the next key, in seconds.
    leader_timeout: f64,
    /// The action each key runs after the leader.
    leader_bindings: std::collections::BTreeMap<char, String>,
    /// When the leader was pressed (egui input time), while it waits.
    leader_since: Option<f64>,
    /// The terminal key presses of catalog actions whose default chord the
    /// keymap no longer binds to them (the user moved it): app-core would
    /// still run the action on that press, so it is not forwarded.
    moved_keys: Vec<KeyInput>,
}

/// The terminal key presses of the catalog actions `keymap` no longer binds
/// to their default chord.
fn moved_action_keys(keymap: &Keymap) -> Vec<KeyInput> {
    let scope = Scope::app();
    action_catalog()
        .iter()
        .filter_map(|action| {
            let key = action.key?;
            let text = if keymap.platform().is_mac() {
                action.mac_key
            } else {
                action.default_key
            }?;
            let default = Binding::parse(text).ok()?.text(&scope);
            let still_bound = keymap
                .bindings(&CommandRef::new(action.id))
                .iter()
                .any(|b| b.text(&scope) == default);
            (!still_bound).then_some(key)
        })
        .collect()
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
            moved_keys: moved_action_keys(&keymap),
            keymap,
            dispatcher: Dispatcher::new(),
            label_selection: false,
            actions: Vec::new(),
            leader: None,
            leader_timeout: 0.0,
            leader_bindings: default_leader_bindings(),
            leader_since: None,
        }
        .with_leader(&codemason_app_core::shortcuts::leader_startup())
    }

    /// This router with the leader key and wait of `settings`. A leader
    /// text that does not parse turns leader sequences off.
    #[must_use]
    pub fn with_leader(mut self, settings: &codemason_app_core::shortcuts::LeaderSettings) -> Self {
        self.leader = settings.key.as_deref().and_then(parse_chord);
        self.leader_timeout = settings.timeout_ms as f64 / 1000.0;
        self
    }

    /// Add or replace the actions keys run after the leader, by key (a
    /// user's `"<leader> x"` bindings, see [`load_leader_bindings`]).
    pub fn bind_after_leader(&mut self, bindings: std::collections::BTreeMap<char, String>) {
        self.leader_bindings.extend(bindings);
    }

    /// Whether the leader was pressed and the router waits for the next key.
    #[must_use]
    pub fn pending_leader(&self) -> bool {
        self.leader_since.is_some()
    }

    /// How long the leader has waited, in seconds, at egui time `now`.
    #[must_use]
    pub fn leader_waited(&self, now: f64) -> Option<f64> {
        self.leader_since.map(|since| now - since)
    }

    /// The leader's wait, in seconds.
    #[must_use]
    pub fn leader_timeout(&self) -> f64 {
        self.leader_timeout
    }

    /// The keys that run an action after the leader, with the action's
    /// title, in key order.
    #[must_use]
    pub fn leader_entries(&self) -> Vec<(char, String)> {
        self.leader_bindings
            .iter()
            .map(|(key, id)| {
                let title = codemason_app_core::shortcuts::action(id)
                    .map_or_else(|| id.clone(), |a| a.title.to_string());
                (*key, title)
            })
            .collect()
    }

    /// Take the leader and the key after it out of `input`: the leader
    /// starts a wait, and the next key press ends it, running the action
    /// bound to that key (if any) and never typing. A wait longer than the
    /// leader's timeout ends with no action. Not while a modal owns the keys.
    fn take_leader_sequences(
        &mut self,
        input: &mut egui::InputState,
        focus: Focus,
        keys: &mut Vec<KeyInput>,
    ) {
        let now = input.time;
        if self
            .leader_since
            .is_some_and(|since| now - since > self.leader_timeout)
        {
            self.leader_since = None;
        }
        let Some((leader_key, leader_mods)) = self.leader else {
            return;
        };
        if matches!(focus, Focus::Modal { .. }) {
            self.leader_since = None;
            return;
        }
        let mut drop_text = false;
        let mut ran: Vec<String> = Vec::new();
        input.events.retain(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                drop_text = false;
                if self.leader_since.take().is_some() {
                    // The key after the leader: its action, if bound.
                    if let Key::Char(c) = neutral_key(*key, modifiers.shift)
                        && let Some(id) = self.leader_bindings.get(&c)
                    {
                        ran.push(id.clone());
                    }
                    drop_text = true;
                    return false;
                }
                if *key == leader_key && same_modifiers(*modifiers, leader_mods) {
                    self.leader_since = Some(now);
                    drop_text = true;
                    return false;
                }
                true
            }
            egui::Event::Text(_) if drop_text || self.leader_since.is_some() => {
                drop_text = false;
                false
            }
            _ => true,
        });
        for id in ran {
            match codemason_app_core::shortcuts::action(&id).and_then(|a| a.key) {
                Some(key) => keys.push(key),
                None => self.actions.push(id),
            }
        }
    }

    /// The key each catalog action is bound to now (its first binding, a
    /// user override included), as text, by action id; unbound actions are
    /// left out.
    #[must_use]
    pub fn action_keys(&self) -> std::collections::BTreeMap<String, String> {
        action_catalog()
            .iter()
            .filter_map(|action| {
                self.keymap
                    .bindings(&CommandRef::new(action.id))
                    .first()
                    .map(|binding| (action.id.to_string(), binding.text(&Scope::app())))
            })
            .collect()
    }

    /// The ids of the actions with no terminal key press that the keymap
    /// ran since the last call, oldest first. The shell runs each through
    /// `App::run_action`, or itself when app-core does not know it.
    pub fn take_actions(&mut self) -> Vec<String> {
        std::mem::take(&mut self.actions)
    }

    /// Say whether selectable text outside any text field (the transcript)
    /// holds a selection this frame. While it does, Ctrl+C and Ctrl+X stay
    /// with egui, which copies that selection, instead of reaching the App
    /// (where Ctrl+C would arm quit). egui places an empty selection where a
    /// press lands on text, so after a click on the transcript Ctrl+C copies
    /// nothing until the selection is cleared (Esc or a click elsewhere).
    pub fn set_label_selection(&mut self, on: bool) {
        self.label_selection = on;
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
    ///   and Ctrl+X cancels a running agent, as in the TUI. A transcript
    ///   selection ([`KeyRouter::set_label_selection`]) keeps them too, under
    ///   any focus.
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
        let mut leader_keys = Vec::new();
        self.take_leader_sequences(input, focus, &mut leader_keys);
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
        let mut keys: Vec<KeyInput> = leader_keys;
        for hit in &hits {
            match command_key_input(&hit.command) {
                Some(key) => keys.push(key),
                // No terminal key press: the shell runs it.
                None => self.actions.push(hit.command.to_string()),
            }
        }
        // Presses after this point are forwarded raw, not run by a binding.
        let raw_from = keys.len();
        let events = &input.events;
        let mut drop = vec![false; events.len()];
        for (i, event) in events.iter().enumerate() {
            if let Some(key) = clipboard_key_input(event, platform, &input.modifiers) {
                // A selection in the prompt, another text field, a dialog's
                // text field or the transcript's text takes Ctrl+C and
                // Ctrl+X for itself.
                let keeps_clipboard =
                    (typing && has_selection) || field_selection || self.label_selection;
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
            if let Focus::Modal {
                field_focused,
                widget_keys,
                ..
            } = focus
            {
                // A dialog's text field keeps its typing and editing keys.
                if field_focused && is_field_edit_key(key) {
                    continue;
                }
                // A keyboard-driven dialog keeps its focus and activation
                // keys, so Save or Reset can be reached and pressed.
                if widget_keys && is_widget_key(key) {
                    if typed_text {
                        drop[i + 1] = true;
                    }
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
        // A moved action's old chord must not reach app-core, which would
        // still run the action on it.
        let raw = keys.split_off(raw_from);
        keys.extend(raw.into_iter().filter(|k| !self.moved_keys.contains(k)));
        keys
    }
}

/// The keys that run an action after the leader by default: each catalog
/// action's `leader_key`.
fn default_leader_bindings() -> std::collections::BTreeMap<char, String> {
    action_catalog()
        .iter()
        .filter_map(|a| a.leader_key.map(|c| (c, a.id.to_string())))
        .collect()
}

/// Whether two sets of modifiers name the same chord: Ctrl, Alt, Shift and
/// macOS Cmd all match.
fn same_modifiers(a: egui::Modifiers, b: egui::Modifiers) -> bool {
    a.ctrl == b.ctrl && a.alt == b.alt && a.shift == b.shift && a.mac_cmd == b.mac_cmd
}

/// Parse a chord written like `Ctrl+G` or `Cmd+Shift+K` into its egui key
/// and modifiers; `None` when it does not parse.
#[must_use]
pub fn parse_chord(text: &str) -> Option<(egui::Key, egui::Modifiers)> {
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let key_name = parts.pop()?;
    let key = egui::Key::from_name(key_name)
        .or_else(|| egui::Key::from_name(&key_name.to_uppercase()))?;
    let mut mods = egui::Modifiers::NONE;
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods.ctrl = true,
            "alt" | "option" => mods.alt = true,
            "shift" => mods.shift = true,
            "cmd" | "command" | "super" | "meta" => mods.mac_cmd = true,
            _ => return None,
        }
    }
    Some((key, mods))
}

/// The prefix of a `gui-keys.json` value that binds a key after the leader.
const LEADER_PREFIX: &str = "<leader>";

/// Read the `"<leader> x"` bindings from the key overrides file at `path`.
///
/// # Returns
///
/// The action bound to each key after the leader, and one warning per entry
/// that names an unknown action or whose text after `<leader>` is not one
/// character. A missing or unreadable file gives none (see
/// [`load_keymap`] for its warnings).
pub fn load_leader_bindings(
    path: &Path,
) -> (std::collections::BTreeMap<char, String>, Vec<String>) {
    let mut bindings = std::collections::BTreeMap::new();
    let mut warnings = Vec::new();
    let Some(entries) = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
    else {
        return (bindings, warnings);
    };
    let shown = path.display();
    for (id, value) in entries {
        for text in leader_texts(&value) {
            let rest: Vec<char> = text.trim().chars().collect();
            match (codemason_app_core::shortcuts::action(&id), rest.as_slice()) {
                (None, _) => warnings.push(format!("{shown}: unknown action `{id}`, ignored")),
                (Some(_), [c]) => {
                    bindings.insert(c.to_ascii_lowercase(), id.clone());
                }
                (Some(_), _) => warnings.push(format!(
                    "{shown}: `<leader>{text}` for `{id}` is not one key after the leader, ignored"
                )),
            }
        }
    }
    (bindings, warnings)
}

/// The texts after `<leader>` in an overrides value (one text or a list).
fn leader_texts(value: &serde_json::Value) -> Vec<String> {
    let texts: Vec<&str> = match value {
        serde_json::Value::String(text) => vec![text.as_str()],
        serde_json::Value::Array(items) => items.iter().filter_map(|v| v.as_str()).collect(),
        _ => Vec::new(),
    };
    texts
        .into_iter()
        .filter_map(|t| t.trim().strip_prefix(LEADER_PREFIX).map(str::to_string))
        .collect()
}

/// `value` without its `"<leader> x"` texts, for craft-keymap's overrides.
fn without_leader_texts(value: serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(text) if text.trim().starts_with(LEADER_PREFIX) => None,
        serde_json::Value::Array(items) => {
            let kept: Vec<serde_json::Value> = items
                .into_iter()
                .filter(|v| {
                    !v.as_str()
                        .is_some_and(|t| t.trim().starts_with(LEADER_PREFIX))
                })
                .collect();
            (!kept.is_empty()).then_some(serde_json::Value::Array(kept))
        }
        other => Some(other),
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
/// A user binding on a chord that another action holds by default takes the
/// chord: the default is removed from that action, so the user's binding is
/// the one that runs.
///
/// # Arguments
///
/// * `platform` - the platform whose default chords to use.
/// * `path` - the JSON overrides file.
///
/// # Returns
///
/// The keymap with the stored overrides applied, or [`default_keymap`]
/// when the file is missing or cannot be read or parsed, and one warning per
/// problem: an unreadable or invalid file, an unknown action id, a key that
/// does not parse or is reserved, and a default a user binding took over
/// (naming both actions). The caller shows them to the user (the Log pane).
pub fn load_keymap(platform: Platform, path: &Path) -> (Keymap, Vec<String>) {
    let keymap = default_keymap(platform);
    let shown = path.display();
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return (keymap, Vec::new()),
        Err(err) => {
            return (
                keymap,
                vec![format!(
                    "{shown}: cannot read, using the default keys: {err}"
                )],
            );
        }
    };
    // `"<leader> x"` texts are the router's (`load_leader_bindings`), not
    // craft-keymap's.
    let text = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Object(entries)) => serde_json::Value::Object(
            entries
                .into_iter()
                .filter_map(|(id, value)| without_leader_texts(value).map(|v| (id, v)))
                .collect(),
        )
        .to_string(),
        Ok(_) | Err(_) => text,
    };
    let overrides = match serde_json::from_str::<Overrides>(&text) {
        Ok(overrides) => overrides,
        Err(err) => {
            return (
                keymap,
                vec![format!("{shown}: invalid, using the default keys: {err}")],
            );
        }
    };
    let mut keymap = keymap.with_overrides(overrides);
    let mut warnings: Vec<String> = keymap
        .problems()
        .into_iter()
        .map(|problem| format!("{shown}: {}", describe_problem(&problem)))
        .collect();
    let users: Vec<CommandRef> = keymap
        .overrides()
        .iter()
        .map(|(key, _)| CommandRef::new(key))
        .collect();
    for user in users {
        for conflict in keymap.conflicts_with(&user) {
            let (other, binding) = &conflict.b;
            let default = keymap.default_bindings(other).contains(binding);
            if conflict.kind != craft_keymap::ConflictKind::Clash || !default {
                continue;
            }
            if keymap.remove(other, binding) {
                warnings.push(format!(
                    "{shown}: {} is now `{user}`; it was `{other}`'s default, which no longer has it",
                    conflict.chord
                ));
            }
        }
    }
    (keymap, warnings)
}

/// One line for a problem in the stored key overrides.
fn describe_problem(problem: &craft_keymap::Problem) -> String {
    use craft_keymap::Problem;
    match problem {
        Problem::UnknownCommand { key } => format!("unknown action `{key}`, ignored"),
        Problem::BadBinding { key, text, error } => {
            format!("`{text}` for `{key}` is not a key ({error:?}), ignored")
        }
        Problem::Reserved { key, text } => {
            format!("`{text}` for `{key}` is reserved by the system, ignored")
        }
        Problem::DuplicateKey { key } => format!("`{key}` is listed twice; the second is ignored"),
        other => format!("{other:?}"),
    }
}
