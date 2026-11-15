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
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_session_models_state_deep.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use codemason_app_core::app::App;

#[path = "../../codemason-app-core/tests/slash_support/mod.rs"]
mod slash_support;
#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project};

// ---------------------------------------------------------------------------
// harness helpers
// ---------------------------------------------------------------------------

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

/// Render the app once into a `width` x `height` test terminal.
fn render(app: &mut App, _width: u16, _height: u16) {
    crate::frame::publish(app);
}

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

// ===========================================================================
// state.rs - MCP enable, connector status, polls and housekeeping
// ===========================================================================
