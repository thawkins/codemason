//! Tests for `codemason_app_core::input_event`.

use codemason_app_core::input_event::{
    Key, KeyInput, Modifiers, MouseButton, MouseInput, MouseKind,
};

#[test]
fn test_modifiers_default_all_false() {
    let m = Modifiers::default();
    assert!(!m.ctrl && !m.alt && !m.shift && !m.cmd);
}

#[test]
fn test_key_input_equality_depends_on_mods() {
    let a = KeyInput {
        key: Key::Char('l'),
        mods: Modifiers::default(),
    };
    let b = KeyInput {
        key: Key::Char('l'),
        mods: Modifiers {
            alt: true,
            ..Modifiers::default()
        },
    };
    assert_eq!(a, a);
    assert_ne!(a, b);
}

#[test]
fn test_modifiers_consts_and_contains() {
    let both = Modifiers::CTRL | Modifiers::SHIFT;
    assert!(both.ctrl && both.shift && !both.alt && !both.cmd);
    assert!(both.contains(Modifiers::CTRL));
    assert!(both.contains(Modifiers::NONE));
    assert!(!Modifiers::CTRL.contains(both));
    assert!(Modifiers::NONE.is_empty());
    assert!(!Modifiers::ALT.is_empty());
    assert_eq!(
        Modifiers::CMD,
        Modifiers {
            cmd: true,
            ..Modifiers::default()
        }
    );
}

#[test]
fn test_mouse_input_fields_and_equality() {
    let down = MouseInput {
        kind: MouseKind::Down(MouseButton::Left),
        column: 3,
        row: 7,
        mods: Modifiers::default(),
    };
    let drag = MouseInput {
        kind: MouseKind::Drag(MouseButton::Left),
        ..down
    };
    assert_eq!(down.column, 3);
    assert_eq!(down.row, 7);
    assert_ne!(down, drag);
    assert_ne!(
        MouseKind::Up(MouseButton::Right),
        MouseKind::Up(MouseButton::Middle)
    );
    assert_ne!(MouseKind::ScrollUp, MouseKind::ScrollDown);
    assert_ne!(MouseKind::Moved, MouseKind::ScrollUp);
}
