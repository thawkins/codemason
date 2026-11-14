//! Consolidated integration-test target for `codemason-gui`.
//!
//! Every `tests/test_*.rs` file is a child module of this one binary (see
//! AGENTS-LANGUAGE.md, "Test Organization"). The binary-wide environment lock
//! lives here so every test that touches a process-global takes the same one.

use std::sync::{Mutex, MutexGuard, OnceLock};

#[path = "test_launch.rs"]
mod test_launch;

/// The one mutex that serialises every test mutating a process-global
/// (environment variables, cwd) in this binary.
pub fn global_env_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering the guard if a previous holder
/// panicked (a poisoned lock still serialises correctly).
pub fn global_env_lock() -> MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
