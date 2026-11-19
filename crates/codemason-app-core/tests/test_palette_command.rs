//! Tests for the command palette model (`codemason_app_core::palette`).

use codemason_app_core::input_event::{Key, KeyInput, Modifiers};
use codemason_app_core::palette::{PALETTE_MAX_ROWS, palette_entries};
use codemason_app_core::theme_tokens::{ThemeCatalog, ThemeSelection};

mod support;

fn app() -> codemason_app_core::app::App {
    let mut app = support::make_app();
    app.themes = ThemeCatalog::builtin();
    app.theme = ThemeSelection::System;
    app
}

async fn press(app: &mut codemason_app_core::app::App, key: Key) {
    let _ = app
        .handle_key(KeyInput {
            key,
            mods: Modifiers::default(),
        })
        .await;
}

#[test]
fn test_palette_lists_every_kind() {
    let app = app();
    let entries = palette_entries(&app);
    for prefix in ["action:", "slash:", "theme:", "agent:"] {
        assert!(entries.iter().any(|e| e.id.starts_with(prefix)), "{prefix}");
    }
    let log = entries
        .iter()
        .find(|e| e.id == "action:panel.log")
        .expect("action");
    assert_eq!(log.key.as_deref(), Some("Alt+L"));
    assert!(entries.iter().any(|e| e.id == "theme:nord"));
}

#[tokio::test]
async fn test_palette_query_ranks_and_enter_runs_the_row() {
    let mut app = app();
    app.open_palette();
    app.set_palette_query("theme nord");
    let results = app.palette_results();
    assert_eq!(results.first().map(|e| e.id.as_str()), Some("theme:nord"));
    assert!(results.len() <= PALETTE_MAX_ROWS);
    press(&mut app, Key::Enter).await;
    assert!(app.palette.is_none(), "Enter closes the palette");
    assert_eq!(app.theme, ThemeSelection::Named("nord".to_string()));
}

#[tokio::test]
async fn test_palette_keys_move_and_esc_closes_without_change() {
    let mut app = app();
    app.open_palette();
    app.set_palette_query("theme");
    press(&mut app, Key::Down).await;
    assert_eq!(app.palette.as_ref().map(|p| p.selected), Some(1));
    press(&mut app, Key::Up).await;
    press(&mut app, Key::Up).await;
    assert_eq!(app.palette.as_ref().map(|p| p.selected), Some(0));
    press(&mut app, Key::Esc).await;
    assert!(app.palette.is_none());
    assert_eq!(app.theme, ThemeSelection::System);
}

#[tokio::test]
async fn test_palette_slash_rows_run_or_fill_the_prompt() {
    let mut app = app();
    // `/theme` takes arguments: it goes to the prompt to finish.
    app.run_palette_entry("slash:theme").await;
    assert_eq!(app.session.input, "/theme ");
    // An action with a terminal key runs as that key.
    app.session.input.clear();
    app.input_edited(0);
    let shown = app.show_log;
    app.run_palette_entry("action:panel.log").await;
    assert_ne!(app.show_log, shown);
}

#[tokio::test]
async fn test_palette_agent_picker_lists_agents_and_switches() {
    let mut app = app();
    assert!(app.run_action("picker.agent").await);
    assert_eq!(
        app.palette.as_ref().map(|p| p.scope),
        Some(codemason_app_core::palette::PaletteScope::Agent)
    );
    let results = app.palette_results();
    assert_ne!(results.len(), 0, "agents are listed");
    assert!(results.iter().all(|e| e.id.starts_with("agent:")));
    let target = app
        .cycleable_agents
        .iter()
        .map(|a| a.name.clone())
        .find(|name| *name != app.session.agent_name)
        .expect("a second agent");
    app.set_palette_query(&target);
    assert_eq!(
        app.palette_results().first().map(|e| e.id.clone()),
        Some(format!("agent:{target}"))
    );
    press(&mut app, Key::Enter).await;
    assert!(app.palette.is_none());
    assert_eq!(app.session.agent_name, target);
}

#[tokio::test]
async fn test_palette_session_picker_lists_sessions_and_switches() {
    let mut app = app();
    app.execute_slash_command("/session new").await;
    let current = app.session.session_id.clone().expect("focused session");
    let other = app
        .background
        .iter()
        .find_map(|v| v.session_id.clone())
        .expect("background session");
    codemason_app_core::sessions_sidebar_state::refresh_if_dirty(&mut app);
    assert!(app.run_action("picker.session").await);
    let results = app.palette_results();
    assert!(results.iter().all(|e| e.id.starts_with("session:")));
    let row = results
        .iter()
        .position(|e| e.id == format!("session:{other}"))
        .expect("the other session is listed");
    app.palette.as_mut().expect("open").selected = row;
    press(&mut app, Key::Enter).await;
    assert!(app.palette.is_none());
    assert_eq!(app.session.session_id.as_deref(), Some(other.as_str()));
    assert_ne!(other, current);
}

#[tokio::test]
async fn test_palette_slash_waits_for_the_turn_and_keeps_the_draft() {
    let mut app = app();
    app.session.input = "half-written".to_string();
    app.session.input_cursor = 4;
    app.session.is_processing = true;
    app.run_palette_entry("slash:clear").await;
    let queued: Vec<&str> = app
        .session
        .input_queue
        .iter()
        .map(|q| q.text.as_str())
        .collect();
    assert_eq!(queued, ["/clear"], "queued for the turn boundary, not run");
    assert_eq!(app.session.input, "half-written");
    assert_eq!(app.session.input_cursor, 4);
}

#[tokio::test]
async fn test_palette_editor_keeps_the_draft_for_the_editor() {
    let mut app = app();
    app.session.input = "draft to edit".to_string();
    app.run_palette_entry("slash:editor").await;
    assert_eq!(app.take_frontend_action().as_deref(), Some("prompt.editor"));
    assert_eq!(app.session.input, "draft to edit");
}
