//! Adapters between `codemason-app-core` neutral types and ratatui/crossterm.
//!
//! Depends on `codemason-app-core`, `ratatui`, `crossterm` and [`crate::theme`].

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};
use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton as CtMouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::theme;

/// Resolve a [`ThemeColor`] to a base ratatui style using the theme style fns.
///
/// Semantic colors go through the `theme` style fns; literal colors (the
/// ANSI palette, `Indexed`, `Rgb`, `Reset`) map to exactly one ratatui
/// [`Color`] so a [`StyledLine`] built by [`from_ratatui_line`] round-trips.
fn base_style(color: ThemeColor) -> Style {
    match color {
        ThemeColor::Accent => theme::heading(),
        ThemeColor::Text => Style::default(),
        ThemeColor::Muted | ThemeColor::Border => theme::muted(),
        ThemeColor::Error => theme::error(),
        ThemeColor::Warning => theme::warning(),
        ThemeColor::Success => theme::success(),
        ThemeColor::Info => theme::info(),
        ThemeColor::Rgb(_, _, _)
        | ThemeColor::Reset
        | ThemeColor::Black
        | ThemeColor::Red
        | ThemeColor::Green
        | ThemeColor::Yellow
        | ThemeColor::Blue
        | ThemeColor::Magenta
        | ThemeColor::Cyan
        | ThemeColor::Gray
        | ThemeColor::DarkGray
        | ThemeColor::LightRed
        | ThemeColor::LightGreen
        | ThemeColor::LightYellow
        | ThemeColor::LightBlue
        | ThemeColor::LightMagenta
        | ThemeColor::LightCyan
        | ThemeColor::White
        | ThemeColor::Indexed(_) => Style::default().fg(literal_color(color).unwrap_or_default()),
    }
}

/// The ratatui [`Color`] of a literal [`ThemeColor`]; `None` for semantic ones.
fn literal_color(color: ThemeColor) -> Option<Color> {
    Some(match color {
        ThemeColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        ThemeColor::Reset => Color::Reset,
        ThemeColor::Black => Color::Black,
        ThemeColor::Red => Color::Red,
        ThemeColor::Green => Color::Green,
        ThemeColor::Yellow => Color::Yellow,
        ThemeColor::Blue => Color::Blue,
        ThemeColor::Magenta => Color::Magenta,
        ThemeColor::Cyan => Color::Cyan,
        ThemeColor::Gray => Color::Gray,
        ThemeColor::DarkGray => Color::DarkGray,
        ThemeColor::LightRed => Color::LightRed,
        ThemeColor::LightGreen => Color::LightGreen,
        ThemeColor::LightYellow => Color::LightYellow,
        ThemeColor::LightBlue => Color::LightBlue,
        ThemeColor::LightMagenta => Color::LightMagenta,
        ThemeColor::LightCyan => Color::LightCyan,
        ThemeColor::White => Color::White,
        ThemeColor::Indexed(i) => Color::Indexed(i),
        ThemeColor::Accent
        | ThemeColor::Text
        | ThemeColor::Muted
        | ThemeColor::Error
        | ThemeColor::Warning
        | ThemeColor::Success
        | ThemeColor::Info
        | ThemeColor::Border => return None,
    })
}

/// The literal [`ThemeColor`] for a ratatui [`Color`] (inverse of
/// [`literal_color`]).
fn theme_color(color: Color) -> ThemeColor {
    match color {
        Color::Reset => ThemeColor::Reset,
        Color::Black => ThemeColor::Black,
        Color::Red => ThemeColor::Red,
        Color::Green => ThemeColor::Green,
        Color::Yellow => ThemeColor::Yellow,
        Color::Blue => ThemeColor::Blue,
        Color::Magenta => ThemeColor::Magenta,
        Color::Cyan => ThemeColor::Cyan,
        Color::Gray => ThemeColor::Gray,
        Color::DarkGray => ThemeColor::DarkGray,
        Color::LightRed => ThemeColor::LightRed,
        Color::LightGreen => ThemeColor::LightGreen,
        Color::LightYellow => ThemeColor::LightYellow,
        Color::LightBlue => ThemeColor::LightBlue,
        Color::LightMagenta => ThemeColor::LightMagenta,
        Color::LightCyan => ThemeColor::LightCyan,
        Color::White => ThemeColor::White,
        Color::Rgb(r, g, b) => ThemeColor::Rgb(r, g, b),
        Color::Indexed(i) => ThemeColor::Indexed(i),
    }
}

/// Convert a [`TextStyle`] into a ratatui [`Style`].
///
/// Colors resolve through [`base_style`]; bold, italic, underline and dim
/// become ratatui modifiers. `monospace` is ignored (a terminal is always
/// monospace).
pub fn to_ratatui_style(style: &TextStyle) -> Style {
    let mut out = style.fg.map(base_style).unwrap_or_default();
    if let Some(bg) = style.bg {
        out.bg = base_style(bg).fg;
    }
    let mut mods = Modifier::empty();
    mods.set(Modifier::BOLD, style.bold);
    mods.set(Modifier::ITALIC, style.italic);
    mods.set(Modifier::UNDERLINED, style.underline);
    mods.set(Modifier::DIM, style.dim);
    out.add_modifier(mods)
}

/// Convert a ratatui [`Style`] into a [`TextStyle`] using literal colors.
///
/// Lossless for every style the app builds (fg/bg colors plus the bold,
/// italic, underline and dim modifiers): `to_ratatui_style(&from_ratatui_style(s)) == s`.
/// Other modifier bits, removed modifiers and underline colors are not
/// represented and are dropped.
pub fn from_ratatui_style(style: Style) -> TextStyle {
    let m = style.add_modifier;
    TextStyle {
        fg: style.fg.map(theme_color),
        bg: style.bg.map(theme_color),
        bold: m.contains(Modifier::BOLD),
        italic: m.contains(Modifier::ITALIC),
        underline: m.contains(Modifier::UNDERLINED),
        dim: m.contains(Modifier::DIM),
        monospace: false,
    }
}

/// Convert a [`StyledLine`] into an owned ratatui [`Line`].
///
/// See [`to_ratatui_style`] for how each span's style is mapped.
pub fn to_ratatui_line(line: &StyledLine) -> Line<'static> {
    let spans = line
        .spans
        .iter()
        .map(|s| Span::styled(s.text.clone(), to_ratatui_style(&s.style)))
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// Convert a ratatui [`Line`] into a [`StyledLine`].
///
/// The line-level style is patched into every span (as ratatui does when it
/// paints), so the painted result is unchanged; see [`from_ratatui_style`]
/// for what a style keeps.
pub fn from_ratatui_line(line: &Line<'_>) -> StyledLine {
    StyledLine {
        spans: line
            .spans
            .iter()
            .map(|s| StyledSpan {
                text: s.content.to_string(),
                style: from_ratatui_style(line.style.patch(s.style)),
            })
            .collect(),
    }
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
