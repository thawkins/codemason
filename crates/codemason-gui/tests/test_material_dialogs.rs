//! The dialog sweep (MATERIALDIALOGS T8.3): every dialog `dialogs::active`
//! can return opens its own state and is drawn on a Material surface, with
//! the accessible role and label that surface gives it.
//!
//! One row per [`ActiveDialog`] variant, written as an exhaustive `match`
//! ([`case`]) so a new dialog cannot join the enum without a row here: the
//! compiler rejects the match until the variant is named, and the sweep then
//! fails until the variant's state is opened the way app-core opens it and a
//! labelled surface is found.
//!
//! The two menu surfaces are covered as menus rather than dialogs:
//! `SessionMenu` is drawn on a `Role::Menu` surface and the right-click
//! context menu on a `Role::MenuListPopup`, both on `material::menu`.
//!
//! Each case builds its own App, so no case's state can shadow another's in
//! the priority order `dialogs::active` uses.

#[path = "support/mod.rs"]
mod support;

use std::time::Duration;

use codemason_app_core::app::{
    App, BackendPanelRow, BackendPanelState, ConfigSavePickerState, HistoryPickerState,
    McpDiscoverState, MemoryViewState, ModelPickerEntry, OutputViewLineCache, OutputViewState,
    OutputViewTarget, PendingForceCleanup, PendingMemoryDelete, PendingRowAction,
    PlanApprovalState, PluginStoreBrowser, PluginsListState, ProviderSetupStep, ProviderSource,
    QuestionRequest, ROW_ACTION_CONFIRM_NO, RollbackOfferState, RowAction,
};
use codemason_app_core::geometry::Area;
use codemason_app_core::log_channel::tui_log_channel;
use codemason_app_core::palette::{PaletteScope, PaletteState};
use codemason_app_core::project_properties::{Choice, Field, ProjectProperties};
use codemason_app_core::session_menu::{MenuAction, MenuItem, MenuTarget, SessionMenu};
use codemason_app_core::theme_tokens::ThemeCatalog;
use codemason_config::permission::PermissionRequest;
use codemason_connectors::CatalogueBrowser;
use codemason_gui::Shell;
use codemason_gui::dialogs::ActiveDialog;
use codemason_gui::dialogs::active;
use codemason_gui::dialogs::confirm::{
    COMPACT_PRUNE_TITLE, FORCE_CLEANUP_TITLE, MEMORY_CLEAR_TITLE, MEMORY_DELETE_TITLE, PLAN_TITLE,
    QUEUE_CLEAR_TITLE, ROLLBACK_TITLE, ROUTER_SAVE_TITLE, db_prune_title, memory_view_title,
    row_action_title,
};
use codemason_gui::dialogs::context_menu::CONTEXT_MENU_LABEL;
use codemason_gui::dialogs::lists::{MCP_DISCOVER_TITLE, PLUGINS_LIST_TITLE, backend_title};
use codemason_gui::dialogs::loop_setup::LOOP_TITLE;
use codemason_gui::dialogs::model_picker::MODEL_PICKER_TITLE;
use codemason_gui::dialogs::palette::PALETTE_TITLE;
use codemason_gui::dialogs::permission::PERMISSION_TITLE;
use codemason_gui::dialogs::pickers::{CONFIG_SAVES_TITLE, HISTORY_TITLE, THEME_TITLE};
use codemason_gui::dialogs::properties::properties_title;
use codemason_gui::dialogs::provider::SELECT_PROVIDER_TITLE;
use codemason_gui::dialogs::question::QUESTION_TITLE;
use codemason_gui::dialogs::router::ROUTER_TITLE;
use codemason_gui::dialogs::session_menu::SESSION_MENU_TITLE;
use codemason_gui::dialogs::shortcuts::SHORTCUTS_TITLE;
use codemason_gui::dialogs::stores::{CONNECTORS_TITLE, PLUGIN_STORE_TITLE};
use codemason_gui::dialogs::theme_editor::editor_title;
use codemason_gui::keys::default_keymap;
use codemason_llm::providers::router_config::{RouterConfig, Tier};
use codemason_plugins::{PluginListRow, PluginListView, StoreKind};
use craft_keymap::Platform;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The session every case's state belongs to.
const SESSION: &str = "session-material-dialogs";

/// Frames run per case. A store browser keeps repainting while it loads, so
/// the sweep steps a fixed number of frames instead of settling.
const STEPS: usize = 4;

