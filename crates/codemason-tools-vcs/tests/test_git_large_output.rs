//! TL-01: `run_git_output` must drain git's stdout/stderr while it waits.
//!
//! The former implementation piped stdout but only polled `try_wait`, so once
//! git wrote more than the pipe buffer (about 64 KiB) it blocked on the write
//! and was killed at `GIT_TIMEOUT`. This test commits a file far larger than
//! the pipe buffer and reads it back through `git show`, asserting the full
//! content arrives well before the timeout.

use std::path::PathBuf;
use std::time::Instant;

#[path = "support/mod.rs"]
mod support;

use codemason_tools_vcs::git::{GIT_TIMEOUT, run_git_output};
use support::{run_shell, setup_git_repo};

/// A per-test scratch repository under the git-ignored `target/temp/` tree.
fn scratch_repo() -> PathBuf {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/test_git_large_output");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch repo dir");
    dir
}

#[test]
fn test_git_large_output_show_returns_full_content() {
    let dir = scratch_repo();
    setup_git_repo(&dir);
    // ~1 MiB of text: far above the 64 KiB pipe buffer.
    let content: String = (0..100_000).map(|i| format!("line {i:06}\n")).collect();
    std::fs::write(dir.join("big.txt"), &content).expect("write big file");
    run_shell("git add big.txt", &dir);
    run_shell("git commit -q -m 'big'", &dir);

    let started = Instant::now();
    let out = run_git_output(&["show", "HEAD:big.txt"], &dir).expect("git show must not time out");
    assert!(out.success, "git show failed");
    assert_eq!(out.stdout.len(), content.len());
    assert_eq!(out.stdout, content.as_bytes());
    assert!(
        started.elapsed() < GIT_TIMEOUT / 2,
        "git show took {:?}",
        started.elapsed()
    );
}
