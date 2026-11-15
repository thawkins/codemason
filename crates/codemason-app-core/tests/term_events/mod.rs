//! Terminal-style key and mouse event builders for the app-core tests.
//!
//! Many app-core tests were written against the terminal frontend, which
//! spoke crossterm's event types. They exercise app-core's frontend-neutral
//! handlers ([`App::handle_key`], [`App::handle_mouse`],
//! [`codemason_app_core::input::handle_key`]), so this module keeps their
//! key-building vocabulary (`KeyEvent::new(KeyCode::Char('x'),
//! KeyModifiers::CONTROL)`, `MouseEvent { kind: MouseEventKind::Down(..), .. }`)
//! without a terminal dependency, and converts each event to the neutral
//! [`KeyInput`] / [`MouseInput`] exactly as the terminal loop did: a key
//! release is dropped, an unnamed key becomes [`Key::Other`], `SUPER` maps to
//! `cmd`, and horizontal scroll has no neutral equivalent.
//!
//! Depends only on `codemason-app-core`.

#![allow(dead_code)]

use codemason_app_core::app::App;
use codemason_app_core::input::InputAction;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton as NeutralButton, MouseInput, MouseKind,
};

/// A key code, as a terminal reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A printable character.
    Char(char),
    /// Enter / Return.
    Enter,
    /// Escape.
    Esc,
    /// Tab.
    Tab,
    /// Shift+Tab.
    BackTab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Function key (F1 = `F(1)`).
    F(u8),
    /// Insert, which no binding names.
    Insert,
    /// Caps Lock, which no binding names.
    CapsLock,
    /// A null key, which no binding names.
    Null,
}

/// Modifier flags, as a terminal reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// No modifier.
    pub const NONE: Self = Self(0);
    /// Shift.
    pub const SHIFT: Self = Self(1);
    /// Control.
    pub const CONTROL: Self = Self(2);
    /// Alt / Option.
    pub const ALT: Self = Self(4);
    /// Super / Command.
    pub const SUPER: Self = Self(8);

    /// No modifier held.
    #[must_use]
    pub const fn empty() -> Self {
        Self::NONE
    }

    /// Whether every flag of `other` is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Press, repeat or release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    /// Key pressed.
    Press,
    /// Key auto-repeated.
    Repeat,
    /// Key released (never handled by the app).
    Release,
}

/// Extra key state flags; carried but ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEventState(u8);

impl KeyEventState {
    /// No extra state.
    pub const NONE: Self = Self(0);
}

/// A key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// The key.
    pub code: KeyCode,
    /// The held modifiers.
    pub modifiers: KeyModifiers,
    /// Press, repeat or release.
    pub kind: KeyEventKind,
    /// Extra state flags.
    pub state: KeyEventState,
}

impl KeyEvent {
    /// A key press of `code` with `modifiers`.
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// A key event of `code` with `modifiers` and an explicit `kind`.
    #[must_use]
    pub const fn new_with_kind(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> Self {
        Self {
            code,
            modifiers,
            kind,
            state: KeyEventState::NONE,
        }
    }
}

impl From<KeyCode> for KeyEvent {
    fn from(code: KeyCode) -> Self {
        Self::new(code, KeyModifiers::NONE)
    }
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left button.
    Left,
    /// Right button.
    Right,
    /// Middle button.
    Middle,
}

/// What a mouse event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Button pressed.
    Down(MouseButton),
    /// Button released.
    Up(MouseButton),
    /// Moved with a button held.
    Drag(MouseButton),
    /// Moved with no button held.
    Moved,
    /// Wheel down.
    ScrollDown,
    /// Wheel up.
    ScrollUp,
    /// Wheel left (no neutral equivalent).
    ScrollLeft,
    /// Wheel right (no neutral equivalent).
    ScrollRight,
}

/// A mouse event at a cell position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// What happened.
    pub kind: MouseEventKind,
    /// Column of the pointer.
    pub column: u16,
    /// Row of the pointer.
    pub row: u16,
    /// The held modifiers.
    pub modifiers: KeyModifiers,
}

/// Convert modifier flags into neutral [`Modifiers`] (`SUPER` is `cmd`).
#[must_use]
pub fn modifiers(m: KeyModifiers) -> Modifiers {
    Modifiers {
        ctrl: m.contains(KeyModifiers::CONTROL),
        alt: m.contains(KeyModifiers::ALT),
        shift: m.contains(KeyModifiers::SHIFT),
        cmd: m.contains(KeyModifiers::SUPER),
    }
}

/// Convert a key event into a [`KeyInput`]; its kind is ignored and an
/// unnamed key becomes [`Key::Other`].
#[must_use]
pub fn key_input(ev: KeyEvent) -> KeyInput {
    let key = match ev.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::F(n) => Key::F(n),
        KeyCode::Insert | KeyCode::CapsLock | KeyCode::Null => Key::Other,
    };
    KeyInput {
        key,
        mods: modifiers(ev.modifiers),
    }
}

/// Convert a mouse event into a [`MouseInput`]; horizontal scroll yields
/// `None`.
#[must_use]
pub fn mouse_input(ev: MouseEvent) -> Option<MouseInput> {
    let button = |b: MouseButton| match b {
        MouseButton::Left => NeutralButton::Left,
        MouseButton::Right => NeutralButton::Right,
        MouseButton::Middle => NeutralButton::Middle,
    };
    let kind = match ev.kind {
        MouseEventKind::Down(b) => MouseKind::Down(button(b)),
        MouseEventKind::Up(b) => MouseKind::Up(button(b)),
        MouseEventKind::Drag(b) => MouseKind::Drag(button(b)),
        MouseEventKind::Moved => MouseKind::Moved,
        MouseEventKind::ScrollUp => MouseKind::ScrollUp,
        MouseEventKind::ScrollDown => MouseKind::ScrollDown,
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => return None,
    };
    Some(MouseInput {
        kind,
        column: ev.column,
        row: ev.row,
        mods: modifiers(ev.modifiers),
    })
}

/// Event-typed methods on [`App`], mirroring the frontend event loop.
#[allow(async_fn_in_trait)]
pub trait CrosstermAppExt {
    /// Feed a key event to [`App::handle_key`], ignoring releases.
    async fn handle_key_event(&mut self, key: KeyEvent);

    /// Feed a mouse event to [`App::handle_mouse`]; horizontal scroll is
    /// dropped.
    fn handle_mouse_event(&mut self, event: MouseEvent);
}

impl CrosstermAppExt for App {
    async fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.handle_key(key_input(key)).await;
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        if let Some(input) = mouse_input(event) {
            self.handle_mouse(input);
        }
    }
}

/// Event-typed [`codemason_app_core::input::handle_key`]: releases yield
/// `None`.
pub async fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    codemason_app_core::input::handle_key(app, key_input(key)).await
}

/// Event-typed [`codemason_app_core::input::is_global_quit_key`].
#[must_use]
pub fn is_global_quit_key(key: KeyEvent) -> bool {
    codemason_app_core::input::is_global_quit_key(key_input(key))
}

/// Event-typed [`codemason_app_core::app::handle_loop_setup_key`].
pub fn handle_loop_setup_key(app: &mut App, key: KeyEvent) {
    codemason_app_core::app::handle_loop_setup_key(app, key_input(key));
}
