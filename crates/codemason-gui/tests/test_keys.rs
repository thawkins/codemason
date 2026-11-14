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
    Focus, KeyRouter, TUI_ALT_LETTERS, command_key_input, default_keymap, is_prompt_edit_key,
    load_keymap, to_key_input,
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

/// The bindings of `command` in `keymap`.
fn bound(keymap: &craft_keymap::Keymap, command: &str) -> Vec<Binding> {
    keymap.bindings(&CommandRef::new(command))
}

/// The prompt has the keyboard and no selection.
const TYPING: Focus = Focus::Prompt {
    has_selection: false,
    completion_open: false,
    up_to_app: false,
    down_to_app: false,
    question_opens_shortcuts: false,
};

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

    let keys = router.route(&mut input, Focus::Elsewhere);

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

    let keys = router.route(&mut input, TYPING);

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

    let keys = router.route(&mut input, Focus::Elsewhere);

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

/// An input state holding `events` with frame modifiers `modifiers`.
fn input_mods(events: Vec<egui::Event>, modifiers: egui::Modifiers) -> egui::InputState {
    let mut input = input_with(events);
    input.modifiers = modifiers;
    input
}

#[test]
fn test_route_forwards_tui_alt_letter_and_drops_its_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::T, egui::Modifiers::ALT),
        egui::Event::Text("t".into()),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::Char('t'),
            mods: Modifiers::ALT,
        }]
    );
    assert!(
        !input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Text(_))),
        "the letter is never typed into the prompt: {:?}",
        input.events
    );
}

#[test]
fn test_route_forwards_ctrl_c_and_ctrl_x_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_mods(
        vec![egui::Event::Copy, egui::Event::Cut],
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    );

    let keys = router.route(&mut input, TYPING);

    assert_eq!(
        keys,
        vec![
            KeyInput {
                key: Key::Char('c'),
                mods: Modifiers::CTRL,
            },
            KeyInput {
                key: Key::Char('x'),
                mods: Modifiers::CTRL,
            },
        ]
    );
}

#[test]
fn test_route_ignores_mac_cmd_x_and_pc_shift_delete_cut() {
    let mut mac = KeyRouter::new(default_keymap(Platform::Mac));
    let mut input = input_mods(
        vec![egui::Event::Cut, egui::Event::Copy],
        egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND,
    );
    assert_eq!(
        mac.route(&mut input, Focus::Elsewhere),
        Vec::new(),
        "Mac Cmd+X/Cmd+C"
    );

    let mut pc = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_mods(vec![egui::Event::Cut], egui::Modifiers::SHIFT);
    assert_eq!(
        pc.route(&mut input, Focus::Elsewhere),
        Vec::new(),
        "Shift+Delete"
    );
}

#[test]
fn test_route_leaves_ctrl_c_and_ctrl_x_to_a_prompt_selection() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_mods(
        vec![egui::Event::Copy, egui::Event::Cut],
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    );

    let keys = router.route(
        &mut input,
        Focus::Prompt {
            has_selection: true,
            completion_open: false,
            up_to_app: false,
            down_to_app: false,
            question_opens_shortcuts: false,
        },
    );

    assert_eq!(
        keys,
        Vec::new(),
        "with a selection Ctrl+C copies and Ctrl+X cuts, in the prompt only"
    );
    assert_eq!(input.events.len(), 2, "the prompt still sees Copy and Cut");
}

#[test]
fn test_route_forwards_sidebar_alt_s_and_drops_its_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::S, egui::Modifiers::ALT),
        egui::Event::Text("s".into()),
        press(egui::Key::R, egui::Modifiers::ALT),
        egui::Event::Text("r".into()),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(
        keys,
        vec![
            KeyInput {
                key: Key::Char('s'),
                mods: Modifiers::ALT,
            },
            KeyInput {
                key: Key::Char('r'),
                mods: Modifiers::ALT,
            },
        ]
    );
    assert!(
        !input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Text(_))),
        "{:?}",
        input.events
    );
}

