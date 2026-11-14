//! Persistent storage layer.
//!
//! This module is a thin re-export of the canonical implementation in
//! [`codemason_storage`].  The agent crate previously held its own ~2,200-line
//! copy of `Storage`, `SessionRow`, `TaskRow`, `MemoryRow`, the
//! `encrypt_key` / `decrypt_key` / `obfuscate_key` / `deobfuscate_key`
//! helpers, and the knowledge-graph methods
//! (`list_entities`, `list_relationships`, `query_entity_neighbours`,
//! `has_assistant_messages`); that copy has
//! been consolidated into `codemason-storage` to eliminate the duplication (see
//! `REMPLAN.md` M2 / T2.2).
//!
//! The PERF-004 `has_format_version` cache (previously an agent-only
//! addition) and the knowledge-graph methods have all been ported into the
//! canonical `codemason_storage::Storage`, so behaviour is unchanged.  The KG
//! row types (`KgEntityRow`, `KgRelationshipRow`) live in `codemason-storage`
//! and are mapped by the agent crate's `memory::knowledge_graph` module into
//! their richer `Entity` / `Relationship` types.
//!
//! All existing `use crate::storage::{...}` sites in `codemason-agent` continue
//! to resolve via the re-exports below.

pub use codemason_storage::{
    ActivityLog, InitiativeMilestone, InitiativeRow, KgEntityRow, KgRelationshipRow, MemoryRow,
    ProjectRemoteRow, ProjectRow, RunCostSummaryRow, SessionCheckout, SessionRow, Storage, TaskRow,
    TaskUpdateParams, decrypt_key, deobfuscate_key, encrypt_key, obfuscate_key,
};
