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
    /// Any other key a frontend reports (Insert, media keys, lone modifier
    /// presses...). No binding matches it, but it still counts as a key
    /// press for "any key" behaviour such as dismissing a banner.
    Other,
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

impl Modifiers {
    /// No modifier held.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };
    /// Only Control held.
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };
    /// Only Alt held.
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    /// Only Shift held.
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    /// Only Command / Super held.
    pub const CMD: Self = Self {
        cmd: true,
        ..Self::NONE
    };

    /// Whether every modifier held in `other` is also held in `self`.
    ///
    /// # Arguments
    /// * `other` - the modifiers that must all be present.
    ///
    /// # Returns
    /// `true` when `self` is a superset of `other` (so `contains(NONE)` is
    /// always `true`).
    pub fn contains(self, other: Self) -> bool {
        (self.ctrl || !other.ctrl)
            && (self.alt || !other.alt)
            && (self.shift || !other.shift)
            && (self.cmd || !other.cmd)
    }

    /// Whether no modifier is held.
    pub fn is_empty(self) -> bool {
        self == Self::NONE
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    /// The union of two modifier sets.
    fn bitor(self, rhs: Self) -> Self {
        Self {
            ctrl: self.ctrl || rhs.ctrl,
            alt: self.alt || rhs.alt,
            shift: self.shift || rhs.shift,
            cmd: self.cmd || rhs.cmd,
        }
    }
}

/// A key press with its modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    /// The key pressed.
    pub key: Key,
    /// Modifiers held during the press.
    pub mods: Modifiers,
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary (left) button.
    Left,
    /// Secondary (right) button.
    Right,
    /// Middle button / wheel click.
    Middle,
}

/// What happened in a [`MouseInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    /// A button was pressed.
    Down(MouseButton),
    /// A button was released.
    Up(MouseButton),
    /// The pointer moved with a button held.
    Drag(MouseButton),
    /// The pointer moved with no button held.
    Moved,
    /// Wheel scrolled up (away from the user).
    ScrollUp,
    /// Wheel scrolled down (towards the user).
    ScrollDown,
}

/// A mouse event at a cell position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseInput {
    /// The kind of event.
    pub kind: MouseKind,
    /// Column of the pointer, in cells from the left edge.
    pub column: u16,
    /// Row of the pointer, in cells from the top edge.
    pub row: u16,
    /// Modifiers held during the event.
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