/// The letters `source` binds with Alt held.
///
/// Each `Key::Char('<c>')` literal is read up to the end of its match arm
/// pattern (the `=>`), taking the next line too when the arm does not end on
/// its own line (a guard rustfmt wrapped, or a `KeyInput { .. }` literal).
/// The letter counts when that text holds `Modifiers::ALT` or an `alt`
/// binding that is not negated (`!key.mods.contains(Modifiers::ALT)`,
/// `!alt`). This covers `Key::Char('x') if key.mods.contains(Modifiers::ALT)`,
/// `Key::Char('s') if alt`, a wrapped guard, `matches!(key.key,
/// Key::Char('x')) && key.mods.contains(Modifiers::ALT)` and
/// `KeyInput { key: Key::Char('x'), mods: Modifiers::ALT }`.
fn alt_letters(source: &str) -> Vec<char> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut rest = *line;
        while let Some(at) = rest.find("Key::Char('") {
            let after = &rest[at + "Key::Char('".len()..];
            let mut chars = after.chars();
            let (Some(c), Some('\''), Some(')')) = (chars.next(), chars.next(), chars.next())
            else {
                rest = after;
                continue;
            };
            let tail = chars.as_str();
            let mut window = tail.to_string();
            if !window.contains("=>")
                && let Some(next) = lines.get(i + 1)
            {
                window.push(' ');
                window.push_str(next);
            }
            let arm = window.split("=>").next().unwrap_or_default();
            let arm = arm
                .replace("!key.mods.contains(Modifiers::ALT)", "")
                .replace("!alt", "");
            let alt = arm
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .any(|token| token == "ALT" || token == "alt");
            if alt {
                found.push(c);
            }
            rest = tail;
        }
    }
    found.sort_unstable();
    found.dedup();
    found
}

#[test]
fn test_alt_letters_scanner_recognises_every_form() {
    let source = "\
        Key::Char('a') if key.mods.contains(Modifiers::ALT) => x,
        Key::Char('b') if alt => {
        Key::Char('c')
            if key.mods.contains(Modifiers::ALT) =>
        matches!(key.key, Key::Char('d')) && key.mods.contains(Modifiers::ALT)
        let k = KeyInput { key: Key::Char('e'), mods: Modifiers::ALT };
        Key::Char('f') if !alt => {
        Key::Char('g') if key.mods.contains(Modifiers::CTRL) && !key.mods.contains(Modifiers::ALT) =>
        Key::Char('h') => {
    ";
    assert_eq!(alt_letters(source), vec!['a', 'b', 'c', 'd', 'e']);
}

#[test]
fn test_tui_alt_letters_match_app_core_input_handler() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../codemason-app-core/src");
    let mut dirs = vec![src];
    let mut found = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("read app-core src dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path).expect("read app-core source");
                found.extend(alt_letters(&source));
            }
        }
    }
    found.sort_unstable();
    found.dedup();

    assert_eq!(
        found,
        TUI_ALT_LETTERS.to_vec(),
        "keys::TUI_ALT_LETTERS must list every Alt+letter app-core handles"
    );
}

/// Whether `input` still holds a `Text` event.
fn has_text(input: &egui::InputState) -> bool {
    input
        .events
        .iter()
        .any(|e| matches!(e, egui::Event::Text(_)))
}

#[test]
fn test_route_drops_text_of_unbound_alt_letter_while_typing() {
    for platform in [Platform::Linux, Platform::Windows] {
        let mut router = KeyRouter::new(default_keymap(platform));
        let mut input = input_with(vec![
            press(egui::Key::Z, egui::Modifiers::ALT),
            egui::Event::Text("z".into()),
        ]);

        router.route(&mut input, TYPING);

        assert!(!has_text(&input), "{platform:?}: Alt+Z types nothing");
    }
}

