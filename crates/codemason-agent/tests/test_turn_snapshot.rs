//! Tests for the per-turn workspace snapshots behind `/undo` and `/redo`.

use std::path::{Path, PathBuf};

use codemason_agent::session::turn_snapshot::SnapshotRepo;

/// A fresh project directory (with a `.git` marker) and snapshot store under
/// the git-ignored `target/temp/` tree.
fn fixture(name: &str) -> (PathBuf, SnapshotRepo) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/turn_snapshot")
        .join(name);
    let _ = std::fs::remove_dir_all(&base);
    let project = base.join("project");
    std::fs::create_dir_all(project.join(".git")).expect("project dir");
    let project = project.canonicalize().expect("canonical project dir");
    let repo = SnapshotRepo::in_store(&base.join("store"), &project).expect("snapshot repo");
    (project, repo)
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

#[test]
fn test_snapshot_repo_requires_a_git_work_tree() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/turn_snapshot");
    let plain = base.join("no_git_here");
    std::fs::create_dir_all(&plain).expect("mkdir");
    // `target/temp` sits inside this repository, so resolve from the filesystem
    // root instead to get a directory with no `.git` above it.
    assert!(SnapshotRepo::in_store(&base.join("store"), Path::new("/")).is_none());
    let nested = SnapshotRepo::in_store(&base.join("store"), &plain).expect("repo above");
    assert!(plain.starts_with(nested.root()));
}

#[tokio::test]
async fn test_restore_reverts_edits_creations_and_deletions() {
    let (project, repo) = fixture("restore");
    write(&project, "kept.txt", "original");
    write(&project, "sub/removed.txt", "will be deleted by the turn");
    repo.record_turn("s1", "m1").await.expect("record");

    // The turn edits one file, creates one and deletes one.
    write(&project, "kept.txt", "edited");
    write(&project, "created.txt", "new");
    std::fs::remove_file(project.join("sub/removed.txt")).expect("rm");

    let tree = repo.turn_tree("s1", "m1").await.expect("turn tree");
    let (after_turn, mut changed) = repo.restore(&tree).await.expect("undo");
    changed.sort();

    assert_eq!(changed, ["created.txt", "kept.txt", "sub/removed.txt"]);
    assert_eq!(read(&project, "kept.txt").as_deref(), Some("original"));
    assert_eq!(read(&project, "created.txt"), None);
    assert_eq!(
        read(&project, "sub/removed.txt").as_deref(),
        Some("will be deleted by the turn")
    );

    // Redo: restoring the replaced state brings the turn's changes back.
    repo.restore(&after_turn).await.expect("redo");
    assert_eq!(read(&project, "kept.txt").as_deref(), Some("edited"));
    assert_eq!(read(&project, "created.txt").as_deref(), Some("new"));
    assert_eq!(read(&project, "sub/removed.txt"), None);
}

#[tokio::test]
async fn test_record_turn_keeps_the_first_snapshot() {
    let (project, repo) = fixture("first_wins");
    write(&project, "a.txt", "before");
    repo.record_turn("s1", "m1").await.expect("record");
    let first = repo.turn_tree("s1", "m1").await.expect("tree");

    write(&project, "a.txt", "after first tool call");
    repo.record_turn("s1", "m1").await.expect("record again");

    assert_eq!(repo.turn_tree("s1", "m1").await, Some(first));
    assert_eq!(repo.turn_tree("s1", "other").await, None);
    assert_eq!(repo.turn_tree("s2", "m1").await, None);
}

#[tokio::test]
async fn test_ignored_files_are_neither_recorded_nor_restored() {
    let (project, repo) = fixture("ignored");
    write(&project, ".gitignore", "build/\n");
    write(&project, "build/out.bin", "v1");
    repo.record_turn("s1", "m1").await.expect("record");

    write(&project, "build/out.bin", "v2");
    let tree = repo.turn_tree("s1", "m1").await.expect("tree");
    let (_, changed) = repo.restore(&tree).await.expect("restore");

    assert!(changed.is_empty(), "{changed:?}");
    assert_eq!(read(&project, "build/out.bin").as_deref(), Some("v2"));
}

#[tokio::test]
async fn test_set_and_forget_turn_tree() {
    let (project, repo) = fixture("set_forget");
    write(&project, "a.txt", "one");
    let one = repo.track().await.expect("track");
    write(&project, "a.txt", "two");
    repo.record_turn("s1", "m1").await.expect("record");

    repo.set_turn_tree("s1", "m1", &one).await.expect("set");
    assert_eq!(repo.turn_tree("s1", "m1").await, Some(one));

    repo.forget_turn("s1", "m1").await.expect("forget");
    assert_eq!(repo.turn_tree("s1", "m1").await, None);
}

#[tokio::test]
async fn test_turn_tree_does_not_create_the_store() {
    let (_project, repo) = fixture("lazy");
    assert_eq!(repo.turn_tree("s1", "m1").await, None);
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/turn_snapshot/lazy/store");
    assert!(
        !store.exists(),
        "a lookup must not create the shadow repository"
    );
}
