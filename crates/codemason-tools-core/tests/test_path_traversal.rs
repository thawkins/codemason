//! Regression tests for C-002: path-traversal protection in file tools.
//!
//! Verifies that the hardened `check_path_within_root` rejects `..`, absolute
//! escapes, symlink escapes, and root-prefix confusion, and that every file
//! tool entry point (`read`, `Write`, `create`, `append_file`, `rm`,
//! `mkdir`, `copy_file`, `move_file`, `file_info`, `list`, `Glob`, `Grep`,
//! `diff`) enforces containment before touching the filesystem.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use codemason_tools_core::append_file::AppendFileTool;
use codemason_tools_core::create::CreateTool;
use codemason_tools_core::diff::DiffFilesTool;
use codemason_tools_core::file_info::FileInfoTool;
use codemason_tools_core::glob::GlobTool;
use codemason_tools_core::grep::GrepTool;
use codemason_tools_core::list::ListTool;
use codemason_tools_core::mkdir::MakeDirTool;
use codemason_tools_core::move_file::MoveFileTool;
use codemason_tools_core::read::ReadTool;
use codemason_tools_core::rm::RmTool;
use codemason_tools_core::write::WriteTool;
use codemason_tools_core::{Tool, ToolContext, check_path_within_root};
use codemason_types::event::EventBus;
use serde_json::json;

fn ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(codemason_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir.to_path_buf()],
    }
}

fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "codemason_path_traversal_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("creating temp dir");
    dir
}

// ============================================================================
// Unit tests for `check_path_within_root`
// ============================================================================

#[test]
fn check_path_allows_same_path() {
    let dir = tmp_dir();
    let file = dir.join("safe.txt");
    std::fs::write(&file, "hello").unwrap();
    assert!(check_path_within_root(&file, &dir).is_ok());
}

#[test]
fn check_path_rejects_dotdot_escape() {
    let dir = tmp_dir();
    let escaped = dir.join("..").join("etc").join("passwd");
    assert!(check_path_within_root(&escaped, &dir).is_err());
}

#[test]
fn check_path_rejects_absolute_outside_root() {
    let dir = tmp_dir();
    assert!(check_path_within_root(Path::new("/etc/passwd"), &dir).is_err());
}

#[test]
fn check_path_does_not_confuse_prefix() {
    // /foo should not contain /foobar just because the string prefix matches.
    let root = tmp_dir().join("foo");
    std::fs::create_dir_all(&root).unwrap();
    let sibling = root.parent().unwrap().join("foobar").join("secret.txt");
    assert!(check_path_within_root(&sibling, &root).is_err());
}

#[test]
#[cfg(unix)]
fn check_path_rejects_symlink_escape() {
    use std::os::unix::fs::symlink as make_symlink;
    let dir = tmp_dir();
    let outside = std::env::temp_dir().join(format!(
        "codemason_path_traversal_secret_{}",
        std::process::id()
    ));
    std::fs::write(&outside, "secret").unwrap();
    let link = dir.join("escape");
    make_symlink(&outside, &link).unwrap();
    assert!(check_path_within_root(&link, &dir).is_err());
    let _ = std::fs::remove_file(&outside);
}

#[test]
fn check_path_rejects_nonexistent_traversal() {
    let dir = tmp_dir();
    let escaped = dir.join("..").join("does-not-exist").join("file.txt");
    assert!(check_path_within_root(&escaped, &dir).is_err());
}

// ============================================================================
// Tool-level tests
// ============================================================================

