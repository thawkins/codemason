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
//! - [`context`] is the context-window breakdown ([`crate::dock::PaneId::Context`]).
//! - [`agents`] is the Agents window table
//!   ([`crate::dock::PaneId::ActiveAgents`]).
//! - [`teams`] is the Teams window table ([`crate::dock::PaneId::Teams`]).
//! - [`research`] is the research and deck progress views, drawn above
//!   the transcript while a `/research` run is going.
//! - [`rows`] is the line list the side panels draw with.

pub mod agents;
pub mod code_block;
pub mod context;
pub mod empty_state;
pub mod log;
pub mod memory;
pub mod messages;
pub mod profile;
pub mod prompt;
pub mod queue;
pub mod research;
pub mod rows;
pub mod sessions;
pub mod status_bar;
pub mod tasks;
pub mod teams;
