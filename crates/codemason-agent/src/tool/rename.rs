//! The tool-rename table (spec `uniformtools` FR-023, FR-031) - a re-export.
//!
//! The table itself lives in
//! [`codemason_types::tool_rename`](../../codemason_types/tool_rename/index.html)
//! because two crates must read it and must not disagree: this crate on the
//! migration and guard paths, and `codemason-config` when a configuration is loaded
//! - where a `tools.aliases` entry naming a retired spelling is refused
//! (FR-031, task T-042). `codemason-config` cannot depend on `codemason-agent`
//! (`codemason-agent` already depends on `codemason-config`), so the table sits in the
//! leaf crate both already depend on.
//!
//! Every path that read `crate::tool::rename::{Rename, TOOL_RENAMES}` keeps
//! working through the re-export below, so the move changed no call site. The
//! invariant test, the escape-hatch audit and the atomic-landing test all still
//! name this module.
//!
//! The table is not a translation layer: the registry holds the adopted
//! (`after`) spelling itself, and nothing consults the table on a dispatch path
//! (NFR-006). Retired (`before`) spellings are retired, not aliased (FR-026).

pub use codemason_types::tool_rename::{Rename, TOOL_RENAMES};