#[test]
fn test_route_keeps_altgr_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Windows));
    let altgr = egui::Modifiers::CTRL | egui::Modifiers::ALT | egui::Modifiers::COMMAND;
    let mut input = input_with(vec![
        press(egui::Key::Q, altgr),
        egui::Event::Text("@".into()),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(keys, Vec::new());
    assert!(has_text(&input), "AltGr+Q still types @");
}

#[test]
fn test_route_keeps_mac_option_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Mac));
    let mut input = input_with(vec![
        press(egui::Key::Z, egui::Modifiers::ALT),
        egui::Event::Text("\u{3a9}".into()),
    ]);

    router.route(&mut input, TYPING);

    assert!(has_text(&input), "macOS Option keeps typing");
}

/// The `Text` events left in `input`, in order.
fn texts(input: &egui::InputState) -> Vec<String> {
    input
        .events
        .iter()
        .filter_map(|e| match e {
            egui::Event::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn test_route_keeps_unrelated_text_after_textless_alt_key_while_typing() {
    for (key, text) in [
        (egui::Key::Backspace, "a"),
        (egui::Key::ArrowLeft, "b"),
        (egui::Key::ArrowUp, "c"),
    ] {
        let mut router = KeyRouter::new(default_keymap(Platform::Linux));
        let mut input = input_with(vec![
            press(key, egui::Modifiers::ALT),
            egui::Event::Text(text.into()),
        ]);

        router.route(&mut input, TYPING);

        assert_eq!(
            texts(&input),
            vec![text.to_owned()],
            "Alt+{key:?} keeps {text}"
        );
    }
}

#[test]
fn test_route_forwards_alt_up_and_keeps_following_text_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::ArrowUp, egui::Modifiers::ALT),
        egui::Event::Text("c".into()),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::Up,
            mods: Modifiers::ALT,
        }]
    );
    assert_eq!(texts(&input), vec!["c".to_owned()]);
}

#[test]
fn test_route_keeps_unmatched_text_after_unbound_alt_letter_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::Z, egui::Modifiers::ALT),
        egui::Event::Text("q".into()),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(keys, Vec::new(), "Alt+Z is not forwarded");
    assert_eq!(
        texts(&input),
        vec!["q".to_owned()],
        "only the unrelated q is typed"
    );
}

#[test]
fn test_route_forwards_and_removes_enter_while_typing() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::Enter, egui::Modifiers::NONE),
        press(egui::Key::Enter, egui::Modifiers::SHIFT),
        press(egui::Key::Enter, egui::Modifiers::ALT),
    ]);

    let keys = router.route(&mut input, TYPING);

    assert_eq!(
        keys,
        vec![
            KeyInput {
                key: Key::Enter,
                mods: Modifiers::NONE,
            },
            KeyInput {
                key: Key::Enter,
                mods: Modifiers::SHIFT,
            },
            KeyInput {
                key: Key::Enter,
                mods: Modifiers::ALT,
            },
        ],
        "the App submits or inserts the newline"
    );
    assert!(
        input.events.is_empty(),
        "the prompt does not act on Enter too"
    );
}

#[test]
fn test_route_forwards_completion_keys_only_while_a_menu_is_open() {
    let keys_for = |completion_open: bool| {
        let mut router = KeyRouter::new(default_keymap(Platform::Linux));
        let mut input = input_with(vec![
            press(egui::Key::ArrowDown, egui::Modifiers::NONE),
            press(egui::Key::Tab, egui::Modifiers::NONE),
        ]);
        let focus = Focus::Prompt {
            has_selection: false,
            completion_open,
            up_to_app: false,
            down_to_app: false,
            question_opens_shortcuts: false,
        };
        let keys = router.route(&mut input, focus);
        (keys, input.events.len())
    };

    let (keys, left) = keys_for(true);
    assert_eq!(
        keys,
        vec![
            KeyInput {
                key: Key::Down,
                mods: Modifiers::NONE,
            },
            KeyInput {
                key: Key::Tab,
                mods: Modifiers::NONE,
            },
        ]
    );
    assert_eq!(left, 0, "the menu keys are removed from the prompt");

    let (keys, left) = keys_for(false);
    assert_eq!(keys, Vec::new(), "with no menu the prompt edits with them");
    assert_eq!(left, 2);
}

