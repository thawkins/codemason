//! Tests for `codemason_tui::adapters`.

use codemason_app_core::geometry::Area;
use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};
use codemason_app_core::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use codemason_tui::adapters::{
    area_to_rect, from_crossterm_key, from_crossterm_mouse, rect_to_area, to_ratatui_line,
};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton as CtMouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

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