#[tokio::test]
async fn read_rejects_traversal() {
    let dir = tmp_dir();
    let out = ReadTool
        .execute(json!({"path": "../ Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "read should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn write_rejects_traversal() {
    let dir = tmp_dir();
    let out = WriteTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "write should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn create_rejects_traversal() {
    let dir = tmp_dir();
    let out = CreateTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "create should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn append_rejects_traversal() {
    let dir = tmp_dir();
    let out = AppendFileTool
        .execute(json!({"path": "../escape.txt", "content": "x"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "append should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn rm_rejects_traversal() {
    let dir = tmp_dir();
    let out = RmTool
        .execute(json!({"path": "../Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "rm should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn mkdir_rejects_traversal() {
    let dir = tmp_dir();
    let out = MakeDirTool
        .execute(json!({"path": "../escape_dir"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "mkdir should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn file_info_rejects_traversal() {
    let dir = tmp_dir();
    let out = FileInfoTool
        .execute(json!({"path": "../Cargo.toml"}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "file_info should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn list_rejects_traversal() {
    let dir = tmp_dir();
    let out = ListTool.execute(json!({"path": ".."}), &ctx(&dir)).await;
    assert!(out.is_err(), "list should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn list_allows_additional_allowed_root() {
    let dir_a = tmp_dir();
    let dir_b = tmp_dir();
    let mut ctx = ctx(&dir_a);
    ctx.allowed_roots.push(dir_b.clone());

    let out = ListTool
        .execute(json!({"path": dir_b.to_str().unwrap(), "depth": 0}), &ctx)
        .await;
    assert!(
        out.is_ok(),
        "list should allow path inside configured allowed_root: {out:?}"
    );
}

#[tokio::test]
async fn glob_rejects_traversal() {
    let dir = tmp_dir();
    let out = GlobTool
        .execute(json!({"pattern": "**/*.rs", "path": ".."}), &ctx(&dir))
        .await;
    assert!(out.is_err(), "glob should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn grep_rejects_traversal() {
    let dir = tmp_dir();
    let out = GrepTool
        .execute(
            json!({"pattern": "fn", "path": "..", "max_results": 1}),
            &ctx(&dir),
        )
        .await;
    assert!(out.is_err(), "grep should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn diff_rejects_traversal() {
    let dir = tmp_dir();
    let out = DiffFilesTool
        .execute(
            json!({"path_a": "../Cargo.toml", "path_b": "../Cargo.toml"}),
            &ctx(&dir),
        )
        .await;
    assert!(out.is_err(), "diff should reject ../ escape: {out:?}");
}

#[tokio::test]
async fn copy_file_rejects_traversal_source() {
    let dir = tmp_dir();
    let out = codemason_tools_core::copy_file::CopyFileTool
        .execute(
            json!({"source": "../Cargo.toml", "destination": "copy.toml"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "copy_file source should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn copy_file_rejects_traversal_destination() {
    let dir = tmp_dir();
    let out = codemason_tools_core::copy_file::CopyFileTool
        .execute(
            json!({"source": "in.txt", "destination": "../escape.txt"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "copy_file destination should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn move_file_rejects_traversal_destination() {
    let dir = tmp_dir();
    let out = MoveFileTool
        .execute(
            json!({"source": "in.txt", "destination": "../escape.txt"}),
            &ctx(&dir),
        )
        .await;
    assert!(
        out.is_err(),
        "move_file destination should reject ../ escape: {out:?}"
    );
}

#[tokio::test]
async fn allows_normal_relative_path() {
    let dir = tmp_dir();
    std::fs::write(dir.join("safe.txt"), "hello").unwrap();
    let out = ReadTool
        .execute(json!({"path": "safe.txt"}), &ctx(&dir))
        .await;
    assert!(
        out.is_ok(),
        "normal relative path should be allowed: {out:?}"
    );
}

// ============================================================================
// Alias / bind-mount acceptance tests
// ============================================================================

#[test]
#[cfg(unix)]
fn check_path_accepts_symlink_alias_to_root() {
    let dir = tmp_dir();
    let alias = dir
        .parent()
        .unwrap()
        .join(format!("codemason_path_alias_{}", std::process::id()));
    std::os::unix::fs::symlink(&dir, &alias).unwrap();

    let file = alias.join("safe.txt");
    std::fs::write(&file, "hello").unwrap();
    assert!(
        check_path_within_root(&file, &dir).is_ok(),
        "symlink alias to root should be accepted"
    );

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&alias);
}

#[test]
#[cfg(unix)]
fn check_path_accepts_nonexistent_under_alias_root() {
    let dir = tmp_dir();
    let alias = dir
        .parent()
        .unwrap()
        .join(format!("codemason_path_alias_new_{}", std::process::id()));
    std::os::unix::fs::symlink(&dir, &alias).unwrap();

    let new_file = alias.join("does-not-exist-yet.txt");
    assert!(
        check_path_within_root(&new_file, &dir).is_ok(),
        "non-existent path under alias root should be accepted"
    );

    let _ = std::fs::remove_file(&alias);
}

#[test]
#[cfg(unix)]
fn check_path_rejects_alias_to_sibling_directory() {
    let dir = tmp_dir();
    let sibling = tmp_dir();
    let alias = dir.parent().unwrap().join(format!(
        "codemason_path_sibling_alias_{}",
        std::process::id()
    ));
    std::os::unix::fs::symlink(&sibling, &alias).unwrap();

    let file = alias.join("secret.txt");
    std::fs::write(&file, "secret").unwrap();
    assert!(
        check_path_within_root(&file, &dir).is_err(),
        "alias to a sibling directory must still be rejected"
    );

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(&alias);
}

// ============================================================================
// `..` inside a not-yet-existing parent (regression: the missing-tail walk used
// to drop every `..`, so `ghost/../../x` was checked as `ghost/x`)
// ============================================================================

#[test]
fn check_rejects_parent_dir_through_missing_directory() {
    let dir = tmp_dir();
    let escape = dir.join("ghost").join("..").join("..").join("escape.txt");
    assert!(
        check_path_within_root(&escape, &dir).is_err(),
        "`..` through a missing directory must not be dropped"
    );
}

#[test]
fn check_allows_parent_dir_that_stays_inside_root() {
    let dir = tmp_dir();
    let inside = dir.join("ghost").join("..").join("inside.txt");
    assert!(check_path_within_root(&inside, &dir).is_ok());
}

#[tokio::test]
async fn write_rejects_traversal_through_missing_directory() {
    let dir = tmp_dir();
    let unique = format!("escape-{}.txt", std::process::id());
    let path = format!("ghost/../../{unique}");
    let out = WriteTool
        .execute(json!({"path": path, "content": "x"}), &ctx(&dir))
        .await;
    assert!(
        out.is_err(),
        "write must reject ghost/../../ escape: {out:?}"
    );
    let outside = dir.parent().map(|p| p.join(&unique));
    assert!(
        outside.is_some_and(|p| !p.exists()),
        "nothing may be written outside the root"
    );
}

// ============================================================================
// Dangling symlinks (regression: a dangling link was treated as a missing
// tail and checked lexically, so `Write` followed it outside the root)
// ============================================================================

/// Unix-only: these tests create symlinks with `std::os::unix::fs::symlink`.
#[cfg(unix)]
mod dangling_symlinks {
    use super::*;

    /// Creates `root/<name>` as a symlink to a not-yet-existing file in a fresh
    /// sibling directory outside `root`, and returns `(link, outside_target)`.
    fn dangling_link_outside(root: &Path, name: &str) -> (PathBuf, PathBuf) {
        let outside = tmp_dir().join("victim.txt");
        let link = root.join(name);
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        (link, outside)
    }

    #[test]
    fn check_rejects_dangling_symlink_to_outside_root() {
        let dir = tmp_dir();
        let (link, _) = dangling_link_outside(&dir, "evil");
        assert!(
            check_path_within_root(&link, &dir).is_err(),
            "a dangling symlink pointing outside the root must be rejected"
        );
    }

    #[test]
    fn check_cached_rejects_dangling_symlink_to_outside_root() {
        let dir = tmp_dir();
        let (link, _) = dangling_link_outside(&dir, "evil");
        let cache = codemason_tools_core::CanonicalPathCache::new();
        assert!(
            codemason_tools_core::check_path_within_root_cached(&link, &dir, &cache).is_err(),
            "the cached check must reject a dangling symlink pointing outside the root"
        );
    }

    #[test]
    fn check_rejects_missing_child_of_dangling_symlink_dir() {
        let dir = tmp_dir();
        let outside_dir = tmp_dir().join("not-yet");
        let link = dir.join("linkdir");
        std::os::unix::fs::symlink(&outside_dir, &link).unwrap();
        assert!(check_path_within_root(&link.join("sub").join("f.txt"), &dir).is_err());
    }

    #[test]
    fn check_allows_dangling_symlink_that_stays_inside_root() {
        let dir = tmp_dir();
        let link = dir.join("later");
        std::os::unix::fs::symlink(dir.join("created-later.txt"), &link).unwrap();
        assert!(check_path_within_root(&link, &dir).is_ok());
        let relative = dir.join("rel");
        std::os::unix::fs::symlink("created-later.txt", &relative).unwrap();
        assert!(check_path_within_root(&relative, &dir).is_ok());
    }

    #[test]
    fn check_rejects_symlink_loop() {
        let dir = tmp_dir();
        std::os::unix::fs::symlink(dir.join("b"), dir.join("a")).unwrap();
        std::os::unix::fs::symlink(dir.join("a"), dir.join("b")).unwrap();
        assert!(check_path_within_root(&dir.join("a"), &dir).is_err());
    }

    #[tokio::test]
    async fn write_refuses_to_follow_dangling_symlink_outside_root() {
        let dir = tmp_dir();
        let (_, outside) = dangling_link_outside(&dir, "evil");
        let out = WriteTool
            .execute(json!({"path": "evil", "content": "pwned"}), &ctx(&dir))
            .await;
        assert!(
            out.is_err(),
            "write through a dangling link must fail: {out:?}"
        );
        assert!(!outside.exists(), "nothing may be created outside the root");
    }

    #[tokio::test]
    async fn create_refuses_to_follow_dangling_symlink_outside_root() {
        let dir = tmp_dir();
        let (_, outside) = dangling_link_outside(&dir, "evil");
        let out = CreateTool
            .execute(json!({"path": "evil", "content": "pwned"}), &ctx(&dir))
            .await;
        assert!(
            out.is_err(),
            "create through a dangling link must fail: {out:?}"
        );
        assert!(!outside.exists(), "nothing may be created outside the root");
    }

    #[tokio::test]
    async fn append_refuses_to_follow_dangling_symlink_outside_root() {
        let dir = tmp_dir();
        let (_, outside) = dangling_link_outside(&dir, "evil");
        let out = AppendFileTool
            .execute(json!({"path": "evil", "content": "pwned"}), &ctx(&dir))
            .await;
        assert!(
            out.is_err(),
            "append through a dangling link must fail: {out:?}"
        );
        assert!(!outside.exists(), "nothing may be created outside the root");
    }

    /// Regression: after `..` pops into the existing prefix, the components
    /// replayed after it were pushed lexically and never re-checked, so
    /// `ghost/../evil` passed while `ghost` was missing and `evil` was a dangling
    /// link out of the root; `Write` then created `ghost` and followed `evil`.
    #[test]
    fn check_rejects_dangling_symlink_reached_through_parent_dir() {
        let dir = tmp_dir();
        let (_, _) = dangling_link_outside(&dir, "evil");
        let path = dir.join("ghost").join("..").join("evil");
        assert!(check_path_within_root(&path, &dir).is_err());
        let cache = codemason_tools_core::CanonicalPathCache::new();
        assert!(
            codemason_tools_core::check_path_within_root_cached(&path, &dir, &cache).is_err(),
            "the cached check must reject it too"
        );
    }

    #[tokio::test]
    async fn write_refuses_dangling_symlink_reached_through_parent_dir() {
        let dir = tmp_dir();
        let (_, outside) = dangling_link_outside(&dir, "evil");
        let out = WriteTool
            .execute(
                json!({"path": "ghost/../evil", "content": "pwned"}),
                &ctx(&dir),
            )
            .await;
        assert!(
            out.is_err(),
            "write through ghost/../evil must fail: {out:?}"
        );
        assert!(!outside.exists(), "nothing may be created outside the root");
    }

    #[test]
    fn check_allows_parent_dir_back_to_an_existing_file() {
        let dir = tmp_dir();
        std::fs::write(dir.join("real.txt"), "x").unwrap();
        assert!(
            check_path_within_root(&dir.join("ghost").join("..").join("real.txt"), &dir).is_ok()
        );
    }
}
