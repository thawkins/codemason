//! Tests for the remaining GUI overlays (Task 3.3c, Part B): the plugin
//! store and connector catalogue browsers, the `/backend` switcher, the
//! `/mcp discover` and `/plugins list` dialogs, the model download and
//! loading windows, the right-click context menu and the tool-output
//! overlay.
//!
//! Each test drives the whole [`Shell`] frame: it checks the overlay is
//! drawn (egui_kittest accessibility queries only, no snapshots) and runs
//! one interaction through app-core. Each test owns a tokio runtime it does
//! not enter while the harness runs, because the shell drives the App with
//! `Handle::block_on`.

#[path = "support/mod.rs"]
mod support;

use codemason_agent::event::Event;
use codemason_agent::mcp::{DiscoveredMcpServer, McpDiscoverySource};
use codemason_agent::message::Message;
use codemason_app_core::app::{
    App, BackendPanelRow, BackendPanelState, McpDiscoverState, OutputViewLineCache,
    OutputViewState, OutputViewTarget, PluginStoreBrowser, PluginsListState, SelectionPane,
};
use codemason_app_core::clipboard::ClipboardTestOverrideGuard;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_connectors::CatalogueBrowser;
use codemason_gui::Shell;
use codemason_gui::dialogs::context_menu::CONTEXT_MENU_LABEL;
use codemason_gui::dialogs::lists::{
    BACKENDS_LABEL, CLOSE, MCP_DISCOVER_TITLE, PLUGINS_LIST_TITLE, backend_title,
};
use codemason_gui::dialogs::output_view::OUTPUT_LABEL;
use codemason_gui::dialogs::progress::{DOWNLOAD_LABEL, LOADING_LABEL};
use codemason_gui::dialogs::stores::{CONNECTORS_TITLE, RESULTS_LABEL, plugin_store_title};
use codemason_gui::dialogs::{ActiveDialog, active};
use codemason_gui::keys::default_keymap;
use codemason_gui::panels::messages::MESSAGES_LABEL;
use codemason_gui::panels::prompt::{PROMPT_LABEL, prompt_id};
use codemason_plugins::{PluginListRow, PluginListView, StoreKind};
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tokio::sync::mpsc::UnboundedSender;

/// The session the tests' messages belong to.
const SESSION: &str = "session-overlays";

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
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, events_tx, harness)
}

/// A press (or release) of `key` with `modifiers`.
fn key_event(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

/// Press and release `key` with its typed text, as egui-winit sends them.
fn type_key(harness: &mut Harness<'static, Shell>, key: egui::Key, text: Option<&str>) {
    let input = harness.input_mut();
    input
        .events
        .push(key_event(key, true, egui::Modifiers::NONE));
    if let Some(text) = text {
        input.events.push(egui::Event::Text(text.to_string()));
    }
    input
        .events
        .push(key_event(key, false, egui::Modifiers::NONE));
    harness.run();
}

/// Whether a dialog labelled `title` is drawn.
fn dialog_shown(harness: &Harness<'static, Shell>, title: &str) -> bool {
    harness
        .query_by_role_and_label(Role::Dialog, title)
        .is_some()
}

#[test]
fn test_plugin_store_renders_and_typing_edits_its_query() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    });
    harness.run();
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::PluginStore)
    );
    assert!(dialog_shown(&harness, &plugin_store_title("Codex")));
    harness.get_by_label_contains("Loading the store index");
    assert!(
        harness
            .query_by_role_and_label(Role::List, RESULTS_LABEL)
            .is_some()
    );

    type_key(&mut harness, egui::Key::A, Some("a"));
    type_key(&mut harness, egui::Key::B, Some("b"));
    let app = harness.state().app();
    assert_eq!(
        app.plugin_store.as_ref().map(|b| b.query.as_str()),
        Some("ab")
    );
    assert_eq!(app.session.input, "", "typing never reaches the prompt");
    harness.get_by_label_contains("search: ab");
}

#[test]
fn test_connector_store_renders_and_esc_edits_then_closes() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.connector_store = Some(CatalogueBrowser::new("", false));
    });
    harness.run();
    assert!(dialog_shown(&harness, CONNECTORS_TITLE));
    harness.get_by_label_contains("Loading the catalogue");

    type_key(&mut harness, egui::Key::X, Some("x"));
    assert_eq!(
        harness
            .state()
            .app()
            .connector_store
            .as_ref()
            .map(|b| b.query.as_str()),
        Some("x")
    );
    assert_eq!(harness.state().app().session.input, "");
    // Esc removes the query's last character, then closes on an empty one.
    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().connector_store.is_some());
    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().connector_store.is_none());
    assert!(!dialog_shown(&harness, CONNECTORS_TITLE));
}

