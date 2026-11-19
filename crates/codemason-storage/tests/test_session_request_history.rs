//! Per-session pull/merge request history (spec SESSIONMENU): every
//! request bound to a session is kept, counted and listed newest first.

use codemason_storage::{DiffStats, SessionRequest, Storage};

fn request(session: &str, number: u64, state: &str) -> SessionRequest {
    SessionRequest {
        session_id: session.to_string(),
        forge: "gitlab".to_string(),
        host: "gitlab.example.com".to_string(),
        repo: "group/app".to_string(),
        number,
        title: format!("MR {number}"),
        web_url: format!("https://gitlab.example.com/group/app/-/merge_requests/{number}"),
        head_branch: format!("b{number}"),
        state: state.to_string(),
    }
}

#[test]
fn test_session_request_history_keeps_every_binding() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("s", "/nonexistent/history")
        .expect("s");
    storage
        .create_session("t", "/nonexistent/history")
        .expect("t");

    storage
        .set_session_request(&request("s", 1, "open"))
        .expect("1");
    std::thread::sleep(std::time::Duration::from_millis(5));
    storage
        .set_session_request(&request("s", 2, "open"))
        .expect("2");
    // Rebinding the same request updates it, it is not counted twice.
    storage
        .set_session_request(&request("s", 2, "merged"))
        .expect("2 again");
    storage
        .set_session_request(&request("t", 9, "open"))
        .expect("9");
    storage.clear_session_request("s").expect("unbind");

    let history = storage.session_request_history("s").expect("history");
    let seen: Vec<(u64, &str)> = history
        .iter()
        .map(|r| (r.number, r.state.as_str()))
        .collect();
    assert_eq!(seen, [(2, "merged"), (1, "open")]);
    // Every session's history at once, in the same order.
    let all = storage.session_request_histories().expect("histories");
    assert_eq!(all.len(), 2);
    assert_eq!(all.get("s"), Some(&history));
    let t: Vec<u64> = all["t"].iter().map(|r| r.number).collect();
    assert_eq!(t, [9]);
}

#[test]
fn test_session_request_history_goes_with_a_deleted_session() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("s", "/nonexistent/history")
        .expect("s");
    storage
        .set_session_request(&request("s", 1, "open"))
        .expect("1");

    storage.delete_session("s").expect("delete");

    assert_eq!(
        storage.session_request_history("s").expect("history"),
        Vec::new()
    );
    assert!(storage.session_request_histories().expect("all").is_empty());
}

#[test]
fn test_session_request_history_ignores_a_dropped_poll() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("s", "/nonexistent/history")
        .expect("s");
    let bound = request("s", 1, "open");
    storage.set_session_request(&bound).expect("bind");

    // Auto-fix is off, so the poll is dropped and nothing is written.
    let kept = storage
        .record_autofix_poll(&bound, "merged", "changed", "abc")
        .expect("poll");

    assert!(!kept);
    let history = storage.session_request_history("s").expect("history");
    assert_eq!(history[0].state, "open");
    assert_eq!(history[0].title, "MR 1");
}

#[test]
fn test_session_request_history_open_rows_and_state_update() {
    let storage = Storage::open_in_memory().expect("storage");
    for id in ["s", "t", "old"] {
        storage
            .create_session(id, "/nonexistent/history")
            .expect("session");
    }
    // One request seen by two sessions, one already merged, one in an
    // archived session.
    storage
        .set_session_request(&request("s", 1, "open"))
        .expect("s1");
    storage
        .set_session_request(&request("t", 1, "open"))
        .expect("t1");
    storage
        .set_session_request(&request("s", 2, "merged"))
        .expect("s2");
    storage
        .set_session_request(&request("old", 3, "open"))
        .expect("old3");
    storage.archive_session("old").expect("archive");

    let open: Vec<u64> = storage
        .open_request_history()
        .expect("open")
        .iter()
        .map(|r| r.number)
        .collect();
    assert_eq!(open, [1], "one row per request, live sessions only");

    let changed = storage
        .set_request_history_state("gitlab.example.com", "group/app", 1, "merged", "Done")
        .expect("update");
    assert_eq!(changed, 2, "every session's row");
    assert_eq!(
        storage
            .set_request_history_state("gitlab.example.com", "group/app", 1, "merged", "Done")
            .expect("again"),
        0,
        "unchanged rows are not rewritten"
    );
    assert_eq!(storage.open_request_history().expect("open"), Vec::new());
    assert_eq!(
        storage.session_request_history("t").expect("t")[0].title,
        "Done"
    );
}

