//! Secret redaction facade.
//!
//! Re-exports the single process-wide sanitisation implementation from
//! [`codemason_types::sanitize`]. A `sanitize` module in this crate previously held
//! a byte-identical second copy with its own private secret registry - so a
//! credential registered through `codemason_agent::sanitize::register_secret`
//! (or `codemason_storage`'s re-export) was invisible to
//! `codemason_server::sse::redact_secrets` and vice versa. Re-exporting the
//! `codemason_types` module makes one registry serve every subsystem (PERF-055).
//!
//! Callers continue to use `crate::sanitize::...` / `codemason_agent::sanitize::...`
//! unchanged.

pub use codemason_types::sanitize::*;
