//! Frontend-neutral `app` leaf modules moved out of `codemason-tui`.
//!
//! Holds the ratatui-free, `App`-independent helpers (toolchain detection,
//! blueprints, MCP bridge, markdown worker, prompt rendering, cron and skill
//! generation). `codemason-tui` re-exports each module so existing
//! `crate::app::<module>` paths keep resolving.

pub mod blueprints;
pub mod cron;
pub mod mcp_bridge;
pub mod md_worker;
pub mod prompt;
pub mod skillgen;
pub mod toolchain;
