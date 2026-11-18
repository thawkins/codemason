//! Per-turn workspace snapshots for `/undo` and `/redo`.
//!
//! Before a turn's first tool call, the processor records the state of the
//! session's git work tree as a tree object in a private "shadow" git
//! repository, one per project, kept outside the project under the snapshot
//! store. `/undo` restores the work tree to that tree; `/redo` restores the
//! tree captured just before the undo.
//!
//! The shadow repository shares nothing with the project's own `.git`: it has
//! its own object database and index (`GIT_DIR`) and uses the project only as
//! its work tree (`GIT_WORK_TREE`), so the project's history, index and stash
//! are never touched. It honours the project's `.gitignore`, so ignored build
//! output is neither recorded nor restored.
//!
//! Snapshots are off until [`enable`] names the store; the binary does that at
//! startup. A directory that is not inside a git work tree is never recorded,
//! which keeps a session started in a home directory from snapshotting it.
//!
//! Depends on the `git` executable and on `tokio` for the subprocesses.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

/// How long one git invocation may run before it is killed.
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Process-wide snapshot store root; unset means snapshots are disabled.
static STORE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Turns per-turn snapshots on, storing shadow repositories under `store_root`.
///
/// The first call wins; later calls are ignored.
pub fn enable(store_root: PathBuf) {
    // INTENTIONAL: a second call keeps the first store.
    let _ = STORE_ROOT.set(store_root);
}

/// The root of the git work tree containing `working_dir`, if any.
///
/// This is the directory a snapshot covers and a restore rewrites, whether or
/// not snapshots are enabled.
#[must_use]
pub fn work_tree_root(working_dir: &Path) -> Option<PathBuf> {
    working_dir
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// One project's shadow repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRepo {
    /// The project's git work-tree root.
    root: PathBuf,
    /// The shadow repository directory (`GIT_DIR`).
    git_dir: PathBuf,
}

impl SnapshotRepo {
    /// The shadow repository for the git work tree containing `working_dir`,
    /// under the store named by [`enable`].
    ///
    /// `None` when snapshots are disabled or `working_dir` is not inside a git
    /// work tree.
    #[must_use]
    pub fn for_working_dir(working_dir: &Path) -> Option<Self> {
        Self::in_store(STORE_ROOT.get()?, working_dir)
    }

    /// The shadow repository for the git work tree containing `working_dir`,
    /// under an explicit `store_root`.
    ///
    /// `None` when `working_dir` is not inside a git work tree.
    #[must_use]
    pub fn in_store(store_root: &Path, working_dir: &Path) -> Option<Self> {
        let root = work_tree_root(working_dir)?;
        // FNV-1a over the root path: a stable directory name per project.
        let hash = root
            .to_string_lossy()
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        Some(Self {
            root,
            git_dir: store_root.join(format!("{hash:016x}")),
        })
    }

    /// The project root this repository snapshots.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Runs `git <args>` against the shadow repository and returns its stdout.
    async fn git(&self, args: &[&str]) -> Result<String> {
        let child = tokio::process::Command::new("git")
            .args(["-c", "core.autocrlf=false", "-c", "core.quotepath=false"])
            .args(args)
            .env("GIT_DIR", &self.git_dir)
            .env("GIT_WORK_TREE", &self.root)
            .env_remove("GIT_INDEX_FILE")
            .current_dir(&self.root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("could not run git")?;
        // Dropping the timed-out future drops the child; `kill_on_drop` kills it.
        let output = tokio::time::timeout(GIT_TIMEOUT, child.wait_with_output())
            .await
            .with_context(|| format!("git {} timed out", args.join(" ")))??;
        if !output.status.success() {
            bail!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Records the work tree's current state and returns its tree id.
    ///
    /// Creates the shadow repository on first use.
    ///
    /// # Errors
    ///
    /// Returns an error when git is missing, fails or times out.
    pub async fn track(&self) -> Result<String> {
        if !self.git_dir.join("HEAD").exists() {
            tokio::fs::create_dir_all(&self.git_dir)
                .await
                .context("could not create the snapshot store")?;
            self.git(&["init", "--quiet"]).await?;
        }
        self.git(&["add", "--all", "."]).await?;
        self.git(&["write-tree"]).await
    }

    /// Records the pre-turn state for the turn opened by `message_id`, unless
    /// that turn already has one.
    ///
    /// # Errors
    ///
    /// Returns an error when git is missing, fails or times out.
    pub async fn record_turn(&self, session_id: &str, message_id: &str) -> Result<()> {
        if self.turn_tree(session_id, message_id).await.is_some() {
            return Ok(());
        }
        let tree = self.track().await?;
        self.git(&["update-ref", &turn_ref(session_id, message_id), &tree])
            .await?;
        Ok(())
    }

    /// The tree recorded for the turn opened by `message_id`, if any.
    pub async fn turn_tree(&self, session_id: &str, message_id: &str) -> Option<String> {
        if !self.git_dir.join("HEAD").exists() {
            return None;
        }
        self.git(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &turn_ref(session_id, message_id),
        ])
        .await
        .ok()
        .filter(|tree| !tree.is_empty())
    }

    /// Restores the work tree to `tree`.
    ///
    /// Returns the tree id of the state being replaced (so the caller can
    /// restore it again) and the paths that differ, relative to the root.
    /// Files absent from `tree` are deleted; ignored files are left alone.
    ///
    /// # Errors
    ///
    /// Returns an error when git is missing, fails or times out.
    pub async fn restore(&self, tree: &str) -> Result<(String, Vec<String>)> {
        let current = self.track().await?;
        let changed = self
            .git(&["diff-tree", "-r", "--name-only", &current, tree])
            .await?
            .lines()
            .map(str::to_string)
            .collect();
        self.git(&["read-tree", "--reset", "-u", tree]).await?;
        Ok((current, changed))
    }

    /// Sets the recorded tree of the turn opened by `message_id` to `tree`.
    ///
    /// # Errors
    ///
    /// Returns an error when git is missing, fails or times out.
    pub async fn set_turn_tree(
        &self,
        session_id: &str,
        message_id: &str,
        tree: &str,
    ) -> Result<()> {
        self.git(&["update-ref", &turn_ref(session_id, message_id), tree])
            .await
            .map(drop)
    }

    /// Drops the recorded tree of the turn opened by `message_id`.
    ///
    /// # Errors
    ///
    /// Returns an error when git is missing, fails or times out.
    pub async fn forget_turn(&self, session_id: &str, message_id: &str) -> Result<()> {
        self.git(&["update-ref", "-d", &turn_ref(session_id, message_id)])
            .await
            .map(drop)
    }
}

/// The ref that keeps a turn's pre-state tree alive.
fn turn_ref(session_id: &str, message_id: &str) -> String {
    let clean = |id: &str| -> String {
        id.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("refs/codemason/{}/{}", clean(session_id), clean(message_id))
}
