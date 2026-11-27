//! TL-04 regression tests: `git_status` structured path lists.
//!
//! The default mode runs `git status --porcelain=v2`, whose `1`/`2`/`u`
//! records carry several space-separated fields before the path; the parser
//! used to report the whole record as the path, never parsed `u` (unmerged)
//! records, and parsed `short: true` (porcelain v1) output as v2. These tests
//! build a real repository with a modified file in a subdirectory, an
//! untracked file, a staged rename and a merge conflict, and assert the exact
//! relative paths in both modes.

// Holds the crate-wide env lock across the body on purpose: it
// serialises against the PATH-swapping git-shim test. See AGENTS.md.
#![allow(clippy::await_holding_lock)]

use std::fs;
use std::path::Path;
use std::process::Command;

#[path = "support/mod.rs"]
mod support;

use codemason_tools_vcs::git::GitStatusTool;
use codemason_tools_vcs::{Tool, ToolContext};
use serde_json::{Value, json};
use support::{run_shell, setup_git_repo};

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        storage: None,
        config: None,
    }
}

/// Build the repository state: modified `src/lib.rs`, untracked `new.txt`,
/// staged rename `old.txt -> renamed.txt`, and conflicted `conflict.txt`.
fn build_repo(dir: &Path) {
    setup_git_repo(dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "fn a() {}\n").unwrap();
    fs::write(dir.join("old.txt"), "rename me, a reasonably long line\n").unwrap();
    fs::write(dir.join("conflict.txt"), "base\n").unwrap();
    run_shell("git add -A", dir);
    run_shell("git commit -q -m base", dir);

    run_shell("git checkout -q -b other", dir);
    fs::write(dir.join("conflict.txt"), "other\n").unwrap();
    run_shell("git commit -q -am other", dir);
    run_shell("git checkout -q main", dir);
    fs::write(dir.join("conflict.txt"), "main\n").unwrap();
    run_shell("git commit -q -am main", dir);

    // The merge is expected to fail with a conflict, so do not use run_shell.
    let merge = Command::new("git")
        .args(["merge", "-q", "other"])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(!merge.status.success(), "merge should conflict");

    fs::write(dir.join("src/lib.rs"), "fn a() {}\nfn b() {}\n").unwrap();
    fs::write(dir.join("new.txt"), "untracked\n").unwrap();
    run_shell("git mv old.txt renamed.txt", dir);
}

fn list(meta: &Value, key: &str) -> Vec<String> {
    serde_json::from_value(meta[key].clone()).unwrap()
}

/// Assert the exact path lists shared by both output modes.
fn assert_paths(meta: &Value, mode: &str) {
    assert_eq!(
        list(meta, "modified"),
        vec!["src/lib.rs"],
        "{mode}: modified"
    );
    assert_eq!(
        list(meta, "untracked"),
        vec!["new.txt"],
        "{mode}: untracked"
    );
    assert_eq!(list(meta, "staged"), vec!["renamed.txt"], "{mode}: staged");
    assert_eq!(
        list(meta, "conflicted"),
        vec!["conflict.txt"],
        "{mode}: conflicted"
    );
}

#[tokio::test]
async fn test_git_status_porcelain_v2_exact_paths() {
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    build_repo(tmp.path());

    let out = GitStatusTool
        .execute(json!({}), &make_ctx(tmp.path()))
        .await
        .unwrap();
    let meta = out.metadata.expect("metadata");
    assert_paths(&meta, "v2");
    assert_eq!(meta["branch"], json!("main"));
}

#[tokio::test]
async fn test_git_status_short_exact_paths() {
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    build_repo(tmp.path());

    let out = GitStatusTool
        .execute(json!({"short": true}), &make_ctx(tmp.path()))
        .await
        .unwrap();
    // Short mode keeps its documented compact v1 output as the content.
    assert!(
        out.content.contains("?? new.txt"),
        "short content: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_paths(&meta, "short");
}
