//! Frontend-neutral input events.
//!
//! No dependencies.

/// A logical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
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
}

/// Modifier key state; all false by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Control held.
    pub ctrl: bool,
    /// Alt / Option held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
    /// Command / Super held.
    pub cmd: bool,
}

/// A key press with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    /// The key pressed.
    pub key: Key,
    /// Modifiers held during the press.
    pub mods: Modifiers,
}

/// A frontend-neutral UI event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiEvent {
    /// A key press.
    Key(KeyInput),
    /// Pasted text.
    Paste(String),
    /// Scroll by a signed number of lines.
    Scroll {
        /// Lines to scroll (negative is up).
        lines: i32,
    },
    /// Viewport resized.
    Resize {
        /// New width in columns.
        cols: u16,
        /// New height in rows.
        rows: u16,
    },
}
