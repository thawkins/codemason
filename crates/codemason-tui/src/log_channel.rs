//! Channel types that carry tracing log records to the app's log panel.
//!
//! Frontend-neutral: the tracing layer (render side, `tracing_layer`) sends
//! [`TuiLogRecord`]s and the frontend event loop drains them into the app.
//!
//! Depends on `tracing` (`Level`).

use std::sync::mpsc;

use tracing::Level;

/// A log record forwarded from the tracing layer to the TUI.
#[derive(Debug)]
pub struct TuiLogRecord {
    /// Severity level of the record.
    pub level: Level,
    /// Formatted log message.
    pub message: String,
}

/// Receiving end of the tracing -> TUI channel.
pub type TuiLogReceiver = mpsc::Receiver<TuiLogRecord>;

/// Sending end of the tracing -> TUI channel.
pub type TuiLogSender = mpsc::SyncSender<TuiLogRecord>;

/// Create a matched (sender, receiver) pair for the TUI tracing layer.
///
/// The `capacity` controls how many records can be buffered before senders
/// block. 512 is enough for any realistic burst.
pub fn tui_log_channel(capacity: usize) -> (TuiLogSender, TuiLogReceiver) {
    mpsc::sync_channel(capacity)
}