/// The kind of surface a dialog is drawn on, and so the accessible role it
/// carries its label on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    /// A Material dialog frame: `Role::Dialog` labelled with the title.
    Dialog,
    /// A Material menu: `Role::Menu` labelled with the menu's name.
    Menu,
    /// A Material menu opened as a popup: `Role::MenuListPopup`.
    MenuPopup,
}

/// One row of the sweep: which dialog, on which surface, and how its state is
/// opened (returning the label that surface must carry).
struct Case {
    /// The variant this row sweeps.
    dialog: ActiveDialog,
    /// The role its label is on.
    surface: Surface,
    /// Opens the state app-core opens for this dialog and returns the label
    /// the surface must carry.
    open: fn(&mut App) -> String,
}

/// The row for `dialog`. The `match` is exhaustive: a variant with no row
/// here does not compile.
fn case(dialog: ActiveDialog) -> Case {
    let row = |surface: Surface, open: fn(&mut App) -> String| Case {
        dialog,
        surface,
        open,
    };
    match dialog {
        ActiveDialog::ConfigSavePicker => row(Surface::Dialog, open_config_save_picker),
        ActiveDialog::Palette => row(Surface::Dialog, open_palette),
        ActiveDialog::ThemeEditor => row(Surface::Dialog, open_theme_editor),
        ActiveDialog::ThemePicker => row(Surface::Dialog, open_theme_picker),
        ActiveDialog::HistoryPicker => row(Surface::Dialog, open_history_picker),
        ActiveDialog::RouterSave => row(Surface::Dialog, open_router_save),
        ActiveDialog::PluginStore => row(Surface::Dialog, open_plugin_store),
        ActiveDialog::ConnectorStore => row(Surface::Dialog, open_connector_store),
        ActiveDialog::BackendPanel => row(Surface::Dialog, open_backend_panel),
        ActiveDialog::ContextMenu => row(Surface::MenuPopup, open_context_menu),
        ActiveDialog::Shortcuts => row(Surface::Dialog, open_shortcuts),
        ActiveDialog::Provider => row(Surface::Dialog, open_provider),
        ActiveDialog::ModelPicker => row(Surface::Dialog, open_model_picker),
        ActiveDialog::Router => row(Surface::Dialog, open_router),
        ActiveDialog::McpDiscover => row(Surface::Dialog, open_mcp_discover),
        ActiveDialog::PluginsList => row(Surface::Dialog, open_plugins_list),
        ActiveDialog::LoopSetup => row(Surface::Dialog, open_loop_setup),
        ActiveDialog::Question => row(Surface::Dialog, open_question),
        ActiveDialog::Permission => row(Surface::Dialog, open_permission),
        ActiveDialog::Notice => row(Surface::Dialog, open_notice),
        ActiveDialog::DbPrune => row(Surface::Dialog, open_db_prune),
        ActiveDialog::CompactPrune => row(Surface::Dialog, open_compact_prune),
        ActiveDialog::ForceCleanup => row(Surface::Dialog, open_force_cleanup),
        ActiveDialog::Rollback => row(Surface::Dialog, open_rollback),
        ActiveDialog::PlanApproval => row(Surface::Dialog, open_plan_approval),
        ActiveDialog::OutputView => row(Surface::Dialog, open_output_view),
        ActiveDialog::Properties => row(Surface::Dialog, open_properties),
        ActiveDialog::SessionMenu => row(Surface::Menu, open_session_menu),
        ActiveDialog::RowAction => row(Surface::Dialog, open_row_action),
        ActiveDialog::MemoryDelete => row(Surface::Dialog, open_memory_delete),
        ActiveDialog::MemoryClear => row(Surface::Dialog, open_memory_clear),
        ActiveDialog::QueueClear => row(Surface::Dialog, open_queue_clear),
        ActiveDialog::MemoryView => row(Surface::Dialog, open_memory_view),
    }
}

