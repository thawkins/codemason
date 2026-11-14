//! Tests for `codemason_app_core::input_event`.

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};

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
