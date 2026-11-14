//! Tests for `codemason_tui::adapters`.

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};
use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use codemason_tui::adapters::{
    area_to_rect, from_crossterm_key, from_crossterm_mouse, from_ratatui_line, from_ratatui_style,
    rect_to_area, to_ratatui_line, to_ratatui_style,
};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton as CtMouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[test]
fn test_from_crossterm_alt_l() {
    let ev = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT);
    assert_eq!(
        from_crossterm_key(ev),
        Some(KeyInput {
            key: Key::Char('l'),
            mods: Modifiers {
                alt: true,
                ..Modifiers::default()
            },
        })
    );
}

#[test]
fn test_from_crossterm_unmapped_key_is_other() {
    // Unnamed keys still reach the app as a key press (for "any key"
    // behaviour such as dismissing the run-cost banner), as `Key::Other`.
    let ev = KeyEvent::new(KeyCode::CapsLock, KeyModifiers::NONE);
    assert_eq!(
        from_crossterm_key(ev),
        Some(KeyInput {
            key: Key::Other,
            mods: Modifiers::default(),
        })
    );
}

#[test]
fn test_from_crossterm_mouse_maps_kind_position_and_mods() {
    let ev = MouseEvent {
        kind: MouseEventKind::Drag(CtMouseButton::Right),
        column: 4,
        row: 9,
        modifiers: KeyModifiers::CONTROL,
    };
    assert_eq!(
        from_crossterm_mouse(ev),
        Some(MouseInput {
            kind: MouseKind::Drag(MouseButton::Right),
            column: 4,
            row: 9,
            mods: Modifiers::CTRL,
        })
    );
    let wheel = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        ..ev
    };
    assert_eq!(
        from_crossterm_mouse(wheel).map(|m| m.kind),
        Some(MouseKind::ScrollDown)
    );
    let horizontal = MouseEvent {
        kind: MouseEventKind::ScrollLeft,
        ..ev
    };
    assert_eq!(from_crossterm_mouse(horizontal), None);
}

#[test]
fn test_to_ratatui_line_maps_colors_and_modifiers() {
    let line = StyledLine {
        spans: vec![
            StyledSpan {
                text: "e".into(),
                style: TextStyle {
                    fg: Some(ThemeColor::Error),
                    ..TextStyle::default()
                },
            },
            StyledSpan {
                text: "b".into(),
                style: TextStyle {
                    fg: Some(ThemeColor::Rgb(1, 2, 3)),
                    bold: true,
                    italic: true,
                    underline: true,
                    dim: true,
                    ..TextStyle::default()
                },
            },
        ],
    };
    let out = to_ratatui_line(&line);
    assert_eq!(out.spans.len(), 2);
    assert_eq!(out.spans[0].style.fg, Some(Color::Red));
    assert_eq!(out.spans[1].style.fg, Some(Color::Rgb(1, 2, 3)));
    let m = out.spans[1].style.add_modifier;
    assert!(m.contains(Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED | Modifier::DIM));
}

#[test]
fn test_area_rect_roundtrip() {
    let a = Area {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    };
    let r = area_to_rect(a);
    assert_eq!(r, Rect::new(1, 2, 3, 4));
    assert_eq!(rect_to_area(r), a);
}

/// Every literal ratatui color the app can emit, with its ThemeColor.
fn literal_pairs() -> Vec<(Color, ThemeColor)> {
    vec![
        (Color::Reset, ThemeColor::Reset),
        (Color::Black, ThemeColor::Black),
        (Color::Red, ThemeColor::Red),
        (Color::Green, ThemeColor::Green),
        (Color::Yellow, ThemeColor::Yellow),
        (Color::Blue, ThemeColor::Blue),
        (Color::Magenta, ThemeColor::Magenta),
        (Color::Cyan, ThemeColor::Cyan),
        (Color::Gray, ThemeColor::Gray),
        (Color::DarkGray, ThemeColor::DarkGray),
        (Color::LightRed, ThemeColor::LightRed),
        (Color::LightGreen, ThemeColor::LightGreen),
        (Color::LightYellow, ThemeColor::LightYellow),
        (Color::LightBlue, ThemeColor::LightBlue),
        (Color::LightMagenta, ThemeColor::LightMagenta),
        (Color::LightCyan, ThemeColor::LightCyan),
        (Color::White, ThemeColor::White),
        (Color::Indexed(236), ThemeColor::Indexed(236)),
        (Color::Rgb(210, 210, 255), ThemeColor::Rgb(210, 210, 255)),
    ]
}

#[test]
fn test_literal_theme_colors_map_to_exact_ratatui_colors() {
    for (color, theme) in literal_pairs() {
        let fg = to_ratatui_style(&TextStyle::new().fg(theme));
        assert_eq!(fg, Style::default().fg(color), "fg {theme:?}");
        let bg = to_ratatui_style(&TextStyle::new().bg(theme));
        assert_eq!(bg, Style::default().bg(color), "bg {theme:?}");
        assert_eq!(
            from_ratatui_style(Style::default().fg(color)).fg,
            Some(theme)
        );
    }
}

#[test]
fn test_ratatui_styles_round_trip_losslessly() {
    let mods = [
        Modifier::empty(),
        Modifier::BOLD,
        Modifier::DIM,
        Modifier::ITALIC,
        Modifier::UNDERLINED,
        Modifier::BOLD | Modifier::DIM | Modifier::ITALIC | Modifier::UNDERLINED,
    ];
    for (fg, _) in literal_pairs() {
        for (bg, _) in literal_pairs() {
            for m in mods {
                let style = Style::default().fg(fg).bg(bg).add_modifier(m);
                assert_eq!(to_ratatui_style(&from_ratatui_style(style)), style);
            }
        }
    }
    assert_eq!(
        to_ratatui_style(&from_ratatui_style(Style::default())),
        Style::default()
    );
}

#[test]
fn test_ratatui_line_round_trips_and_patches_line_style() {
    let line = Line::from(vec![
        Span::raw("plain"),
        Span::styled(
            "x",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("y", Style::default().bg(Color::LightBlue).fg(Color::Black)),
    ]);
    let styled = from_ratatui_line(&line);
    assert_eq!(styled.text(), "plainxy");
    assert_eq!(to_ratatui_line(&styled), line);

    let with_line_style = Line::from(vec![Span::raw("a")]).style(Style::default().fg(Color::Red));
    let patched = from_ratatui_line(&with_line_style);
    assert_eq!(patched.spans[0].style.fg, Some(ThemeColor::Red));
}
