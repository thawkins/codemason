//! Tests for the GUI keymap (`codemason_gui::keys`): the per-platform
//! default bindings, the egui-event to `KeyInput` conversion, routing a
//! frame's key events to the `App`, and the user overrides file.
//!
//! No window or GPU is needed: routing runs over a bare `egui::InputState`.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_gui::keys::{
    KeyRouter, command_key_input, default_keymap, load_keymap, to_key_input,
};
use craft_keymap::{Binding, CommandRef, Platform};

/// A scratch path under `target/temp` for this test file.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/codemason-gui-test-keys")
        .join(name);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The single binding text of `command` in `keymap`.
fn bound(keymap: &craft_keymap::Keymap, command: &str) -> Vec<Binding> {
    keymap.bindings(&CommandRef::new(command))
}

/// An egui key press of `key` with `modifiers`.
fn press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// An input state holding `events`, with the frame modifiers of the last one.
fn input_with(events: Vec<egui::Event>) -> egui::InputState {
    let mut input = egui::InputState::default();
    if let Some(egui::Event::Key { modifiers, .. }) = events.last() {
        input.modifiers = *modifiers;
    }
    input.events = events;
    input
}

#[test]
fn test_keymap_linux_defaults_use_alt() {
    let alt_l = Binding::parse("Alt+L").expect("parse Alt+L");
    for platform in [Platform::Linux, Platform::Windows] {
        let keymap = default_keymap(platform);
        assert_eq!(
            bound(&keymap, "panel.log"),
            vec![alt_l.clone()],
            "{platform:?}"
        );
        assert_eq!(
            bound(&keymap, "panel.profile"),
            vec![Binding::parse("Alt+P").expect("parse")]
        );
        assert_eq!(
            bound(&keymap, "panel.tasks"),
            vec![Binding::parse("Alt+I").expect("parse")]
        );
        assert_eq!(
            bound(&keymap, "panel.memory"),
            vec![Binding::parse("Alt+M").expect("parse")]
        );
        assert_eq!(
            bound(&keymap, "queue.menu"),
            vec![Binding::parse("Alt+Q").expect("parse")]
        );
        assert_eq!(
            bound(&keymap, "agent.stop"),
            vec![Binding::parse("Alt+X").expect("parse")]
        );
        assert_eq!(
            bound(&keymap, "app.quit"),
            vec![Binding::parse("Ctrl+D").expect("parse")]
        );
    }
}

#[test]
fn test_keymap_mac_defaults_use_cmd() {
    let keymap = default_keymap(Platform::Mac);
    assert_eq!(
        bound(&keymap, "panel.log"),
        vec![Binding::parse("Cmd+Shift+L").expect("parse")]
    );
    assert_eq!(
        bound(&keymap, "app.quit"),
        vec![Binding::parse("Cmd+Q").expect("parse")]
    );
    assert!(keymap.conflicts().is_empty(), "{:?}", keymap.conflicts());
}

#[test]
fn test_to_key_input_maps_escape() {
    let got = to_key_input(&press(egui::Key::Escape, egui::Modifiers::NONE));
    assert_eq!(
        got,
        Some(KeyInput {
            key: Key::Esc,
            mods: Modifiers::NONE,
        })
    );
}

#[test]
fn test_to_key_input_maps_alt_letter_and_skips_release() {
    let got = to_key_input(&press(egui::Key::L, egui::Modifiers::ALT));
    assert_eq!(
        got,
        Some(KeyInput {
            key: Key::Char('l'),
            mods: Modifiers::ALT,
        })
    );
    let release = egui::Event::Key {
        key: egui::Key::L,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    assert_eq!(to_key_input(&release), None);
    assert_eq!(to_key_input(&egui::Event::Text("l".into())), None);
}

#[test]
fn test_command_key_input_is_the_linux_chord() {
    assert_eq!(
        command_key_input(&CommandRef::new("panel.log")),
        Some(KeyInput {
            key: Key::Char('l'),
            mods: Modifiers::ALT,
        })
    );
    assert_eq!(command_key_input(&CommandRef::new("no.such")), None);
}

#[tokio::test]
async fn test_mac_cmd_shift_l_toggles_log_like_alt_l() {
    let mut router = KeyRouter::new(default_keymap(Platform::Mac));
    let mods = egui::Modifiers::MAC_CMD | egui::Modifiers::SHIFT;
    let mut input = input_with(vec![press(egui::Key::L, mods)]);

    let keys = router.route(&mut input, false);

    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::Char('l'),
            mods: Modifiers::ALT,
        }]
    );
    assert!(input.events.is_empty(), "the hit is removed from egui");

    let mut app = support::make_app();
    let before = app.show_log;
    for key in keys {
        app.handle_key(key).await;
    }
    assert_ne!(app.show_log, before, "Cmd+Shift+L toggles the log panel");

    let mut alt_app = support::make_app();
    alt_app
        .handle_key(KeyInput {
            key: Key::Char('l'),
            mods: Modifiers::ALT,
        })
        .await;
    assert_eq!(alt_app.show_log, app.show_log, "same flag as Alt+L");
}

#[test]
fn test_route_skips_typed_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::A, egui::Modifiers::NONE),
        egui::Event::Text("a".into()),
        press(egui::Key::Escape, egui::Modifiers::NONE),
    ]);

    let keys = router.route(&mut input, true);

    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::Esc,
            mods: Modifiers::NONE,
        }],
        "typed text stays with the prompt; Esc still reaches the App"
    );
    assert_eq!(
        input.events.len(),
        3,
        "unbound events stay for the TextEdit"
    );
}

#[test]
fn test_route_forwards_unbound_keys_when_not_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![press(egui::Key::PageUp, egui::Modifiers::NONE)]);

    let keys = router.route(&mut input, false);

    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::PageUp,
            mods: Modifiers::NONE,
        }]
    );
}

#[test]
fn test_load_keymap_applies_overrides_file() {
    let path = scratch("override").join("gui-keys.json");
    std::fs::write(&path, r#"{ "panel.log": "Alt+K" }"#).expect("write overrides");

    let keymap = load_keymap(Platform::Linux, &path);

    assert_eq!(
        bound(&keymap, "panel.log"),
        vec![Binding::parse("Alt+K").expect("parse")]
    );
}

#[test]
fn test_load_keymap_falls_back_on_missing_or_invalid_file() {
    let dir = scratch("fallback");
    let missing = load_keymap(Platform::Linux, &dir.join("absent.json"));
    assert_eq!(
        bound(&missing, "panel.log"),
        vec![Binding::parse("Alt+L").expect("parse")]
    );

    let invalid = dir.join("gui-keys.json");
    std::fs::write(&invalid, "not json {").expect("write invalid");
    let keymap = load_keymap(Platform::Linux, &invalid);
    assert_eq!(
        bound(&keymap, "panel.log"),
        vec![Binding::parse("Alt+L").expect("parse")]
    );
}
