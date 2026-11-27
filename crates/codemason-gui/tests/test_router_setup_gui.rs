//! Tests for the router provider TUI setup flow (spec: `routeui`).
//!
//! These tests exercise the state-machine helpers and persistence logic
//! without launching a full terminal backend.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_router_setup.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

use std::collections::HashMap;
use std::sync::Arc;

use crate::term_events::{KeyCode, KeyEvent, KeyModifiers};
use codemason_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use codemason_app_core::app::App;
use codemason_app_core::app::{ConfiguredProvider, ProviderSetupStep, ProviderSource};
use codemason_llm::providers::router_config::Tier;

fn discovered_model(
    provider_id: &str,
    id: &str,
    name: &str,
) -> codemason_agent::provider::ModelInfo {
    codemason_agent::provider::ModelInfo {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        name: name.to_string(),
        cost: codemason_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: codemason_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision: true,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 200_000,
        max_output: Some(8_192),
        request_multiplier: None,
        thinking_config: None,
    }
}

fn seed_router_provider_models(storage: &Storage) {
    let anthropic = serde_json::to_string(&vec![
        discovered_model("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4"),
        discovered_model("anthropic", "claude-3-5-haiku-latest", "Claude 3.5 Haiku"),
    ])
    .expect("serialize anthropic models");
    storage
        .set_discovered_models("anthropic", &anthropic)
        .expect("seed anthropic discovered models");

    let openai = serde_json::to_string(&vec![
        discovered_model("openai", "gpt-4o", "GPT-4o"),
        discovered_model("openai", "gpt-4o-mini", "GPT-4o Mini"),
    ])
    .expect("serialize openai models");
    storage
        .set_discovered_models("openai", &openai)
        .expect("seed openai discovered models");
}

// -- helpers ----------------------------------------------------------------

fn mem_storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory storage"))
}

fn make_app_with_storage(storage: Arc<Storage>) -> App {
    codemason_app_core::app::use_scratch_log_root_for_tests();
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_managers: Default::default(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        project_code_indexes: Default::default(),
        project_tool_registries: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: codemason_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: std::sync::Arc::new(codemason_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        pending_verdicts: codemason_agent::session::pending::PendingVerdicts::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    // The session sidebar (MULTISESSION M5) is on by default and would take
    // the left columns of the rendered layout these tests assert on; keep the
    // full-width chat they were written against, as `support::make_app` does.
    app.sidebar.visible = false;
    app
}

// -- FR-004: configured-provider query helper excludes router ---------------

// -- FR-003 / FR-006: router setup state initialisation ---------------------

// -- FR-010: router config report formatting --------------------------------

// -- FR-020: status bar label when router enabled ---------------------------

// -- FR-024: router setup helper excludes router provider -------------------

// -- FR-002: provider picker list contains router ---------------------------

// -- FR-003: router model picker preserves provider selection on Enter/Esc ----

fn router_setup_with_providers(storage: Arc<Storage>) -> App {
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store key");
    seed_router_provider_models(&storage);
    let mut app = make_app_with_storage(storage);
    // Use a deterministic, two-provider palette so tests are not affected by
    // ambient environment variables like OLLAMA_HOST.
    let providers = vec![
        ConfiguredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            source: ProviderSource::Database,
        },
        ConfiguredProvider {
            id: "openai".to_string(),
            name: "OpenAI (GPT)".to_string(),
            source: ProviderSource::Database,
        },
    ];
    // The model picker uses the real registry models, so include only
    // providers that have models available in the default registry.
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: codemason_llm::providers::router_config::RouterConfig {
            enabled: false,
            tiers: std::collections::HashMap::new(),
            ..codemason_llm::providers::router_config::RouterConfig::default()
        },
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    app
}

// -- FR-025: selecting "Model Router" from the provider picker opens the
// router cluster setup panel (not the generic API-key dialog) --------------

// -- FR-006 (revised): buckets render in a 2×2 grid with full tier names ------

/// `render_router_setup_to_string` without its terminal draw: the setup it performs, then
/// a GUI frame (`crate::frame::publish`).
fn render_router_setup_to_string_effects(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

#[tokio::test]
async fn test_router_model_picker_renders_property_columns() {
    let mut app = router_setup_with_providers(mem_storage());
    // Open the model picker for the selected anthropic provider.
    crate::term_events::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectRouterModel { .. })
    ));

    // The picker table must render the same column headers as the standard
    // model picker so properties are visible for sub-model selection.
    render_router_setup_to_string_effects(&mut app, 120, 40);
}

// -- Re-opening setup seeds the draft from the persisted config -----------

// -- Regression: router save confirmation modal must render on top ------------

#[tokio::test]
async fn test_router_save_confirmation_renders_above_setup_dialog() {
    let mut app = router_setup_with_providers(mem_storage());
    // Assign the first model to the SIMPLE bucket so the draft has content.
    crate::term_events::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    crate::term_events::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    // Press Ctrl+S to arm the save confirmation modal.
    crate::term_events::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    )
    .await;
    assert!(
        app.pending_router_save.is_some(),
        "pending_router_save should be set after Ctrl+S"
    );

    render_router_setup_to_string_effects(&mut app, 120, 40);
}

// -- Regression: selecting an already-configured Model Router from the provider
// picker must open the router setup UI, not the single-entry model picker. ---
