//! Tests for the GUI dialogs (`codemason_gui::dialogs`): the permission
//! dialog, the model picker, the provider setup dialog and the `?` keyboard
//! shortcuts panel, plus the modal keyboard and focus rules around them.
//!
//! Every test drives the whole [`Shell`] frame, so the dialog dispatch, the
//! key router and app-core's key handling are exercised together. GUI
//! assertions use egui_kittest accessibility queries only (no snapshots).
//! Each test owns a tokio runtime that it does not enter while the harness
//! runs, because the shell drives `App::handle_key` with `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::event::Event;
use codemason_agent::permission::PermissionDecision;
use codemason_app_core::app::{App, ModelPickerEntry, ProviderSetupStep};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::dialogs::model_picker::{FILTER_LABEL, MODEL_PICKER_TITLE, MODELS_LABEL};
use codemason_gui::dialogs::permission::{ALLOW_ONCE, ALWAYS_ALLOW, DENY, PERMISSION_TITLE};
use codemason_gui::dialogs::provider::{CONTINUE, PROVIDER_CHOICE_LABEL, SELECT_PROVIDER_TITLE};
use codemason_gui::dialogs::shortcuts::SHORTCUTS_TITLE;
use codemason_gui::dialogs::{ActiveDialog, active};
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::prompt::PROMPT_LABEL;
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tokio::sync::mpsc::UnboundedSender;

/// The session the tests' permission requests belong to.
const SESSION: &str = "session-dialogs";

/// A shell over `app` with sources attached, the sender of its agent-event
/// queue, and the runtime driving it.
fn rig(
    configure: impl FnOnce(&mut App),
) -> (
    tokio::runtime::Runtime,
    UnboundedSender<Event>,
    Harness<'static, Shell>,
) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let app = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        configure(&mut app);
        app
    };
    let (events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 800.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, events_tx, harness)
}

/// The agent asks for permission, as `Event::PermissionRequested`.
fn request_permission(events: &UnboundedSender<Event>, id: &str) {
    events
        .send(Event::PermissionRequested {
            session_id: SESSION.to_string(),
            request_id: id.to_string(),
            permission: "bash".to_string(),
            description: "rm -rf target/temp/x".to_string(),
            options: Vec::new(),
        })
        .expect("send permission request");
}

/// The first `PermissionReplied` the App published on its bus.
fn reply(
    bus: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Option<(String, bool, PermissionDecision)> {
    while let Ok(event) = bus.try_recv() {
        if let Event::PermissionReplied {
            request_id,
            allowed,
            decision,
            ..
        } = event
        {
            return Some((request_id, allowed, decision));
        }
    }
    None
}

/// A press (or release) of `key` with no modifiers.
fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press and release `key` with its typed text, as egui-winit sends them.
fn type_key(harness: &mut Harness<'static, Shell>, key: egui::Key, text: Option<&str>) {
    let input = harness.input_mut();
    input.events.push(key_event(key, true));
    if let Some(text) = text {
        input.events.push(egui::Event::Text(text.to_string()));
    }
    input.events.push(key_event(key, false));
    harness.run();
}

/// A model picker entry with `id` and `name` and no thinking levels, so
/// Enter selects it directly.
fn model(id: &str, name: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "openai".to_string(),
        id: id.to_string(),
        name: name.to_string(),
        context_window: 128_000,
        max_output: None,
        cost_input: 1.0,
        cost_output: 2.0,
        reasoning: false,
        vision: false,
        tool_use: true,
        thinking_levels: Vec::new(),
        thinking_config: None,
        cost_tier: "Low".to_string(),
        cost_multiplier: "1x".to_string(),
    }
}

/// Open the model picker over three models, `GPT-4o` (between the two
/// Sonnets) highlighted.
fn open_model_picker(app: &mut App) {
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "openai".to_string(),
        provider_name: "OpenAI (GPT)".to_string(),
        models: vec![
            model("sonnet-4", "Claude Sonnet 4"),
            model("gpt-4o", "GPT-4o"),
            model("sonnet-3", "Claude Sonnet 3"),
        ],
        selected: 1,
    });
}

/// The selected model the App saved, as `provider/model`.
fn selected_model(harness: &Harness<'static, Shell>) -> Option<String> {
    harness.state().app().session.selected_model.clone()
}

#[test]
fn test_permission_dialog_allow_once_resolves_request() {
    let (_rt, events, mut harness) = rig(|_| {});
    let mut bus = harness.state().app().event_bus.subscribe();
    request_permission(&events, "req-1");
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, PERMISSION_TITLE)
            .is_some(),
        "the request shows the permission dialog"
    );

    harness.get_by_label(ALLOW_ONCE).click();
    harness.run();

    let (id, allowed, decision) = reply(&mut bus).expect("the App replied");
    assert_eq!(id, "req-1");
    assert!(allowed);
    assert_eq!(decision, PermissionDecision::Once);

    // The agent's reply comes back through the bus, as in the running app.
    events
        .send(Event::PermissionReplied {
            session_id: SESSION.to_string(),
            request_id: id,
            allowed,
            decision,
        })
        .expect("send reply");
    harness.run();
    assert!(harness.state().app().session.permission_queue.is_empty());
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, PERMISSION_TITLE)
            .is_none(),
        "the resolved request closes the dialog"
    );
}

