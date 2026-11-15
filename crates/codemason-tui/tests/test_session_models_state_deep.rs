//! Deep coverage for `app::session_ops`, `app::models` and `app::state`.
//!
//! These tests drive the `pub`/`pub(crate)` `App` methods of the three
//! modules through the observable surfaces the TUI actually uses: key events
//! ([`App::handle_key_event`]), slash commands
//! ([`App::execute_slash_command`]), mouse events, the render path and the
//! main-loop poll methods. Nothing under `src/` is modified.
//!
//! Tests that read or write the working directory or the config tree run
//! inside an isolated tempdir project (`slash_support::enter_project`), so a
//! test run never touches the developer's real `~/.config/codemason/` and never
//! writes into the repository. Env mutation is `unsafe` in edition 2024 and the
//! workspace denies `unsafe_code`, so this target opts back in explicitly; the
//! isolation harness holds the process-global mutex across the async body, so
//! `clippy::await_holding_lock` is allowed too.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use codemason_tui::App;
use codemason_tui::app::ProviderSetupStep;
use codemason_tui::layout;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project};

// ---------------------------------------------------------------------------
// harness helpers
// ---------------------------------------------------------------------------

/// A plain key event with no modifiers.

/// A key event carrying one modifier.

/// An app in a fresh isolated project, optionally seeded with config JSON.
fn isolated(
    config: &str,
) -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(config);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// The default isolated project: one agent, no provider overrides.
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    isolated(r#"{ "defaultAgent": "general" }"#)
}

/// Mark the app as fully configured so the send path passes its guards.

/// Type `text` into the prompt and press Enter.

/// Render the app once into a `width` x `height` test terminal.
fn render(app: &mut App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render");
}

/// A log entry with the given sequence number.

/// An empty (stale) log line cache group.

/// A plugin-store entry with the given id and install source.

/// Attach an in-memory code index rooted at `root`.

/// Poll `poll` until it reports progress, at most ~2s.

/// Close the provider-setup `Done` screen with any key, which is the only
/// surface that reaches `check_provider_health`.

// ===========================================================================
// session_ops.rs - dispatch paths
// ===========================================================================

// ===========================================================================
// session_ops.rs - code index background tasks
// ===========================================================================

// ===========================================================================
// session_ops.rs - plugin store / connector catalogue install paths
// ===========================================================================

// ===========================================================================
// session_ops.rs - editor / cursor / clipboard helpers
// ===========================================================================

#[test]
fn team_panel_render_syncs_member_session_ids() {
    let (_g, _e, temp, mut app) = isolated_app();
    let store = TeamStore::create("alpha", "slash-support", temp.path(), true).expect("team store");
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("child-session".to_string());
    member.status = MemberStatus::Working;
    member.current_task_id = Some("task-1".to_string());
    let mut config = store.config.clone();
    config.members.push(member);
    let updated = TeamStore {
        dir: store.dir.clone(),
        config,
    };
    updated.save().expect("save team");
    app.session.active_team = Some(updated.config.clone());
    app.session
        .team_members
        .push(TeamMember::new("writer", "tm-001", "general"));
    app.show_teams_window = true;

    render(&mut app, 120, 40);

    let member = app.session.team_members.first().expect("member");
    assert_eq!(member.session_id.as_deref(), Some("child-session"));
    assert_eq!(member.status, MemberStatus::Working);
    assert_eq!(member.current_task_id.as_deref(), Some("task-1"));
    assert!(
        !app.sid_to_display_name.is_empty(),
        "the session id is registered for log display"
    );
}

#[test]
fn team_panel_render_tolerates_a_missing_team_store() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.session.active_team = Some(TeamConfig::new("missing-team", "slash-support"));
    app.show_teams_window = true;

    render(&mut app, 120, 40);

    assert!(app.sid_to_display_name.is_empty());
}

// ===========================================================================
// models.rs - reports and model resolution
// ===========================================================================

/// Render the router setup panel so `router_model_picker_entry` and
/// `estimate_entry_cost`, whose branches depend on registry metadata, run.
#[test]
fn router_setup_panel_renders_model_metadata_and_cost() {
    let mut app = support::make_app();
    let mut draft = codemason_llm::providers::router_config::RouterConfig::default();
    draft.tiers.insert(
        "SIMPLE".to_string(),
        codemason_llm::providers::router_config::TierConfig {
            models: vec![codemason_llm::providers::router_config::TierEntry {
                provider_id: "gemini".to_string(),
                model: "gemini-2.5-pro-preview-05-06".to_string(),
            }],
            timeout_ms: None,
        },
    );
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers: Vec::new(),
        selected_provider_ids: Vec::new(),
        selected_provider_index: 0,
        draft_config: draft,
        active_bucket: codemason_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });

    render(&mut app, 120, 40);
}

// ===========================================================================
// state.rs - MCP enable, connector status, polls and housekeeping
// ===========================================================================
