//! The window's pane contents, one module per dock pane.
//!
//! - [`messages`] is the conversation transcript ([`crate::dock::PaneId::Messages`]).
//! - [`prompt`] is the prompt editor ([`crate::dock::PaneId::Prompt`]).
//! - [`queue`] is the input queue and its queue-control menu, drawn above
//!   the prompt.
//! - [`sessions`] is the sessions sidebar ([`crate::dock::PaneId::Sessions`]).
//! - [`log`] is the log panel ([`crate::dock::PaneId::Log`]).
//! - [`status_bar`] is the status bar along the bottom of the window.
//! - [`profile`] is the agent-loop profiler table ([`crate::dock::PaneId::Profile`]).
//! - [`tasks`] is the session's task list ([`crate::dock::PaneId::Tasks`]).
//! - [`memory`] is the project's memories ([`crate::dock::PaneId::Memory`]).
//! - [`agents`] is the Agents window table
//!   ([`crate::dock::PaneId::ActiveAgents`]).
//! - [`teams`] is the Teams window table ([`crate::dock::PaneId::Teams`]).
//! - [`rows`] is the line list those five panels draw with.

pub mod agents;
pub mod log;
pub mod memory;
pub mod messages;
pub mod profile;
pub mod prompt;
pub mod queue;
pub mod rows;
pub mod sessions;
pub mod status_bar;
pub mod tasks;
pub mod teams;
