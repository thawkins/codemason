//! Binding a request the agent just created to its session (spec
//! SESSIONMENU T4.3).

use std::path::PathBuf;
use std::sync::Arc;

use codemason_storage::{SessionKind, SessionRequest, Storage};
use codemason_tools_vcs::ToolContext;
use codemason_tools_vcs::binding::{
    bind_created, bind_from_shell, request_from_shell, web_url_host,
};

fn created(number: u64, web_url: &str) -> SessionRequest {
    SessionRequest {
        session_id: String::new(),
        forge: "github".to_string(),
        host: String::new(),
        repo: "acme/app".to_string(),
        number,
        title: "New".to_string(),
        web_url: web_url.to_string(),
        head_branch: "feature".to_string(),
        state: "open".to_string(),
    }
}

fn ctx(session_id: &str, storage: Option<Arc<Storage>>) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("/nonexistent"),
        storage: storage.map(|s| s as Arc<dyn codemason_tools_vcs::storage::StorageBackend>),
        config: None,
    }
}

#[test]
fn test_request_binding_web_url_host() {
    assert_eq!(
        web_url_host("https://GitHub.com/acme/app/pull/1").as_deref(),
        Some("github.com")
    );
    assert_eq!(
        web_url_host("https://gitlab.example.com:8443/g/p/-/merge_requests/2").as_deref(),
        Some("gitlab.example.com:8443")
    );
    assert_eq!(web_url_host("not a url"), None);
}

#[test]
fn test_request_binding_created_request_binds_top_level_session() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage
        .create_session("lead", "/nonexistent")
        .expect("lead");
    storage
        .create_session_with("sub", "/nonexistent", Some("lead"), SessionKind::Subagent)
        .expect("sub");

    bind_created(
        &ctx("sub", Some(Arc::clone(&storage))),
        created(7, "https://github.com/acme/app/pull/7"),
    );

    let bound = storage
        .session_request("lead")
        .expect("read")
        .expect("bound to the top-level session");
    assert_eq!(bound.number, 7);
    assert_eq!(bound.host, "github.com");
    assert_eq!(bound.session_id, "lead");
}

#[test]
fn test_request_binding_skipped_without_storage_or_number() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage.create_session("s", "/nonexistent").expect("s");
    // No storage: nothing to do, and no panic.
    bind_created(
        &ctx("s", None),
        created(1, "https://github.com/acme/app/pull/1"),
    );
    // A create response without a number is not bound.
    bind_created(
        &ctx("s", Some(Arc::clone(&storage))),
        created(0, "https://github.com/acme/app/pull/0"),
    );
    assert!(storage.session_request("s").expect("read").is_none());
}

#[test]
fn test_request_binding_reads_gh_pr_create() {
    let command = "cd /w/feature && gh pr create --base main --head feature \
                   --title \"Removable thumbnails\" --body \"$(cat <<'EOF'\nbody\nEOF\n)\"";
    let output = "Exit code: 0\nremote: https://github.com/acme/app/pull/new/feature\n\
                  https://github.com/acme/app/pull/77\n";
    let request = request_from_shell(command, output).expect("a created PR");
    assert_eq!(request.forge, "github");
    assert_eq!(request.repo, "acme/app");
    assert_eq!(request.number, 77);
    assert_eq!(request.web_url, "https://github.com/acme/app/pull/77");
    assert_eq!(request.title, "Removable thumbnails");
    assert_eq!(request.head_branch, "feature");
    assert_eq!(request.state, "open");
}

#[test]
fn test_request_binding_reads_glab_mr_create() {
    let command = "glab mr create -t 'Fix it' -s fix --yes";
    let output = "!5 Fix it (fix)\n https://gitlab.example.com/g/sub/p/-/merge_requests/5\n";
    let request = request_from_shell(command, output).expect("a created MR");
    assert_eq!(request.forge, "gitlab");
    assert_eq!(request.repo, "g/sub/p");
    assert_eq!(request.number, 5);
    assert_eq!(request.title, "Fix it");
    assert_eq!(request.head_branch, "fix");
}

#[test]
fn test_request_binding_ignores_other_commands() {
    let url = "https://github.com/acme/app/pull/7";
    assert!(request_from_shell("gh pr view 7 --json url", url).is_none());
    assert!(request_from_shell("echo gh pr", url).is_none());
    // A create that printed no request URL (it failed) binds nothing.
    assert!(request_from_shell("gh pr create --fill", "error: no commits").is_none());
}

#[test]
fn test_request_binding_shell_create_binds_top_level_session() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    storage
        .create_session("lead", "/nonexistent")
        .expect("lead");
    storage
        .create_session_with("sub", "/nonexistent", Some("lead"), SessionKind::Subagent)
        .expect("sub");

    bind_from_shell(
        &ctx("sub", Some(Arc::clone(&storage))),
        "gh pr create --fill",
        "https://github.com/acme/app/pull/9\n",
    );

    let bound = storage
        .session_request("lead")
        .expect("read")
        .expect("bound to the top-level session");
    assert_eq!(bound.number, 9);
    assert_eq!(bound.host, "github.com");
    assert_eq!(
        storage
            .session_request_history("lead")
            .expect("history")
            .len(),
        1
    );
}