/// A `/backend` row.
fn backend_row(id: &str, kind: &'static str, active: bool) -> BackendPanelRow {
    BackendPanelRow {
        id: id.to_string(),
        name: id.to_string(),
        kind,
        active,
        health: "ok",
        health_ok: true,
        detail: None,
        connection: format!("{id}-connection"),
    }
}

#[test]
fn test_backend_panel_renders_and_down_moves_the_cursor() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.backend_panel = Some(BackendPanelState {
            rows: vec![
                backend_row("local", "local", true),
                backend_row("docker", "docker", false),
            ],
            cursor: 0,
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, &backend_title("local")));
    assert!(
        harness
            .query_by_role_and_label(Role::List, BACKENDS_LABEL)
            .is_some()
    );
    harness.get_by_label_contains("docker-connection");

    type_key(&mut harness, egui::Key::ArrowDown, None);
    assert_eq!(
        harness
            .state()
            .app()
            .backend_panel
            .as_ref()
            .map(|p| p.cursor),
        Some(1)
    );
    harness.get_by_label(CLOSE).click();
    harness.run();
    assert!(harness.state().app().backend_panel.is_none());
}

#[test]
fn test_mcp_discover_renders_and_keys_move_and_close() {
    let (_rt, _events, mut harness) = rig(|app| {
        let server = |id: &str| DiscoveredMcpServer {
            id: id.to_string(),
            name: format!("{id} server"),
            executable: std::path::PathBuf::from("/usr/bin/true"),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            source: McpDiscoverySource::SystemPath,
            definition_file: None,
        };
        app.mcp_discover = Some(McpDiscoverState {
            servers: vec![server("alpha"), server("beta")],
            enabled: vec![false, false],
            selected: 0,
            scroll_offset: 0,
            feedback: None,
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, MCP_DISCOVER_TITLE));
    harness.get_by_label_contains("beta server");

    type_key(&mut harness, egui::Key::J, Some("j"));
    assert_eq!(
        harness
            .state()
            .app()
            .mcp_discover
            .as_ref()
            .map(|s| s.selected),
        Some(1)
    );
    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().mcp_discover.is_none());
    assert_eq!(harness.state().app().session.input, "");
}

#[test]
fn test_plugins_list_renders_and_down_moves_the_cursor() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.plugins_list = Some(PluginsListState {
            view: PluginListView {
                attribution: String::new(),
                header: Some("ID  ENABLED".to_string()),
                separator: None,
                body: vec!["first  yes".to_string(), "second  no".to_string()],
                rows: vec![
                    PluginListRow {
                        id: "first".to_string(),
                        enabled: true,
                    },
                    PluginListRow {
                        id: "second".to_string(),
                        enabled: false,
                    },
                ],
            },
            heading: None,
            selected: 0,
            scroll_offset: 0,
            verbose: false,
            feedback: None,
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, PLUGINS_LIST_TITLE));
    harness.get_by_label_contains("second  no");

    type_key(&mut harness, egui::Key::ArrowDown, None);
    assert_eq!(
        harness
            .state()
            .app()
            .plugins_list
            .as_ref()
            .map(|s| s.selected),
        Some(1)
    );
    harness.get_by_label(CLOSE).click();
    harness.run();
    assert!(harness.state().app().plugins_list.is_none());
}

#[test]
fn test_model_download_and_loading_windows_follow_agent_events() {
    let (_rt, events, mut harness) = rig(|_| {});
    let send = |event: Event| events.send(event).expect("send event");
    send(Event::ModelDownloadStarted {
        provider_id: "local".to_string(),
        model_id: "tiny-model".to_string(),
        session_id: SESSION.to_string(),
    });
    send(Event::ModelDownloadProgress {
        provider_id: "local".to_string(),
        model_id: "tiny-model".to_string(),
        session_id: SESSION.to_string(),
        percent: 42.0,
    });
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::ProgressIndicator, DOWNLOAD_LABEL)
            .is_some()
    );
    harness.get_by_label_contains("Downloading tiny-model");
    harness.get_by_label_contains("42.0%");

    send(Event::ModelDownloadFinished {
        provider_id: "local".to_string(),
        model_id: "tiny-model".to_string(),
        session_id: SESSION.to_string(),
        error: None,
    });
    send(Event::ProviderLoadingStarted {
        provider_id: "local".to_string(),
        provider_name: "Local".to_string(),
    });
    harness.run();
    assert!(
        harness
            .query_by_role_and_label(Role::ProgressIndicator, DOWNLOAD_LABEL)
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::ProgressIndicator, LOADING_LABEL)
            .is_some()
    );
    harness.get_by_label_contains("Loading models for Local");
}

