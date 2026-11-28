//! The Teams pane lists every team the session runs, one section each.

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::team::{MemberStatus, TeamManager, TeamMember, TeamStore};
use codemason_app_core::app::App;
use codemason_gui::palette::palette_for;
use codemason_gui::panels::teams::team_rows;

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

/// An app with teams `alpha` (2 teammates) and `beta` (1), both bound, under
/// `target/temp/test_teams_multi_gui/<test>`; `alpha` is the active team.
fn app_with_two_teams(test: &str) -> App {
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_teams_multi_gui")
        .join(test);
    let _ = std::fs::remove_dir_all(&ws);
    std::fs::create_dir_all(ws.join(".codemason")).expect("workspace");
    let ws = ws.canonicalize().expect("canonical workspace");
    let mut app = support::make_app();
    app.cwd_path = ws.clone();
    app.cwd = ws.display().to_string();
    app.session.session_id = Some("lead-1".to_string());
    for (name, count) in [("alpha", 2), ("beta", 1)] {
        let mut store = TeamStore::create(name, "lead-1", &ws, true).expect("create team");
        for n in 1..=count {
            let mut member = TeamMember::new(format!("{name}-w{n}"), format!("tm-{n}"), "general");
            member.status = MemberStatus::Working;
            store.config.members.push(member);
        }
        store.save().expect("save team");
        let manager = TeamManager::new(
            name,
            "lead-1",
            store.dir.clone(),
            Arc::clone(&app.session_processor),
            app.event_bus.clone(),
        )
        .into_arc();
        assert!(app.session_processor.team_managers.bind(manager).is_ok());
        if name == "alpha" {
            app.session.team_members = store.config.members.clone();
            app.session.active_team = Some(store.config.clone());
        }
    }
    app
}

#[test]
fn test_teams_multi_rows_list_every_team() {
    let app = app_with_two_teams("rows");
    let palette = palette_for(&app, &egui::Context::default());
    let rows = team_rows(&app, &palette);
    let titles: Vec<String> = rows
        .iter()
        .map(|(line, _)| line.text())
        .filter(|text| text.starts_with("Team: "))
        .collect();
    assert_eq!(titles.len(), 2, "one title row per team: {titles:?}");
    assert!(titles[0].contains("alpha") && titles[0].contains("lead + 2 teammates"));
    assert!(titles[1].contains("beta") && titles[1].contains("lead + 1 teammate)"));
    let teammate_keys: Vec<(String, String)> = rows
        .iter()
        .map(|(_, key)| key.clone())
        .filter(|(_, id)| id.starts_with("tm-"))
        .collect();
    assert_eq!(
        teammate_keys,
        [("alpha", "tm-1"), ("alpha", "tm-2"), ("beta", "tm-1")]
            .map(|(t, a)| (t.to_string(), a.to_string()))
    );
}

#[test]
fn test_teams_multi_painted_keys_span_both_teams() {
    let mut app = app_with_two_teams("painted");
    app.show_teams_window = true;
    crate::frame::publish(&mut app);
    assert_eq!(
        app.teams_teammate_agent_ids,
        [("alpha", "tm-1"), ("alpha", "tm-2"), ("beta", "tm-1")]
            .map(|(t, a)| (t.to_string(), a.to_string()))
    );
}
