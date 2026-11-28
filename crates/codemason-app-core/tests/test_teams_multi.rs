//! Several teams run on one processor: each has its own `TeamManager`, events
//! route by team name, and teammate focus walks every team in pane order.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::event::Event;
use codemason_agent::team::{MemberStatus, TeamManager, TeamMember, TeamStore};
use codemason_app_core::app::App;
use codemason_storage::SessionKind;
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

/// An app whose cwd is `ws` with one primary session and two team-leader
/// child sessions of it: `alpha_lead` leads team `alpha` (`alpha_members`
/// teammates) and `beta_lead` leads `beta` (`beta_members`), both created in
/// `ws` and bound to the processor. `alpha` is the primary's active team, as
/// the team-leader model has it: the primary never leads a team itself.
struct Teams {
    app: App,
    alpha_lead: String,
    beta_lead: String,
}

fn app_with_teams(ws: &std::path::Path, alpha_members: usize, beta_members: usize) -> Teams {
    let mut app = make_app();
    app.cwd_path = ws.to_path_buf();
    app.cwd = ws.display().to_string();
    let sessions = &app.session_processor.session_manager;
    let primary = sessions
        .create_session(ws.to_path_buf())
        .expect("primary session")
        .id;
    let mut leads = Vec::new();
    for _ in 0..2 {
        leads.push(
            sessions
                .create_child_session(ws.to_path_buf(), &primary, SessionKind::Subagent)
                .expect("leader session")
                .id,
        );
    }
    app.session.session_id = Some(primary);
    for ((name, count), lead) in [("alpha", alpha_members), ("beta", beta_members)]
        .into_iter()
        .zip(&leads)
    {
        let mut store = TeamStore::create(name, lead, ws, true).expect("create team");
        for n in 1..=count {
            let mut member =
                TeamMember::new(format!("{name}-writer-{n}"), format!("tm-{n}"), "general");
            member.status = MemberStatus::Working;
            store.config.members.push(member);
        }
        store.save().expect("save team");
        let manager = TeamManager::new(
            name,
            lead,
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
    let beta_lead = leads.pop().expect("beta lead");
    let alpha_lead = leads.pop().expect("alpha lead");
    Teams {
        app,
        alpha_lead,
        beta_lead,
    }
}

#[tokio::test]
async fn test_teams_multi_events_route_to_own_team() {
    let ws = workspace("events_route");
    let Teams {
        mut app,
        alpha_lead,
        beta_lead,
    } = app_with_teams(&ws, 1, 1);
    assert_eq!(app.session.team_members[0].status, MemberStatus::Working);

    app.handle_event(Event::TeammateIdle {
        session_id: beta_lead.clone(),
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
        session_id: alpha_lead.clone(),
        team_name: "alpha".to_string(),
        agent_id: "tm-1".to_string(),
    })
    .await;
    assert_eq!(app.session.team_members[0].status, MemberStatus::Idle);
}

#[tokio::test]
async fn test_teams_multi_focus_cycles_across_teams() {
    let ws = workspace("focus_cycles");
    let mut app = app_with_teams(&ws, 2, 1).app;
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
    let app = app_with_teams(&ws, 2, 1).app;
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
    let Teams {
        mut app, beta_lead, ..
    } = app_with_teams(&ws, 1, 1);
    app.handle_event(Event::TeammateMessage {
        session_id: beta_lead,
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

#[tokio::test]
async fn test_teams_multi_recount_covers_non_active_team_mailboxes() {
    let ws = workspace("recount_other_team");
    let mut app = app_with_teams(&ws, 1, 1).app;
    let beta = TeamStore::load_by_name("beta", &ws).expect("beta store");
    let mailbox = codemason_agent::team::Mailbox::open(&beta.dir, "tm-1").expect("mailbox");
    let msg = codemason_agent::team::MailboxMessage::new(
        "tm-1",
        "lead",
        codemason_agent::team::MessageType::Message,
        "hello",
    );
    mailbox.push(msg).expect("send");
    app.refresh_team_member_session_ids();
    let key = ("beta".to_string(), "tm-1".to_string());
    assert!(
        app.session
            .team_message_counts
            .get(&key)
            .is_some_and(|(sent, _)| *sent >= 1),
        "beta's mailbox is counted while alpha is active: {:?}",
        app.session.team_message_counts
    );
}

#[tokio::test]
async fn test_teams_multi_close_leaves_other_team_listed() {
    let ws = workspace("close_leaves_other");
    let mut app = app_with_teams(&ws, 1, 1).app;
    app.show_teams = true;
    app.execute_slash_command("/team close").await;
    let names: Vec<String> = app.team_sections().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["beta"], "the closed team is gone from the pane");
    assert!(app.show_teams, "the pane stays while beta runs");
}

/// The primary's member mirror for alpha's `tm-1`.
fn alpha_tm1(app: &App) -> &TeamMember {
    app.session
        .team_members
        .iter()
        .find(|m| m.agent_id == "tm-1")
        .expect("alpha tm-1 in the mirror")
}

#[tokio::test]
async fn test_teams_multi_leader_events_reach_the_primary() {
    let ws = workspace("leader_events");
    let Teams {
        mut app,
        alpha_lead,
        ..
    } = app_with_teams(&ws, 1, 0);
    let team = || "alpha".to_string();
    let agent = || "tm-1".to_string();

    app.handle_event(Event::TeamTaskClaimed {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
        task_id: "t-1".to_string(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Working);
    assert_eq!(alpha_tm1(&app).current_task_id.as_deref(), Some("t-1"));

    app.handle_event(Event::TeamTaskCompleted {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
        task_id: "t-1".to_string(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).current_task_id, None);

    app.handle_event(Event::TeammateIdle {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Idle);

    app.handle_event(Event::TeammateSuspended {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Suspended);

    app.handle_event(Event::TeammateResumed {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Idle);

    app.handle_event(Event::TeammateFailed {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
        error: "boom".to_string(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Failed);
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("Teammate tm-1 failed: boom")),
        "the failure is logged"
    );

    app.handle_event(Event::TeammateStopped {
        session_id: alpha_lead.clone(),
        team_name: team(),
        agent_id: agent(),
        cause: "shutdown".to_string(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Stopped);

    app.handle_event(Event::TeammateMessage {
        session_id: alpha_lead.clone(),
        team_name: team(),
        from: agent(),
        to: "lead".to_string(),
        message_type: "message".to_string(),
        preview: "hi".to_string(),
    })
    .await;
    assert_eq!(
        app.session
            .team_message_counts
            .get(&(team(), agent()))
            .copied(),
        Some((1, 0))
    );

    app.handle_event(Event::TeammateSpawned {
        session_id: alpha_lead,
        team_name: team(),
        teammate_name: "late".to_string(),
        agent_id: "tm-9".to_string(),
    })
    .await;
    assert!(
        app.session
            .team_members
            .iter()
            .any(|m| m.agent_id == "tm-9"),
        "a spawn reported by the leader joins the primary's mirror"
    );
}

#[tokio::test]
async fn test_teams_multi_unrelated_session_events_are_ignored() {
    let ws = workspace("unrelated_session");
    let mut app = app_with_teams(&ws, 1, 0).app;
    app.handle_event(Event::TeammateFailed {
        session_id: "someone-else".to_string(),
        team_name: "alpha".to_string(),
        agent_id: "tm-1".to_string(),
        error: "boom".to_string(),
    })
    .await;
    assert_eq!(alpha_tm1(&app).status, MemberStatus::Working);
}

#[tokio::test]
async fn test_teams_multi_cleanup_while_viewing_the_leader() {
    let ws = workspace("cleanup_viewing_leader");
    let Teams {
        mut app,
        alpha_lead,
        ..
    } = app_with_teams(&ws, 1, 1);
    let primary = app.session.session_id.clone().expect("primary");
    app.session.selected_model = Some("provider/model-x".to_string());
    app.session.swarm_state = Some(codemason_agent::team::SwarmState {
        team_name: "alpha".to_string(),
        prompt: "earlier goal".to_string(),
        decomposition: codemason_agent::team::SwarmDecomposition { tasks: vec![] },
        spawned: true,
        completed: false,
        default_agent_type: None,
    });
    app.show_teams = true;

    app.switch_to_session(&alpha_lead).expect("open the leader");
    app.handle_event(Event::TeamCleanedUp {
        session_id: alpha_lead.clone(),
        team_name: "alpha".to_string(),
    })
    .await;
    app.switch_to_session(&primary)
        .expect("back to the primary");

    assert!(app.session.swarm_state.is_none(), "the swarm ended");
    assert!(app.session.active_team.is_none(), "the team ended");
    assert!(app.session.team_members.is_empty());
    let names: Vec<String> = app.team_sections().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["beta"], "the cleaned-up team left the pane");

    app.execute_slash_command("/swarm build a thing").await;
    assert!(
        app.status.contains("decomposing goal"),
        "a new swarm is accepted: {}",
        app.status
    );
}
