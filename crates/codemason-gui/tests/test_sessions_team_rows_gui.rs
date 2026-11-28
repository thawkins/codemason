//! Team rows in the Sessions pane (`codemason_gui::panels::sessions`): a
//! team leader is drawn under its parent session as `team-leader: <team>`
//! with a disclosure triangle (greyed with a hint once it is not running),
//! its teammates under it with their status; the triangle shows and hides
//! the teammates, and a click elsewhere on the row opens the leader.
//!
//! The team is written under the git-ignored `target/temp/` tree only.

#[path = "support/mod.rs"]
mod support;

use std::path::PathBuf;

use codemason_agent::team::{MemberStatus, TeamMember, TeamStore};
use codemason_app_core::log_channel::tui_log_channel;
use codemason_gui::Shell;
use codemason_gui::keys::default_keymap;
use codemason_storage::SessionKind;
use craft_keymap::Platform;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A running window: its runtime, harness and the leader's session id.
struct Rig {
    _runtime: tokio::runtime::Runtime,
    harness: Harness<'static, Shell>,
    leader: String,
}

/// A window whose open session spawned a leader (no task running) of the
/// Active team `alpha` with teammate `rev-1`.
fn rig(test: &str) -> Rig {
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_sessions_team_rows_gui")
        .join(test);
    let _ = std::fs::remove_dir_all(&ws); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(ws.join(".codemason")).expect("workspace");
    let ws = ws.canonicalize().expect("canonical workspace");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut app = {
        let _enter = runtime.enter();
        support::make_app()
    };
    app.cwd_path = ws.clone();
    app.cwd = ws.display().to_string();
    app.sidebar.visible = true;
    let sessions = &app.session_processor.session_manager;
    let primary = sessions.create_session(ws.clone()).expect("primary").id;
    let leader = sessions
        .create_child_session(ws.clone(), &primary, SessionKind::Subagent)
        .expect("leader")
        .id;
    let mate = sessions
        .create_child_session(ws.clone(), &leader, SessionKind::Teammate)
        .expect("teammate")
        .id;
    let mut store = TeamStore::create("alpha", &leader, &ws, true).expect("team");
    let mut member = TeamMember::new("rev-1", "tm-001", "general");
    member.session_id = Some(mate);
    member.status = MemberStatus::Idle;
    store.config.members.push(member);
    store.save().expect("save team");
    app.switch_to_session(&primary).expect("open the primary");
    app.sidebar.dirty = true;
    let (_events_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_logs_tx, logs) = tui_log_channel(64);
    let shell = Shell::new(app)
        .with_keymap(default_keymap(Platform::Linux))
        .with_sources(events, logs, runtime.handle().clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .allow_missing_glyphs()
        .build_ui_state(|ui, shell: &mut Shell| shell.update(ui), shell);
    harness.run();
    Rig {
        _runtime: runtime,
        harness,
        leader,
    }
}

/// Presses and releases the primary button at `pos`.
fn click_at(harness: &mut Harness<'static, Shell>, pos: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(pos));
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
}

#[test]
fn test_sessions_team_rows_gui_leader_and_teammate_drawn() {
    let rig = rig("drawn");
    let leader = rig
        .harness
        .get_by_label_contains("team-leader: alpha (stopped)")
        .rect();
    let mate = rig.harness.get_by_label_contains("rev-1 idle").rect();
    assert!(
        mate.top() > leader.top(),
        "the teammate is under its leader"
    );
    assert!(mate.left() >= leader.left());
}

#[test]
fn test_sessions_team_rows_gui_triangle_toggles_and_row_opens() {
    let mut rig = rig("toggle");
    let row = rig
        .harness
        .get_by_label_contains("team-leader: alpha")
        .rect();
    rig.harness.get_by_label_contains("rev-1 idle");
    // The triangle sits after the row's indent, at its left end.
    let triangle = egui::pos2(row.left() + 24.0, row.center().y);
    click_at(&mut rig.harness, triangle);
    assert!(
        rig.harness.query_by_label_contains("rev-1 idle").is_none(),
        "the triangle hides the teammates"
    );
    assert_ne!(
        rig.harness.state().app().session.session_id.as_deref(),
        Some(rig.leader.as_str()),
        "the triangle does not open the leader"
    );

    click_at(&mut rig.harness, row.right_center() - egui::vec2(40.0, 0.0));
    rig.harness.run();
    assert_eq!(
        rig.harness.state().app().session.session_id.as_deref(),
        Some(rig.leader.as_str())
    );
}

#[test]
fn test_sessions_team_rows_gui_blocked_tooltip_says_blocked() {
    let tip =
        codemason_gui::panels::sessions::teammate_tooltip("swarm-s3", MemberStatus::Blocked, true);
    assert!(
        tip.contains("Blocked: waiting for the subtasks it depends on to finish"),
        "{tip}"
    );
    assert!(tip.contains("cannot be opened"), "{tip}");

    let working =
        codemason_gui::panels::sessions::teammate_tooltip("swarm-s1", MemberStatus::Working, false);
    assert!(working.contains("Status: working"), "{working}");
    assert!(working.contains("Open to watch it"), "{working}");
}