#[test]
fn test_permission_dialog_always_and_deny_buttons_reply() {
    for (button, allowed, decision) in [
        (ALWAYS_ALLOW, true, PermissionDecision::Always),
        (DENY, false, PermissionDecision::Deny),
    ] {
        let (_rt, events, mut harness) = rig(|_| {});
        let mut bus = harness.state().app().event_bus.subscribe();
        request_permission(&events, "req-2");
        harness.run();

        harness.get_by_label(button).click();
        harness.run();

        assert_eq!(
            reply(&mut bus),
            Some(("req-2".to_string(), allowed, decision)),
            "{button}"
        );
    }
}

#[test]
fn test_permission_dialog_keys_answer_like_the_tui() {
    for (key, text, allowed, decision) in [
        (egui::Key::Y, "y", true, PermissionDecision::Once),
        (egui::Key::A, "a", true, PermissionDecision::Always),
        (egui::Key::N, "n", false, PermissionDecision::Deny),
    ] {
        let (_rt, events, mut harness) = rig(|_| {});
        let mut bus = harness.state().app().event_bus.subscribe();
        request_permission(&events, "req-3");
        harness.run();

        type_key(&mut harness, key, Some(text));

        assert_eq!(
            reply(&mut bus),
            Some(("req-3".to_string(), allowed, decision)),
            "key {text}"
        );
        assert_eq!(
            harness.state().app().session.input,
            "",
            "the answer key never types into the prompt"
        );
    }
}

#[test]
fn test_model_picker_filter_narrows_list() {
    let (_rt, _events, mut harness) = rig(open_model_picker);
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, MODEL_PICKER_TITLE)
            .is_some()
    );
    assert!(harness.query_by_label("GPT-4o").is_some());
    assert!(harness.query_by_label("Claude Sonnet 4").is_some());

    // Typed as egui-winit sends it: each press with its text.
    for (key, text) in [
        (egui::Key::S, "S"),
        (egui::Key::O, "O"),
        (egui::Key::N, "N"),
    ] {
        type_key(&mut harness, key, Some(text));
    }
    harness.run();

    assert_eq!(
        harness.get_by_label(FILTER_LABEL).value().as_deref(),
        Some("SON"),
        "the focused filter takes the typing"
    );
    assert!(
        harness.query_by_label("GPT-4o").is_none(),
        "the filter hides the models that do not match"
    );
    assert!(harness.query_by_label("Claude Sonnet 4").is_some());
    assert!(harness.query_by_label("Claude Sonnet 3").is_some());
    assert!(
        harness
            .query_by_role_and_label(Role::List, MODELS_LABEL)
            .is_some()
    );

    // The App's highlight left the hidden GPT-4o for the first shown row.
    assert!(matches!(
        harness.state().app().provider_setup,
        Some(ProviderSetupStep::SelectModel { selected: 0, .. })
    ));

    // Down moves within the shown rows (over the hidden GPT-4o); Enter
    // selects the highlighted one, through app-core's own Enter.
    type_key(&mut harness, egui::Key::ArrowDown, None);
    type_key(&mut harness, egui::Key::Enter, None);
    assert_eq!(
        selected_model(&harness).as_deref(),
        Some("openai/sonnet-3"),
        "the second shown model, not the hidden one between"
    );
}

#[test]
fn test_model_picker_enter_with_no_match_selects_nothing() {
    let (_rt, _events, mut harness) = rig(open_model_picker);
    harness.run();

    harness
        .get_by_label(FILTER_LABEL)
        .type_text("no such model");
    harness.run();
    harness.run();
    type_key(&mut harness, egui::Key::Enter, None);

    assert_eq!(selected_model(&harness), None);
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::ModelPicker),
        "the picker stays open"
    );
}

#[test]
fn test_model_picker_esc_closes_like_the_tui() {
    let (_rt, _events, mut harness) = rig(open_model_picker);
    harness.run();

    type_key(&mut harness, egui::Key::Escape, None);

    assert!(harness.state().app().provider_setup.is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, MODEL_PICKER_TITLE)
            .is_none()
    );
}

