//! Direct user turns: a user message to a teammate runs before mailbox work and
//! resumes a parked teammate; a user message to a finished leader runs one more
//! detached turn in the leader's own session and is never injected into the
//! primary.

use std::collections::HashSet;
use std::time::Duration;

use codemason_agent::team::config::MemberStatus;
use codemason_agent::team::{Mailbox, MailboxMessage, MessageType, TeamStore, TeammateNextTurn};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

#[path = "task_support/mod.rs"]
mod task_support;

/// Set `tm-001`'s status directly on disk.
fn set_member(team_dir: &std::path::Path, status: MemberStatus) {
    let mut store = TeamStore::load(team_dir).expect("load");
    let member = store
        .config
        .members
        .iter_mut()
        .find(|m| m.agent_id == "tm-001")
        .expect("member");
    member.status = status;
    member.current_task_id = None;
    store.save().expect("save");
}

/// Give `tm-001` a running agent loop; the loop ends when the sender drops.
fn live_loop(manager: &codemason_agent::team::TeamManager) -> tokio::sync::oneshot::Sender<()> {
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let _ = rx.await;
    });
    manager.set_agent_loop_handle("tm-001", Some(handle));
    tx
}

/// Status of `tm-001` as stored on disk.
fn status_of(team_dir: &std::path::Path) -> MemberStatus {
    TeamStore::load(team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .status
}

#[tokio::test]
async fn test_team_user_turns_run_before_mailbox() {
    let (manager, _tmp, _bus) = team_manager("ut-a", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Idle);
    let _loop = live_loop(&manager);
    Mailbox::open(&manager.team_dir, "tm-001")
        .expect("open")
        .push(
            MailboxMessage::new("lead", "tm-001", MessageType::Message, "review")
                .with_sender_session("lead-sess"),
        )
        .expect("push");
    manager.send_user_turn("tm-001", "hi").expect("queue");

    let mut prompted = HashSet::new();
    match manager.next_teammate_turn("tm-001", &mut prompted) {
        TeammateNextTurn::Run(p) => assert!(p.contains("hi"), "prompt: {p}"),
        other => panic!("expected a user turn, got {other:?}"),
    }
    match manager.next_teammate_turn("tm-001", &mut prompted) {
        TeammateNextTurn::Run(p) => assert!(p.contains("team_read_messages"), "prompt: {p}"),
        other => panic!("expected the mailbox turn next, got {other:?}"),
    }
}

#[tokio::test]
async fn test_team_user_turns_wake_parked_teammate() {
    let (manager, _tmp, _bus) = team_manager("ut-b", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Suspended);
    let _loop = live_loop(&manager);

    manager.send_user_turn("tm-001", "wake up").expect("queue");

    assert_ne!(status_of(&manager.team_dir), MemberStatus::Suspended);
    assert!(matches!(
        manager.next_teammate_turn("tm-001", &mut HashSet::new()),
        TeammateNextTurn::Run(_)
    ));
}

#[tokio::test]
async fn test_team_user_turns_exited_teammate_refused() {
    let (manager, _tmp, _bus) = team_manager("ut-c", "lead-sess", true);
    set_member(&manager.team_dir, MemberStatus::Stopped);

    let err = manager
        .send_user_turn("tm-001", "hello?")
        .expect_err("a stopped teammate has exited");
    assert!(err.to_string().contains("has exited"), "err: {err}");
}

#[tokio::test]
async fn test_team_user_turns_member_without_live_loop_refused() {
    let (manager, _tmp, _bus) = team_manager("ut-d", "lead-sess", true);
    // Non-terminal status, a registered handle, but no loop task to run the turn.
    set_member(&manager.team_dir, MemberStatus::Idle);
    assert!(!manager.has_live_agent_loop("tm-001"));

    let err = manager
        .send_user_turn("tm-001", "anyone?")
        .expect_err("a member whose loop has returned cannot run a turn");
    assert!(err.to_string().contains("has exited"), "err: {err}");
    assert!(
        !matches!(
            manager.next_teammate_turn("tm-001", &mut HashSet::new()),
            TeammateNextTurn::Run(_)
        ),
        "the refused turn must not be queued"
    );
}

#[tokio::test]
async fn test_team_user_turns_resume_finished_leader_not_injected() {
    let processor = task_support::test_processor();
    let sessions = processor.session_manager.clone();
    let mgr = codemason_agent::task::AgentManager::new(
        std::sync::Arc::new(codemason_agent::event::EventBus::new(16)),
        processor,
        2,
        300,
    );
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/temp/user-turns");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let parent = sessions.create_session(dir.clone()).expect("parent");
    let child = sessions
        .create_child_session(dir, &parent.id, codemason_storage::SessionKind::Subagent)
        .expect("child");

    let task_id = mgr
        .resume_detached(&child.id, "general", "carry on")
        .await
        .expect("resume");

    // The run settles (it fails fast under the stub provider); either way the
    // detached contract holds.
    let mut settled = false;
    for _ in 0..100 {
        if mgr
            .get_task(&task_id)
            .await
            .is_some_and(|t| t.status.is_terminal())
        {
            settled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(settled, "the resumed turn must finish");
    let entry = mgr.get_task(&task_id).await.expect("entry");
    assert!(entry.detached, "the resumed turn is detached");
    assert_eq!(entry.child_session_id, child.id, "same session, no new one");
    assert!(mgr.list_agents(&parent.id).await.is_empty());
    assert!(
        mgr.drain_completed(&parent.id).await.is_empty(),
        "a detached result is never injected into the parent"
    );
}

#[tokio::test]
async fn test_team_user_turns_send_to_session_unowned_is_task_not_found() {
    let mgr = task_support::manager(2);
    let err = mgr
        .send_to_session("no-such-session", "hello")
        .expect_err("nothing runs there");
    assert_eq!(
        codemason_agent::tool::param_schema::reason_in(&err.to_string()),
        Some("task_not_found"),
        "err: {err}"
    );
}