#[test]
fn test_route_forwards_up_and_down_only_at_the_history_edges() {
    let route = |up_to_app: bool, down_to_app: bool| {
        let mut router = KeyRouter::new(default_keymap(Platform::Linux));
        let mut input = input_with(vec![
            press(egui::Key::ArrowUp, egui::Modifiers::NONE),
            press(egui::Key::ArrowDown, egui::Modifiers::NONE),
            press(egui::Key::ArrowUp, egui::Modifiers::SHIFT),
        ]);
        let focus = Focus::Prompt {
            has_selection: false,
            completion_open: false,
            up_to_app,
            down_to_app,
            question_opens_shortcuts: false,
        };
        let keys: Vec<Key> = router
            .route(&mut input, focus)
            .into_iter()
            .map(|k| k.key)
            .collect();
        (keys, input.events.len())
    };

    assert_eq!(
        route(true, false),
        (vec![Key::Up], 2),
        "first line: Up browses"
    );
    assert_eq!(
        route(false, true),
        (vec![Key::Down], 2),
        "last line: Down browses"
    );
    assert_eq!(
        route(false, false),
        (Vec::new(), 3),
        "middle: the prompt moves"
    );
    assert_eq!(
        route(true, true).0,
        vec![Key::Up, Key::Down],
        "Shift+Up stays with the prompt's selection"
    );
}

#[test]
fn test_prompt_edit_keys_are_typing_and_caret_keys_only() {
    let key = |key: Key, mods: Modifiers| KeyInput { key, mods };
    assert!(is_prompt_edit_key(key(Key::Char('a'), Modifiers::NONE)));
    assert!(is_prompt_edit_key(key(Key::Char('A'), Modifiers::SHIFT)));
    assert!(is_prompt_edit_key(key(Key::Backspace, Modifiers::NONE)));
    assert!(is_prompt_edit_key(key(Key::Left, Modifiers::CTRL)));
    assert!(!is_prompt_edit_key(key(Key::Char('d'), Modifiers::CTRL)));
    assert!(!is_prompt_edit_key(key(Key::Char('l'), Modifiers::ALT)));
    assert!(!is_prompt_edit_key(key(Key::Esc, Modifiers::NONE)));
    assert!(!is_prompt_edit_key(key(Key::Enter, Modifiers::NONE)));
    assert!(!is_prompt_edit_key(key(Key::F(1), Modifiers::NONE)));
}

/// The keys `router` forwards from `events` under `focus`, and the events
/// left for egui's widgets.
fn route_events(focus: Focus, events: Vec<egui::Event>) -> (Vec<KeyInput>, Vec<egui::Event>) {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(events);
    let keys = router.route(&mut input, focus);
    (keys, input.events)
}

/// A TUI key press of `key` with no modifiers.
fn plain(key: Key) -> KeyInput {
    KeyInput {
        key,
        mods: Modifiers::NONE,
    }
}

#[test]
fn test_route_modal_forwards_and_removes_every_press() {
    let (keys, left) = route_events(
        Focus::Modal {
            field_focused: false,
            field_selection: false,
        },
        vec![
            press(egui::Key::Enter, egui::Modifiers::NONE),
            press(egui::Key::Space, egui::Modifiers::NONE),
            egui::Event::Text(" ".to_string()),
            press(egui::Key::Tab, egui::Modifiers::NONE),
            press(egui::Key::Y, egui::Modifiers::NONE),
            egui::Event::Text("y".to_string()),
        ],
    );

    assert_eq!(
        keys,
        vec![
            plain(Key::Enter),
            plain(Key::Char(' ')),
            plain(Key::Tab),
            plain(Key::Char('y')),
        ],
        "the App gets every press, as in the TUI"
    );
    assert!(
        left.is_empty(),
        "no dialog button sees Enter or Space, and Tab moves no egui focus: {left:?}"
    );
}