/// The sweep's variant list, in `ActiveDialog`'s own declaration order. Every
/// entry is asserted to be the row's own dialog, and [`case`] cannot be
/// written without a row per variant, so a variant missing from this list is
/// a variant whose row is never exercised - which
/// `test_material_dialog_sweep_is_exhaustive` catches.
const VARIANTS: &[ActiveDialog] = &[
    ActiveDialog::ConfigSavePicker,
    ActiveDialog::Palette,
    ActiveDialog::ThemeEditor,
    ActiveDialog::ThemePicker,
    ActiveDialog::HistoryPicker,
    ActiveDialog::RouterSave,
    ActiveDialog::PluginStore,
    ActiveDialog::ConnectorStore,
    ActiveDialog::BackendPanel,
    ActiveDialog::ContextMenu,
    ActiveDialog::Shortcuts,
    ActiveDialog::Provider,
    ActiveDialog::ModelPicker,
    ActiveDialog::Router,
    ActiveDialog::McpDiscover,
    ActiveDialog::PluginsList,
    ActiveDialog::LoopSetup,
    ActiveDialog::Question,
    ActiveDialog::Permission,
    ActiveDialog::Notice,
    ActiveDialog::DbPrune,
    ActiveDialog::CompactPrune,
    ActiveDialog::ForceCleanup,
    ActiveDialog::Rollback,
    ActiveDialog::PlanApproval,
    ActiveDialog::OutputView,
    ActiveDialog::Properties,
    ActiveDialog::SessionMenu,
    ActiveDialog::RowAction,
    ActiveDialog::MemoryDelete,
    ActiveDialog::MemoryClear,
    ActiveDialog::QueueClear,
    ActiveDialog::MemoryView,
];

/// A shell over an App `open` has prepared, and the label `open` returned.
fn rig(open: fn(&mut App) -> String) -> (tokio::runtime::Runtime, Harness<'static, Shell>, String) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (app, label) = {
        let _enter = runtime.enter();
        let mut app = support::make_app();
        app.session.session_id = Some(SESSION.to_string());
        // The permission and question answer delays read a frozen clock.
        app.advance_permission_clock_for_tests(Duration::ZERO);
        let label = open(&mut app);
        (app, label)
    };
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(16);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    (runtime, harness, label)
}

#[test]
fn test_every_dialog_is_drawn_on_a_material_surface() {
    for &dialog in VARIANTS {
        let row = case(dialog);
        assert_eq!(
            row.dialog, dialog,
            "the sweep's row for {dialog:?} opens another dialog"
        );
        let (_runtime, mut harness, label) = rig(row.open);
        harness.run_steps(STEPS);
        assert_eq!(
            active(harness.state().app()),
            Some(dialog),
            "{dialog:?} is open but the key router serves another surface"
        );
        let role = match row.surface {
            Surface::Dialog => Role::Dialog,
            Surface::Menu => Role::Menu,
            Surface::MenuPopup => Role::MenuListPopup,
        };
        assert!(
            harness.query_by_role_and_label(role, &label).is_some(),
            "{dialog:?} draws no {role:?} labelled {label:?}"
        );
    }
}

#[test]
fn test_material_dialog_sweep_is_exhaustive() {
    // Every variant has a row (the `match` in `case` is exhaustive), and no
    // variant is listed twice in the sweep.
    let mut seen: Vec<ActiveDialog> = VARIANTS.to_vec();
    seen.sort_by_key(|dialog| format!("{dialog:?}"));
    let count = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), count, "a variant is listed twice in the sweep");
    for &dialog in VARIANTS {
        assert_eq!(case(dialog).dialog, dialog);
    }
}

// ---------------------------------------------------------------------------
// One opener per dialog: the state app-core sets, and the label it expects
// ---------------------------------------------------------------------------

/// The `/config list` save picker.
fn open_config_save_picker(app: &mut App) -> String {
    app.config_save_picker = Some(ConfigSavePickerState {
        entries: vec![std::path::PathBuf::from("saves/codemason-1.json")],
        selected: 0,
        scroll_offset: 0,
        config_dir: std::path::PathBuf::from("target/temp/sweep-config"),
    });
    CONFIG_SAVES_TITLE.to_string()
}

/// The command palette (`Ctrl+P`).
fn open_palette(app: &mut App) -> String {
    app.themes = ThemeCatalog::builtin();
    app.palette = Some(PaletteState {
        query: String::new(),
        selected: 0,
        scope: PaletteScope::All,
    });
    PALETTE_TITLE.to_string()
}

/// The theme editor, opened the way the theme picker's Edit button does.
fn open_theme_editor(app: &mut App) -> String {
    app.themes = ThemeCatalog::builtin();
    app.open_theme_editor("nord");
    editor_title("nord")
}

/// The `/theme` picker.
fn open_theme_picker(app: &mut App) -> String {
    app.themes = ThemeCatalog::builtin();
    app.open_theme_picker();
    THEME_TITLE.to_string()
}

/// The input history picker.
fn open_history_picker(app: &mut App) -> String {
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["newest".to_string(), "older".to_string()],
        selected: 0,
        scroll_offset: 0,
    });
    HISTORY_TITLE.to_string()
}

/// The router configuration save confirmation.
fn open_router_save(app: &mut App) -> String {
    app.pending_router_save = Some(RouterConfig::default());
    ROUTER_SAVE_TITLE.to_string()
}

