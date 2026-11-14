//! Event streaming infrastructure for codemason sessions.
//!
//! The [`EventBus`] broadcasts [`Event`] values to any number of subscribers
//! using a Tokio broadcast channel. Events cover the full lifecycle of a
//! session: creation, message streaming, tool calls, permission gates,
//! agent switches, errors, and token usage.

pub use codemason_types::event::{Event, EventBus, FinishReason};