#[test]
fn test_route_modal_text_field_keeps_typing_and_editing_keys() {
    let (keys, left) = route_events(
        Focus::Modal {
            field_focused: true,
            field_selection: false,
        },
        vec![
            press(egui::Key::S, egui::Modifiers::NONE),
            egui::Event::Text("s".to_string()),
            press(egui::Key::Backspace, egui::Modifiers::NONE),
            press(egui::Key::ArrowDown, egui::Modifiers::NONE),
            press(egui::Key::Enter, egui::Modifiers::NONE),
            press(egui::Key::Escape, egui::Modifiers::NONE),
        ],
    );

    assert_eq!(
        keys,
        vec![plain(Key::Down), plain(Key::Enter), plain(Key::Esc)],
        "the dialog's list and buttons keys still go to the App"
    );
    assert_eq!(left.len(), 3, "the field keeps s, its text and Backspace");
}

#[test]
fn test_route_other_widget_keeps_typing_and_clipboard_only_with_a_selection() {
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let mut input = input_with(vec![
        press(egui::Key::A, egui::Modifiers::NONE),
        egui::Event::Text("a".to_string()),
        press(egui::Key::Escape, egui::Modifiers::NONE),
    ]);
    let keys = router.route(
        &mut input,
        Focus::Widget {
            has_selection: false,
        },
    );
    assert_eq!(keys, vec![plain(Key::Esc)], "only Esc reaches the App");
    assert_eq!(input.events.len(), 3, "the widget keeps its events");

    let ctrl_c = || {
        input_mods(
            vec![egui::Event::Copy],
            egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
        )
    };
    assert_eq!(
        router.route(
            &mut ctrl_c(),
            Focus::Widget {
                has_selection: true
            }
        ),
        Vec::new(),
        "Ctrl+C copies the focused field's selection"
    );
    assert_eq!(
        router.route(
            &mut ctrl_c(),
            Focus::Widget {
                has_selection: false
            }
        ),
        vec![KeyInput {
            key: Key::Char('c'),
            mods: Modifiers::CTRL,
        }],
        "without a selection Ctrl+C goes to the App, as from the prompt"
    );
}

#[test]
fn test_route_modal_text_field_keeps_ctrl_c_only_with_a_selection() {
    let ctrl_c = || {
        input_mods(
            vec![egui::Event::Copy],
            egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
        )
    };
    let mut router = KeyRouter::new(default_keymap(Platform::Linux));
    let with = Focus::Modal {
        field_focused: true,
        field_selection: true,
    };
    let without = Focus::Modal {
        field_focused: true,
        field_selection: false,
    };
    assert_eq!(router.route(&mut ctrl_c(), with), Vec::new());
    assert_eq!(
        router.route(&mut ctrl_c(), without),
        vec![KeyInput {
            key: Key::Char('c'),
            mods: Modifiers::CTRL,
        }]
    );
}

#[test]
fn test_route_question_mark_opens_shortcuts_only_on_an_empty_prompt() {
    let focus = |question_opens_shortcuts| Focus::Prompt {
        has_selection: false,
        completion_open: false,
        up_to_app: false,
        down_to_app: false,
        question_opens_shortcuts,
    };
    let events = || {
        vec![
            press(egui::Key::Questionmark, egui::Modifiers::SHIFT),
            egui::Event::Text("?".to_string()),
        ]
    };

    let (keys, left) = route_events(focus(true), events());
    assert_eq!(
        keys,
        vec![KeyInput {
            key: Key::Char('?'),
            mods: Modifiers::SHIFT,
        }]
    );
    assert!(left.is_empty(), "the `?` that opens the panel never types");

    let (keys, left) = route_events(focus(false), events());
    assert!(keys.is_empty(), "inside text `?` types");
    assert_eq!(left.len(), 2);
}
