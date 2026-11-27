//! Several teams run on one processor: each has its own `TeamManager`, events
//! route by team name, and teammate focus walks every team in pane order.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::event::Event;
use codemason_agent::team::{MemberStatus, TeamManager, TeamMember, TeamStore};
use codemason_app_core::app::App;
use support::make_app;

/// A fresh workspace under `target/temp/test_teams_multi/<test>`.
fn workspace(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_teams_multi")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".codemason")).expect("workspace");
    dir.canonicalize().expect("canonical workspace")
}

/// An app whose cwd is `ws`, with teams `alpha` (`alpha_members` teammates)
/// and `beta` (`beta_members`) created in `ws` and bound to the processor.
/// `alpha` is the session's active team.
fn app_with_teams(ws: &std::path::Path, alpha_members: usize, beta_members: usize) -> App {
    let mut app = make_app();
    app.cwd_path = ws.to_path_buf();
    app.cwd = ws.display().to_string();
    app.session.session_id = Some("lead-1".to_string());
    for (name, count) in [("alpha", alpha_members), ("beta", beta_members)] {
        let mut store = TeamStore::create(name, "lead-1", ws, true).expect("create team");
        for n in 1..=count {
            let mut member =
                TeamMember::new(format!("{name}-writer-{n}"), format!("tm-{n}"), "general");
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

#[tokio::test]
async fn test_teams_multi_events_route_to_own_team() {
    let ws = workspace("events_route");
    let mut app = app_with_teams(&ws, 1, 1);
    assert_eq!(app.session.team_members[0].status, MemberStatus::Working);

    app.handle_event(Event::TeammateIdle {
        session_id: "lead-1".to_string(),
        team_name: "beta".to_string(),
        agent_id: "tm-1".to_string(),
    })
    .await;

    assert_eq!(
        app.session.team_members[0].status,
        MemberStatus::Working,
        "alpha's member must not follow beta's event"
    );
    let log = app
        .log_entries
        .iter()
        .any(|e| e.message.contains("[beta]") && e.message.contains("tm-1"));
    assert!(log, "the event is logged under beta");
    assert_eq!(app.session_processor.team_managers.all().len(), 2);

    app.handle_event(Event::TeammateIdle {
        session_id: "lead-1".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-1".to_string(),
    })
    .await;
    assert_eq!(app.session.team_members[0].status, MemberStatus::Idle);
}

#[tokio::test]
async fn test_teams_multi_focus_cycles_across_teams() {
    let ws = workspace("focus_cycles");
    let mut app = app_with_teams(&ws, 2, 1);
    let mut visited = Vec::new();
    for _ in 0..3 {
        app.cycle_focused_teammate(true);
        visited.push(app.session.focused_teammate.clone().expect("focused"));
    }
    let expected: Vec<(String, String)> = [("alpha", "tm-1"), ("alpha", "tm-2"), ("beta", "tm-1")]
        .iter()
        .map(|(t, a)| (t.to_string(), a.to_string()))
        .collect();
    assert_eq!(visited, expected);
    app.cycle_focused_teammate(true);
    assert!(
        app.session.focused_teammate.is_none(),
        "past the end clears focus"
    );
}

#[tokio::test]
async fn test_teams_multi_sections_list_every_team() {
    let ws = workspace("sections");
    let app = app_with_teams(&ws, 2, 1);
    let sections = app.team_sections();
    let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta"]);
    assert_eq!(sections[0].members.len(), 2);
    assert_eq!(
        sections[1].members.len(),
        1,
        "beta's members come from its store"
    );
}

#[tokio::test]
async fn test_teams_multi_message_counters_follow_their_own_team() {
    let ws = workspace("message_counters");
    let mut app = app_with_teams(&ws, 1, 1);
    app.handle_event(Event::TeammateMessage {
        session_id: "lead-1".to_string(),
        team_name: "beta".to_string(),
        from: "tm-1".to_string(),
        to: "lead".to_string(),
        message_type: "notice".to_string(),
        preview: "hi".to_string(),
    })
    .await;
    let counts = &app.session.team_message_counts;
    let key = |team: &str| (team.to_string(), "tm-1".to_string());
    assert_eq!(counts.get(&key("beta")).copied(), Some((1, 0)));
    assert_eq!(counts.get(&key("alpha")), None, "alpha stays untouched");
}