/// Request `number` on GitHub, in session `session`.
fn github(session: &str, number: u64, state: &str) -> SessionRequest {
    SessionRequest {
        forge: "github".to_string(),
        host: "github.com".to_string(),
        repo: "acme/app".to_string(),
        ..request(session, number, state)
    }
}

#[test]
fn test_session_request_history_diff_stats() {
    let storage = Storage::open_in_memory().expect("storage");
    for id in ["s", "t", "old"] {
        storage
            .create_session(id, "/nonexistent/history")
            .expect("session");
    }
    storage
        .set_session_request(&github("s", 1, "merged"))
        .expect("s1");
    storage
        .set_session_request(&github("t", 1, "merged"))
        .expect("t1");
    storage
        .set_session_request(&github("s", 2, "open"))
        .expect("s2");
    storage
        .set_session_request(&request("s", 3, "merged"))
        .expect("gitlab 3");
    storage
        .set_session_request(&github("old", 4, "merged"))
        .expect("old4");
    storage.archive_session("old").expect("archive");

    // GitHub requests of live sessions without counts, one row each; GitLab
    // has no counts to read, and archived sessions are not read.
    let missing: Vec<u64> = storage
        .request_history_missing_stats()
        .expect("missing")
        .iter()
        .map(|r| r.number)
        .collect();
    assert_eq!(missing, [1, 2]);
    assert!(storage.request_diff_stats().expect("stats").is_empty());

    let changed = storage
        .set_request_diff_stats("github.com", "acme/app", 1, 120, 7)
        .expect("set");
    assert_eq!(changed, 2, "every session's row");
    assert_eq!(
        storage
            .set_request_diff_stats("github.com", "acme/app", 1, 120, 7)
            .expect("again"),
        0,
        "unchanged rows are not rewritten"
    );
    let stats = storage.request_diff_stats().expect("stats");
    let key = ("github.com".to_string(), "acme/app".to_string(), 1);
    assert_eq!(
        stats.get(&key),
        Some(&DiffStats {
            additions: 120,
            deletions: 7
        })
    );
    assert_eq!(stats.len(), 1);
    let missing: Vec<u64> = storage
        .request_history_missing_stats()
        .expect("missing")
        .iter()
        .map(|r| r.number)
        .collect();
    assert_eq!(missing, [2]);
}

/// A schema 13 database (no diff-size columns) gains them on open.
#[test]
fn test_session_request_history_migrates_a_v13_database() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/session_request_history/migrate_v13");
    let _ = std::fs::remove_dir_all(&dir); // INTENTIONAL: may not exist yet
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    let path = dir.join("codemason.db");
    {
        let storage = Storage::open(&path).expect("open");
        storage
            .create_session("s", "/nonexistent/history")
            .expect("s");
        storage
            .set_session_request(&github("s", 1, "merged"))
            .expect("1");
        let guard = storage.conn_lock_for_test().expect("lock");
        guard
            .execute_batch(
                "ALTER TABLE session_request_history DROP COLUMN additions;
                 ALTER TABLE session_request_history DROP COLUMN deletions;
                 UPDATE settings SET value = '13' WHERE key = 'schema_version';",
            )
            .expect("rewind to a v13 schema");
    }

    let storage = Storage::open(&path).expect("migrate");

    let missing = storage
        .request_history_missing_stats()
        .expect("columns exist");
    assert_eq!(missing.len(), 1);
    storage
        .set_request_diff_stats("github.com", "acme/app", 1, 5, 2)
        .expect("stats");
    assert_eq!(storage.request_diff_stats().expect("stats").len(), 1);
}