/// The plugin-store browser.
fn open_plugin_store(app: &mut App) -> String {
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    PLUGIN_STORE_TITLE.to_string()
}

/// The connector-catalogue browser.
fn open_connector_store(app: &mut App) -> String {
    app.connector_store = Some(CatalogueBrowser::new("", false));
    CONNECTORS_TITLE.to_string()
}

/// The `/backend` switcher.
fn open_backend_panel(app: &mut App) -> String {
    app.backend_panel = Some(BackendPanelState {
        rows: vec![BackendPanelRow {
            id: "local".to_string(),
            name: "local".to_string(),
            kind: "local",
            active: true,
            health: "ok",
            health_ok: true,
            detail: None,
            connection: "local-connection".to_string(),
        }],
        cursor: 0,
    });
    backend_title("local")
}

/// The right-click Cut/Copy/Paste menu on a prompt selection.
fn open_context_menu(app: &mut App) -> String {
    app.session.input = "hello world".to_string();
    app.session.input_cursor = 11;
    app.open_context_menu(0, 0, codemason_app_core::app::SelectionPane::Input, true);
    CONTEXT_MENU_LABEL.to_string()
}

/// The `?` keyboard shortcuts panel.
fn open_shortcuts(app: &mut App) -> String {
    app.show_shortcuts = true;
    SHORTCUTS_TITLE.to_string()
}

/// The provider setup flow's provider list.
fn open_provider(app: &mut App) -> String {
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: 0,
        force_key_entry: false,
    });
    SELECT_PROVIDER_TITLE.to_string()
}

/// The provider setup flow's model list.
fn open_model_picker(app: &mut App) -> String {
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "openai".to_string(),
        provider_name: "OpenAI (GPT)".to_string(),
        models: vec![model("gpt-4o", "GPT-4o")],
        selected: 0,
    });
    MODEL_PICKER_TITLE.to_string()
}

/// The provider setup flow's Model Router setup.
fn open_router(app: &mut App) -> String {
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers: vec![codemason_app_core::app::ConfiguredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            source: ProviderSource::Database,
        }],
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: RouterConfig::default(),
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    ROUTER_TITLE.to_string()
}

/// A model entry for the provider setup flow's model list.
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

/// The `/mcp discover` dialog, with nothing discovered.
fn open_mcp_discover(app: &mut App) -> String {
    app.mcp_discover = Some(McpDiscoverState {
        servers: Vec::new(),
        enabled: Vec::new(),
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    });
    MCP_DISCOVER_TITLE.to_string()
}

/// The `/plugins list` dialog.
fn open_plugins_list(app: &mut App) -> String {
    app.plugins_list = Some(PluginsListState {
        view: PluginListView {
            attribution: String::new(),
            header: Some("ID  ENABLED".to_string()),
            separator: None,
            body: vec!["first  yes".to_string()],
            rows: vec![PluginListRow {
                id: "first".to_string(),
                enabled: true,
            }],
        },
        heading: None,
        selected: 0,
        scroll_offset: 0,
        verbose: false,
        feedback: None,
    });
    PLUGINS_LIST_TITLE.to_string()
}

/// The `/loop setup` dialog.
fn open_loop_setup(app: &mut App) -> String {
    codemason_app_core::app::open_loop_setup(app);
    LOOP_TITLE.to_string()
}

/// The agent's front question.
fn open_question(app: &mut App) -> String {
    app.session.question_queue.push_back(QuestionRequest {
        id: "q-sweep".to_string(),
        session_id: SESSION.to_string(),
        question: "Which colour?".to_string(),
        options: vec!["red".to_string(), "green".to_string()],
        default_index: Some(0),
        expire_at_ms: None,
        secret: false,
    });
    QUESTION_TITLE.to_string()
}

/// The front permission request.
fn open_permission(app: &mut App) -> String {
    app.session.permission_queue.push_back(PermissionRequest {
        id: "perm-sweep".to_string(),
        session_id: SESSION.to_string(),
        permission: "bash".to_string(),
        patterns: vec!["rm -rf target/temp/x".to_string()],
        metadata: serde_json::Value::Null,
        tool_call_id: None,
    });
    PERMISSION_TITLE.to_string()
}

/// The single-button notice, whose title is the App's trimmed one.
fn open_notice(app: &mut App) -> String {
    app.notice_open = true;
    app.notice_title = " Worktree missing ".to_string();
    app.notice_message = "The session's worktree no longer exists.".to_string();
    app.notice_detail = None;
    "Worktree missing".to_string()
}

/// The `/db prune` confirmation.
fn open_db_prune(app: &mut App) -> String {
    app.open_db_prune_confirm(30);
    db_prune_title(30)
}

