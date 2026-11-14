//! Adapters between `codemason-app-core` neutral types and ratatui/crossterm.
//!
//! Depends on `codemason-app-core`, `ratatui`, `crossterm` and [`crate::theme`].

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::styled::{StyledLine, ThemeColor};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

/// Convert a crossterm key event into a neutral [`KeyInput`].
///
/// Returns `None` for keys with no neutral equivalent (media keys, caps lock...).
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
        _ => return None,
    };
    let m = ev.modifiers;
    Some(KeyInput {
        key,
        mods: Modifiers {
            ctrl: m.contains(KeyModifiers::CONTROL),
            alt: m.contains(KeyModifiers::ALT),
            shift: m.contains(KeyModifiers::SHIFT),
            cmd: m.contains(KeyModifiers::SUPER),
        },
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