#[test]
fn test_provider_dialog_saves_selection() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.provider_setup = Some(ProviderSetupStep::SelectProvider {
            selected: 3,
            force_key_entry: false,
        });
    });
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, SELECT_PROVIDER_TITLE)
            .is_some()
    );

    // Choose Ollama in the provider combo box, then continue.
    let combo = harness.get_by_role_and_label(Role::ComboBox, PROVIDER_CHOICE_LABEL);
    assert_eq!(
        combo.value().as_deref(),
        Some("OpenAI (GPT)"),
        "the App's highlight"
    );
    combo.click();
    harness.run();
    harness.get_by_label("Ollama (Local)").click();
    harness.run();
    harness.get_by_label(CONTINUE).click();
    harness.run();

    let app = harness.state().app();
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::LoadingModels { ref provider_id, .. })
                if provider_id == "ollama"
        ),
        "the flow continues with the chosen provider: {:?}",
        app.provider_setup
    );
    assert!(
        App::get_configured_providers(&app.storage)
            .iter()
            .any(|p| p.id == "ollama"),
        "the chosen provider is saved as configured"
    );
}

#[test]
fn test_provider_dialog_typing_fills_the_key_field() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.provider_setup = Some(ProviderSetupStep::EnterKey {
            provider_id: "openai".to_string(),
            provider_name: "OpenAI (GPT)".to_string(),
            key_field: codemason_app_core::input_field::InputField::new(),
            endpoint_field: codemason_app_core::input_field::InputField::new(),
            active_field: 0,
            error: None,
        });
    });
    harness.run();

    type_key(&mut harness, egui::Key::K, Some("k"));
    type_key(&mut harness, egui::Key::Num1, Some("1"));

    let app = harness.state().app();
    let Some(ProviderSetupStep::EnterKey { ref key_field, .. }) = app.provider_setup else {
        panic!("still entering the key: {:?}", app.provider_setup);
    };
    assert_eq!(key_field.text(), "k1", "each key press types once");
    assert_eq!(app.session.input, "", "the prompt behind it is untouched");
}

#[test]
fn test_shortcuts_panel_lists_app_core_shortcuts_and_esc_closes() {
    let (_rt, _events, mut harness) = rig(|_| {});
    harness.run();

    type_key(&mut harness, egui::Key::Questionmark, Some("?"));

    assert!(harness.state().app().show_shortcuts);
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, SHORTCUTS_TITLE)
            .is_some()
    );
    for (keys, description) in codemason_app_core::shortcuts::KEYBINDINGS {
        assert!(
            harness.query_by_label(description).is_some(),
            "{keys}: {description}"
        );
    }

    type_key(&mut harness, egui::Key::Escape, None);
    assert!(!harness.state().app().show_shortcuts);
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, SHORTCUTS_TITLE)
            .is_none()
    );
}

#[test]
fn test_dialog_dispatch_follows_app_core_key_priority() {
    let (_rt, events, mut harness) = rig(|app| app.show_shortcuts = true);
    request_permission(&events, "req-4");
    harness.run();

    // `handle_key` serves the shortcuts panel before the permission request.
    assert_eq!(active(harness.state().app()), Some(ActiveDialog::Shortcuts));
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, PERMISSION_TITLE)
            .is_none()
    );

    type_key(&mut harness, egui::Key::Escape, None);
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::Permission)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, PERMISSION_TITLE)
            .is_some()
    );
}

#[test]
fn test_modal_typing_never_reaches_the_prompt() {
    let (_rt, events, mut harness) = rig(|_| {});
    request_permission(&events, "req-5");
    harness.run();

    type_key(&mut harness, egui::Key::X, Some("x"));

    let prompt = harness.get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL);
    assert!(!prompt.is_focused(), "the prompt does not take the focus");
    assert_eq!(harness.state().app().session.input, "");
}

#[test]
fn test_prompt_ime_preedit_is_dropped_when_a_modal_opens() {
    let (_rt, events, mut harness) = rig(|_| {});
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .type_text("ab");
    harness.run();
    harness.event(egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ni".to_string(),
        active_range_chars: None,
    }));
    harness.run();
    assert_eq!(
        harness.state().app().session.input,
        "abni",
        "the editor keeps the marked text in the buffer while composing"
    );

    request_permission(&events, "req-6");
    harness.run();

    let app = harness.state().app();
    assert_eq!(
        app.session.input, "ab",
        "the uncommitted composition is not left in the prompt"
    );
    assert_eq!(app.session.input_cursor, 2);
    assert!(
        !harness
            .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
            .is_focused(),
        "the dialog takes the keyboard from the prompt"
    );
}

#[test]
fn test_closing_a_dialog_gives_the_keyboard_back_to_the_prompt() {
    let (_rt, events, mut harness) = rig(|_| {});
    harness
        .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
        .focus();
    harness.run();
    request_permission(&events, "req-7");
    harness.run();
    type_key(&mut harness, egui::Key::Y, Some("y"));
    events
        .send(Event::PermissionReplied {
            session_id: SESSION.to_string(),
            request_id: "req-7".to_string(),
            allowed: true,
            decision: PermissionDecision::Once,
        })
        .expect("send reply");
    harness.run();
    harness.run();

    assert!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, PROMPT_LABEL)
            .is_focused()
    );
}
