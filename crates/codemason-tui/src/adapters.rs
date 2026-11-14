//! Adapters between `codemason-app-core` neutral types and ratatui/crossterm.
//!
//! Depends on `codemason-app-core`, `ratatui`, `crossterm` and [`crate::theme`].

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};
use codemason_app_core::styled::{StyledLine, ThemeColor};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton as CtMouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// Resolve a [`ThemeColor`] to a base ratatui style using the theme style fns.
fn base_style(color: ThemeColor) -> Style {
    match color {
        ThemeColor::Accent => theme::heading(),
        ThemeColor::Text => Style::default(),
        ThemeColor::Muted | ThemeColor::Border => theme::muted(),
        ThemeColor::Error => theme::error(),
        ThemeColor::Warning => theme::warning(),
        ThemeColor::Success => theme::success(),
        ThemeColor::Info => theme::info(),
        ThemeColor::Rgb(r, g, b) => Style::default().fg(Color::Rgb(r, g, b)),
    }
}

/// Convert a [`StyledLine`] into an owned ratatui [`Line`].
///
/// Colors resolve through the existing `theme` style fns; bold, italic,
/// underline and dim become ratatui modifiers. `monospace` is ignored (a
/// terminal is always monospace).
pub fn to_ratatui_line(line: &StyledLine) -> Line<'static> {
    let spans = line
        .spans
        .iter()
        .map(|s| {
            let mut style = s.style.fg.map(base_style).unwrap_or_default();
            if let Some(bg) = s.style.bg {
                style.bg = base_style(bg).fg;
            }
            let mut mods = Modifier::empty();
            mods.set(Modifier::BOLD, s.style.bold);
            mods.set(Modifier::ITALIC, s.style.italic);
            mods.set(Modifier::UNDERLINED, s.style.underline);
            mods.set(Modifier::DIM, s.style.dim);
            Span::styled(s.text.clone(), style.add_modifier(mods))
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// Convert crossterm modifier flags into neutral [`Modifiers`].
///
/// `SUPER` maps to `cmd`; `META` and `HYPER` have no neutral equivalent and
/// are dropped (the app binds neither).
pub fn from_crossterm_modifiers(m: KeyModifiers) -> Modifiers {
    Modifiers {
        ctrl: m.contains(KeyModifiers::CONTROL),
        alt: m.contains(KeyModifiers::ALT),
        shift: m.contains(KeyModifiers::SHIFT),
        cmd: m.contains(KeyModifiers::SUPER),
    }
}

/// Convert a crossterm key event into a neutral [`KeyInput`].
///
/// The event kind (press/release/repeat) is not carried over; callers filter
/// releases before converting. Keys with no named neutral equivalent (Insert,
/// media keys, lone modifier presses...) become [`Key::Other`], so "any key"
/// behaviour still sees them. Always returns `Some` today; the `Option` keeps
/// room for frontends' unmappable events.
pub fn from_crossterm_key(ev: KeyEvent) -> Option<KeyInput> {
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
        _ => Key::Other,
    };
    Some(KeyInput {
        key,
        mods: from_crossterm_modifiers(ev.modifiers),
    })
}

/// Convert a crossterm mouse event into a neutral [`MouseInput`].
///
/// Returns `None` for horizontal scroll (`ScrollLeft`/`ScrollRight`), which
/// has no neutral equivalent and no handler in the app.
pub fn from_crossterm_mouse(ev: MouseEvent) -> Option<MouseInput> {
    let button = |b: CtMouseButton| match b {
        CtMouseButton::Left => MouseButton::Left,
        CtMouseButton::Right => MouseButton::Right,
        CtMouseButton::Middle => MouseButton::Middle,
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
        mods: from_crossterm_modifiers(ev.modifiers),
    })
}

/// Convert a neutral [`Area`] into a ratatui [`Rect`].
///
/// A free function because the orphan rule forbids `From` between two
/// foreign types in this crate.
pub fn area_to_rect(a: Area) -> Rect {
    Rect::new(a.x, a.y, a.width, a.height)
}

/// Convert a ratatui [`Rect`] into a neutral [`Area`].
pub fn rect_to_area(r: Rect) -> Area {
    Area {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
