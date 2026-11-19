//! `SQLite` persistence layer for codemason
//!
//! This crate provides:
//! - Session storage (conversations, messages, history)
//! - Memory storage (memory blocks, structured memories)
//! - Snapshot storage (file snapshots, diffs)
//! - Team storage (team state, mailboxes, tasks)

#![deny(unsafe_code)]

pub mod activity_log;
pub mod project;
pub mod snapshot;
pub mod storage;

// Re-export commonly used types
pub use activity_log::{ActivityLog, AppendError};
pub use project::{
    ProjectRemoteRow, ProjectRow, SessionCheckout, default_project_name, project_root,
};
pub use snapshot::{IncrementalSnapshot, Snapshot};
pub use storage::{
    AutofixBinding, AutofixState, BackgroundTaskRow, ConversationStats, CronEventRow, CycleError,
    InitiativeMilestone, InitiativeRow, KgEntityRow, KgRelationshipRow, MemoryRow,
    MessageSearchResult, ProjectSessions, RunCostSummaryRow, SCHEMA_VERSION, SessionKind,
    SessionRequest, SessionRow, SessionSearchParams, SidebarSection, Storage, TaskDerived, TaskRow,
    TaskUpdateParams, TaskView, compute_task_dag, decrypt_key, detect_cycle, encrypt_key,
};

// Automation run-history types (spec `openhands` T-016; FR-018). Re-exported
// so callers of the run-history storage methods need not depend on
// `codemason-types` directly.
pub use codemason_types::{AutomationRun, AutomationTrigger, RunOutcome};

// Re-export sanitize module from codemason_types
pub use codemason_types::sanitize;
