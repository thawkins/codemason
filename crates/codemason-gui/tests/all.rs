//! Consolidated integration-test target for `codemason-gui`.
//!
//! Every `tests/test_*.rs` file is a child module of this one binary (see
//! AGENTS-LANGUAGE.md, "Test Organization"). The binary-wide environment lock
//! lives here so every test that touches a process-global takes the same one.

// Each child file declares `#[path = "support/mod.rs"] mod support;`, so the
// helper is loaded once per child (AGENTS-LANGUAGE.md, "Test Organization").
#![allow(clippy::duplicate_mod)]

use std::sync::{Mutex, MutexGuard, OnceLock};

#[path = "test_dialogs.rs"]
mod test_dialogs;
#[path = "test_dialogs_more.rs"]
mod test_dialogs_more;
#[path = "test_dock.rs"]
mod test_dock;
#[path = "test_housekeeping.rs"]
mod test_housekeeping;
#[path = "test_keys.rs"]
mod test_keys;
#[path = "test_launch.rs"]
mod test_launch;
#[path = "test_main_thread.rs"]
mod test_main_thread;
#[path = "test_markdown.rs"]
mod test_markdown;
#[path = "test_messages.rs"]
mod test_messages;
#[path = "test_overlays.rs"]
mod test_overlays;
#[path = "test_palette.rs"]
mod test_palette;
#[path = "test_panels.rs"]
mod test_panels;
#[path = "test_prompt.rs"]
mod test_prompt;
#[path = "test_side_panels.rs"]
mod test_side_panels;
#[path = "test_theme.rs"]
mod test_theme;

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