/// Select the prompt's characters `start..end` in its editor.
fn select_in_prompt(harness: &mut Harness<'static, Shell>, start: usize, end: usize) {
    let mut state = egui::text_edit::TextEditState::default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(&harness.ctx, prompt_id());
    harness.state_mut().app_mut().session.input_cursor = end;
}

#[test]
fn test_context_menu_cut_acts_on_the_prompt_selection() {
    let _clipboard = ClipboardTestOverrideGuard::new("");
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.input = "hello world".to_string();
        app.session.input_cursor = 11;
    });
    harness.run();
    harness.get_by_label(PROMPT_LABEL).click();
    harness.run();
    select_in_prompt(&mut harness, 6, 11);
    harness.run();

    harness.get_by_label(PROMPT_LABEL).click_secondary();
    harness.run();
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::ContextMenu)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::MenuListPopup, CONTEXT_MENU_LABEL)
            .is_some()
    );
    let items = harness
        .state()
        .app()
        .context_menu
        .as_ref()
        .map(|m| m.items.clone())
        .expect("menu open");
    assert!(items[0].1, "Cut is enabled with a prompt selection");
    assert!(!items[2].1, "Paste is disabled with an empty clipboard");

    // Enter runs the highlighted item (Cut) on the prompt's selection.
    type_key(&mut harness, egui::Key::Enter, None);
    harness.run();
    let app = harness.state().app();
    assert!(app.context_menu.is_none());
    assert_eq!(app.session.input, "hello ");
}

#[test]
fn test_output_view_renders_scrolls_with_keys_and_esc_closes() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.messages = (0..300)
            .map(|i| Message::user_text(SESSION, format!("output line {i}")))
            .collect();
        app.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session {
                session_id: SESSION.to_string(),
                label: "build".to_string(),
            },
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: OutputViewLineCache::default(),
        });
    });
    harness.run();
    assert!(dialog_shown(&harness, "Output: build"));
    assert!(
        harness
            .query_by_role_and_label(Role::Log, OUTPUT_LABEL)
            .is_some()
    );
    // The focused session's transcript is behind the overlay too, so the
    // newest line shows twice: there and in the overlay.
    assert_eq!(
        harness
            .query_all_by_label_contains("output line 299")
            .count(),
        2
    );
    assert!(
        harness.query_by_label("output line 0").is_none(),
        "only the messages in view are laid out"
    );
    let max_scroll = harness
        .state()
        .app()
        .output_view
        .as_ref()
        .map_or(0, |v| v.max_scroll);
    assert!(max_scroll > 0, "the overlay measured its scroll range");

    // Ctrl+PageUp jumps to the start through app-core, and the view follows.
    let input = harness.input_mut();
    input
        .events
        .push(key_event(egui::Key::PageUp, true, egui::Modifiers::CTRL));
    input
        .events
        .push(key_event(egui::Key::PageUp, false, egui::Modifiers::CTRL));
    harness.run();
    let offset = harness
        .state()
        .app()
        .output_view
        .as_ref()
        .map(|v| (v.scroll_offset, v.max_scroll));
    assert!(
        offset.is_some_and(|(offset, max)| offset == max && max > 0),
        "scrolled to the start: {offset:?}"
    );
    // Only the overlay shows the oldest line; the transcript behind stays
    // at its bottom.
    harness.get_by_label("output line 0");

    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().output_view.is_none());
    assert!(!dialog_shown(&harness, "Output: build"));
}

