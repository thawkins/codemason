//! Startup session selection.
//!
//! No dependencies.

/// Which session the TUI focuses at startup (spec MULTISESSION FR-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupSession {
    /// Reopen the sessions that were open at the last exit; start a fresh
    /// session when none can be reopened. The default for `codemason`.
    Restore,
    /// Always start a fresh session (`codemason --new`).
    New,
    /// Resume this stored session (`codemason session resume <id>`).
    Resume(String),
}
