//! Keeps every test binary away from the developer's real user profile.
//!
//! Many tests exercise code that writes user-global state: the global
//! `codemason.json`, the plugin ledger, the team store, fetched catalogues.
//! Without a redirect those writes land in the developer's own
//! `~/.config/codemason` (`%APPDATA%\codemason` on Windows).
//!
//! codemason resolves its user-global directories through `XDG_CONFIG_HOME`,
//! `HOME` and `XDG_DATA_HOME` on every platform (see
//! `codemason_config::Config::global_config_dir` and
//! `codemason_config::user_dirs`). This constructor points all three at a
//! per-process directory under the git-ignored `target/temp/test-profile/`
//! before any test starts, so a run never touches the real profile. A test
//! that sets one of these variables itself still does so under the crate's
//! `global_env_lock`, and restores this redirect when it is done.
//!
//! Each crate's `tests/all.rs` (and the root `tests/all.rs`) includes this file
//! with `#[path]`, so the redirect is defined once.
//!
//! Dependencies: `ctor` (dev-dependency), to run before the test harness.

use std::path::{Path, PathBuf};

/// The workspace root: the nearest ancestor of this crate holding `Cargo.lock`.
fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|dir| dir.join("Cargo.lock").is_file())
        .unwrap_or(manifest)
        .to_path_buf()
}

/// Redirect the user-profile variables to `target/temp/test-profile/<crate>-<pid>`.
///
/// The directories are created lazily by whatever code writes to them.
#[ctor::ctor(unsafe)]
fn isolate_user_profile() {
    let root = workspace_root()
        .join("target")
        .join("temp")
        .join("test-profile")
        .join(format!("{}-{}", env!("CARGO_PKG_NAME"), std::process::id()));
    for (var, sub) in [
        ("XDG_CONFIG_HOME", "config"),
        ("HOME", "home"),
        ("XDG_DATA_HOME", "data"),
    ] {
        // SAFETY: a `ctor` runs before `main`, so no other thread exists yet
        // to read the environment concurrently.
        unsafe { std::env::set_var(var, root.join(sub)) };
    }
}