/// The `/compact prune --all` confirmation.
fn open_compact_prune(app: &mut App) -> String {
    app.open_compact_prune_confirm();
    COMPACT_PRUNE_TITLE.to_string()
}

/// The team force-cleanup confirmation.
fn open_force_cleanup(app: &mut App) -> String {
    app.session.pending_forcecleanup = Some(PendingForceCleanup {
        team_name: "alpha".to_string(),
        active_members: vec!["builder".to_string()],
    });
    FORCE_CLEANUP_TITLE.to_string()
}

/// The post-loop rollback offer.
fn open_rollback(app: &mut App) -> String {
    app.session.pending_rollback = Some(RollbackOfferState {
        session_id: SESSION.to_string(),
        status: "budget_exhausted".to_string(),
        iterations: 7,
        diffstat: "2 files changed".to_string(),
        files: vec!["src/lib.rs".to_string()],
    });
    ROLLBACK_TITLE.to_string()
}

/// The plan approval.
fn open_plan_approval(app: &mut App) -> String {
    app.session.session_id = None;
    app.session.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "1. Read the code".to_string(),
        cursor_approve: false,
    });
    PLAN_TITLE.to_string()
}

/// The tool-output overlay.
fn open_output_view(app: &mut App) -> String {
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: SESSION.to_string(),
            label: "build".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    "Output: build".to_string()
}

/// The project properties dialog.
fn open_properties(app: &mut App) -> String {
    let global = |label: &str| Choice {
        label: label.to_string(),
        value: None,
        enabled: true,
    };
    app.sidebar.properties = Some(ProjectProperties {
        project_id: "p-1".to_string(),
        project_name: "demo".to_string(),
        description: String::new(),
        caret: 0,
        backends: vec![global("Global (local)")],
        backend: 0,
        models: vec![global("Global (none)")],
        model: 0,
        focus: Field::Description,
        list: None,
        error: None,
        area: Area::default(),
    });
    properties_title("demo")
}

/// The sidebar's session menu.
fn open_session_menu(app: &mut App) -> String {
    app.sidebar.selected.insert("s-a".to_string());
    app.sidebar.menu = Some(SessionMenu {
        target: MenuTarget::Selection,
        row_y: 0,
        items: vec![MenuItem {
            label: "Clear selection".to_string(),
            accel: Some('c'),
            action: MenuAction::ClearSelection,
            group: 0,
            danger: false,
            enabled: true,
        }],
        cursor: 0,
        area: Area::default(),
        scroll: 0,
        submenu: None,
    });
    SESSION_MENU_TITLE.to_string()
}

/// The destructive row-action confirmation.
fn open_row_action(app: &mut App) -> String {
    app.row_action_confirm = Some(PendingRowAction {
        action: RowAction::KillSubagent {
            task_id: "task-sweep".to_string(),
            agent_name: "builder".to_string(),
        },
        selected: ROW_ACTION_CONFIRM_NO,
    });
    row_action_title(app).unwrap_or_default()
}

/// The Memory panel's delete confirmation.
fn open_memory_delete(app: &mut App) -> String {
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id: 7,
        preview: "a stored fact".to_string(),
    });
    MEMORY_DELETE_TITLE.to_string()
}

/// The `/memory clear` confirmation.
fn open_memory_clear(app: &mut App) -> String {
    app.open_memory_clear_confirm();
    MEMORY_CLEAR_TITLE.to_string()
}

/// The `Clear the input queue?` confirmation, armed the way the Alt+Q menu's
/// `Clear` row arms it.
fn open_queue_clear(app: &mut App) -> String {
    app.session
        .input_queue
        .push_back(codemason_app_core::app::QueuedInput {
            text: "a queued prompt".to_string(),
            image_paths: Vec::new(),
        });
    app.queue_clear_confirm_open = true;
    app.queue_clear_confirm_selected = codemason_app_core::app::QUEUE_CLEAR_CONFIRM_NO;
    QUEUE_CLEAR_TITLE.to_string()
}

/// The full-memory view.
fn open_memory_view(app: &mut App) -> String {
    app.memory_view = Some(MemoryViewState {
        row: codemason_storage::storage::MemoryRow {
            id: 7,
            content: "a stored fact".to_string(),
            category: "fact".to_string(),
            source: "sweep".to_string(),
            confidence: 0.7,
            project: ".".to_string(),
            session_id: SESSION.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_count: 0,
            last_accessed: None,
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: OutputViewLineCache::default(),
    });
    memory_view_title(7, "fact")
}
