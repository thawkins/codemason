//! The window's pane contents, one module per dock pane.
//!
//! - [`messages`] is the conversation transcript ([`crate::dock::PaneId::Messages`]).
//! - [`prompt`] is the prompt editor ([`crate::dock::PaneId::Prompt`]).
//! - [`queue`] is the input queue and its queue-control menu.

pub mod messages;
pub mod prompt;
pub mod queue;
