//! The window's pane contents, one module per dock pane.
//!
//! - [`messages`] is the conversation transcript ([`crate::dock::PaneId::Messages`]).
//! - [`prompt`] is the prompt editor ([`crate::dock::PaneId::Prompt`]).
//! - [`queue`] is the input queue and its queue-control menu, drawn above
//!   the prompt.
//! - [`sessions`] is the sessions sidebar ([`crate::dock::PaneId::Sessions`]).
//! - [`log`] is the log panel ([`crate::dock::PaneId::Log`]).
//! - [`status_bar`] is the status bar along the bottom of the window.

pub mod log;
pub mod messages;
pub mod prompt;
pub mod queue;
pub mod sessions;
pub mod status_bar;
