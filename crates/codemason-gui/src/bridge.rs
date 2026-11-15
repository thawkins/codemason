//! Bridge from the agent `EventBus` to the GUI.
//!
//! The broadcast bus drops events for a slow receiver, so (as in the terminal
//! frontend) a task drains it into an unbounded queue that the [`Shell`]
//! empties into the `App` each frame. After every forwarded event the task
//! asks egui to repaint, so the window wakes even when the user is idle.
//!
//! eframe owns the UI thread; this task runs on a tokio worker thread.
//!
//! The receiver is subscribed by [`crate::run_gui`] before the App's startup
//! runs, so events published during startup (instruction-file notices,
//! early watcher and scheduler events) are queued for the window. A lag is
//! added to the App's shared lag counter (`App::tui_event_lag`), which the
//! Agents panel reconcile poll watches to repair dropped subagent events.
//!
//! [`Shell`]: crate::Shell

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use codemason_agent::event::Event;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

/// Spawn the task that forwards agent events into the shell's queue.
///
/// Must be called inside a tokio runtime context.
///
/// # Arguments
///
/// * `bus_rx` - a receiver subscribed to the agent event bus.
/// * `ctx` - the egui context to wake with `request_repaint` after each event.
/// * `queue` - the sending half of the queue the [`crate::Shell`] drains.
/// * `lag` - the App's broadcast-lag counter; each lag adds the number of
///   skipped events.
///
/// # Returns
///
/// The task handle. The task ends when the bus closes or the queue's
/// receiver is dropped (the window closed).
pub fn spawn_event_bridge(
    mut bus_rx: Receiver<Event>,
    ctx: egui::Context,
    queue: UnboundedSender<Event>,
    lag: Arc<AtomicU64>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match bus_rx.recv().await {
                Ok(event) => {
                    if queue.send(event).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("{n} broadcast events skipped in GUI bridge task");
                    lag.fetch_add(n, Ordering::Relaxed);
                    ctx.request_repaint();
                }
                Err(RecvError::Closed) => break,
            }
        }
    })
}
