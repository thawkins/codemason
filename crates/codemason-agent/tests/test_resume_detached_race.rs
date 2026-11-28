//! `AgentManager::resume_detached` reserves the session atomically: two
//! near-simultaneous resumes of one finished session run exactly one task, and a
//! resume that fails releases its reservation.

use std::sync::Arc;

use codemason_agent::task::AgentManager;

#[path = "task_support/mod.rs"]
mod task_support;

/// Concurrent resumers per round.
const RESUMERS: usize = 4;

/// A manager over the stub processor plus a scratch directory under `target/temp`.
fn manager() -> (
    Arc<AgentManager>,
    Arc<codemason_agent::session::SessionManager>,
    std::path::PathBuf,
) {
    let processor = task_support::test_processor();
    let sessions = processor.session_manager.clone();
    let mgr = Arc::new(AgentManager::new(
        Arc::new(codemason_agent::event::EventBus::new(16)),
        processor,
        4,
        300,
    ));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/resume-detached-race")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("scratch dir");
    (mgr, sessions, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_resume_detached_race_never_two_tasks_at_once() {
    let (mgr, sessions, dir) = manager();
    // Repeat so a check-then-act gap is hit reliably, not by luck of scheduling.
    for round in 0..50 {
        let parent = sessions.create_session(dir.clone()).expect("parent");
        let child = sessions
            .create_child_session(
                dir.clone(),
                &parent.id,
                codemason_storage::SessionKind::Subagent,
            )
            .expect("child");
        let barrier = Arc::new(tokio::sync::Barrier::new(RESUMERS));
        let handles: Vec<_> = (0..RESUMERS)
            .map(|_| {
                let (mgr, id, barrier) = (mgr.clone(), child.id.clone(), barrier.clone());
                tokio::spawn(async move {
                    barrier.wait().await;
                    mgr.resume_detached(&id, "general", "go").await
                })
            })
            .collect();
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.expect("join"));
        }
        // The stub run can finish before a late resumer is scheduled, and a
        // resume after completion is legitimate. What must never happen is two
        // runs alive in the session at once: every Ok task has to have completed
        // before the next one was created.
        let mut runs = Vec::new();
        for id in results.iter().filter_map(|r| r.as_ref().ok()) {
            runs.push(mgr.get_task(id).await.expect("resumed task is registered"));
        }
        assert!(!runs.is_empty(), "round {round}: one resume must win");
        runs.sort_by_key(|run| run.created_at);
        for pair in runs.windows(2) {
            assert!(
                pair[0]
                    .completed_at
                    .is_some_and(|done| done <= pair[1].created_at),
                "round {round}: two resumed tasks ran in one session at once"
            );
        }
        for err in results.iter().filter_map(|r| r.as_ref().err()) {
            assert!(
                err.to_string().contains("already running"),
                "round {round}: err: {err}"
            );
        }
    }
}

#[tokio::test]
async fn test_resume_detached_race_failed_resume_releases_reservation() {
    let (mgr, sessions, dir) = manager();
    // A root session has no parent, so the resume fails after the reservation.
    let orphan = sessions.create_session(dir.clone()).expect("orphan");

    let err = mgr
        .resume_detached(&orphan.id, "general", "go")
        .await
        .expect_err("no parent");
    assert!(err.to_string().contains("no parent"), "err: {err}");
    assert!(
        mgr.suspend_flag_for_session(&orphan.id).is_none(),
        "a failed resume must release the session reservation"
    );

    // An unknown session fails the same way and leaves nothing reserved.
    mgr.resume_detached("missing-session", "general", "go")
        .await
        .expect_err("unknown session");
    assert!(mgr.suspend_flag_for_session("missing-session").is_none());
}
