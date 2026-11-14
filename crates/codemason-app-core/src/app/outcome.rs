//! Result of dispatching one input event to the [`App`](crate::app::App).
//!
//! No dependencies.

/// What [`App::handle_key`](crate::app::App::handle_key) did with a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
    /// The key was dispatched and the app keeps running.
    Consumed,
    /// The key was not handled. Reserved: the current dispatcher routes every
    /// press somewhere (at worst the input editor), so it never returns this.
    Ignored,
    /// The key made the app stop running (`App::is_running` is now `false`).
    Quit,
}
