//! Typed newtype wrappers for identifiers used throughout codemason.
//!
//! This module is a thin re-export of the canonical implementation in
//! [`codemason_types::id`]. The agent crate previously held its own copy of the
//! `define_id!` macro and the id newtypes; those have been consolidated into
//! `codemason-types` to eliminate the duplication (see `ANTIPAT.md` M3.1).
//!
//! All types below are re-exported verbatim, so existing `use crate::id::*`
//! sites continue to resolve unchanged.

pub use codemason_types::id::{MessageId, ProviderId, SessionId, ToolCallId};