#[test]
fn test_context_menu_on_the_transcript_has_nothing_enabled_and_esc_closes() {
    let _clipboard = ClipboardTestOverrideGuard::new("clipboard text");
    let (_rt, _events, mut harness) = rig(|app| {
        app.session.messages = vec![Message::user_text(SESSION, "a message")];
    });
    harness.run();
    harness
        .get_by_role_and_label(Role::Log, MESSAGES_LABEL)
        .click_secondary();
    harness.run();
    let items = harness
        .state()
        .app()
        .context_menu
        .as_ref()
        .map(|m| (m.pane, m.items.clone()))
        .expect("menu open");
    assert_eq!(items.0, SelectionPane::Messages);
    assert!(
        items.1.iter().all(|(_, enabled)| !enabled),
        "no selection and not the input: nothing to cut, copy or paste"
    );

    type_key(&mut harness, egui::Key::Escape, None);
    assert!(harness.state().app().context_menu.is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::MenuListPopup, CONTEXT_MENU_LABEL)
            .is_none()
    );
}

#[test]
fn test_overlay_dispatch_follows_app_core_key_priority() {
    let (_rt, _events, mut harness) = rig(|app| {
        app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Claude, "", false));
        app.show_shortcuts = true;
        app.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session {
                session_id: SESSION.to_string(),
                label: "build".to_string(),
            },
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: OutputViewLineCache::default(),
        });
    });
    harness.run();
    // The store takes keys before the shortcuts panel, as in app-core.
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::PluginStore)
    );
    assert!(dialog_shown(&harness, &plugin_store_title("Claude")));
    assert!(!dialog_shown(&harness, "Output: build"));

    // Esc on the empty query closes the store; the shortcuts panel is next.
    type_key(&mut harness, egui::Key::Escape, None);
    assert_eq!(active(harness.state().app()), Some(ActiveDialog::Shortcuts));
    // Esc closes the panel; the output overlay, last in the order, is next.
    type_key(&mut harness, egui::Key::Escape, None);
    assert_eq!(
        active(harness.state().app()),
        Some(ActiveDialog::OutputView)
    );
    assert!(dialog_shown(&harness, "Output: build"));
}

/// The stored (non-focused) session the tail tests show.
const STORED: &str = "session-overlays-stored";

/// Store message `i` of the stored session.
fn store_line(app: &App, i: usize) {
    app.storage
        .create_message(&Message::user_text(STORED, format!("stored line {i}")))
        .expect("store message");
}

/// A shell showing the output overlay over [`STORED`], holding `count`
/// stored messages.
fn stored_output_rig(
    count: usize,
) -> (
    tokio::runtime::Runtime,
    UnboundedSender<Event>,
    Harness<'static, Shell>,
) {
    rig(|app| {
        app.storage
            .create_session(STORED, "/")
            .expect("store session");
        for i in 0..count {
            store_line(app, i);
        }
        app.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session {
                session_id: STORED.to_string(),
                label: "sub-agent".to_string(),
            },
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: OutputViewLineCache::default(),
        });
    })
}

/// The output overlay's `(scroll_offset, max_scroll)`.
fn output_offsets(harness: &Harness<'static, Shell>) -> Option<(u16, u16)> {
    harness
        .state()
        .app()
        .output_view
        .as_ref()
        .map(|v| (v.scroll_offset, v.max_scroll))
}

#[test]
fn test_output_view_follows_the_tail_of_a_growing_stored_session() {
    let (_rt, _events, mut harness) = stored_output_rig(80);
    harness.run();
    assert!(dialog_shown(&harness, "Output: sub-agent"));
    harness.get_by_label("stored line 79");
    assert!(output_offsets(&harness).is_some_and(|(offset, max)| offset == 0 && max > 0));

    // The running agent stores more output: the pinned view shows it.
    for i in 80..83 {
        store_line(harness.state().app(), i);
    }
    harness.run();
    harness.get_by_label("stored line 82");
    assert_eq!(output_offsets(&harness).map(|(offset, _)| offset), Some(0));
}

#[test]
fn test_output_view_scrolled_up_keeps_its_offset_as_a_stored_session_grows() {
    let (_rt, _events, mut harness) = stored_output_rig(80);
    harness.run();
    type_key(&mut harness, egui::Key::PageUp, None);
    let scrolled = output_offsets(&harness).map(|(offset, _)| offset);
    assert!(scrolled.is_some_and(|offset| offset > 0), "{scrolled:?}");

    store_line(harness.state().app(), 80);
    harness.run();
    // As in the TUI: the offset from the bottom is kept, the view is not
    // pulled to the tail, so the new line is not shown.
    assert_eq!(output_offsets(&harness).map(|(offset, _)| offset), scrolled);
    assert!(harness.query_by_label("stored line 80").is_none());
}
