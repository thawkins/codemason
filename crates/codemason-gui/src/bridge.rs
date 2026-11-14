//! Bridge from the agent [`EventBus`] to the GUI.
//!
//! The broadcast bus drops events for a slow receiver, so (as in the terminal
//! frontend) a task drains it into an unbounded queue that the [`Shell`]
//! empties into the `App` each frame. After every forwarded event the task
//! asks egui to repaint, so the window wakes even when the user is idle.
//!
//! eframe owns the UI thread; this task runs on a tokio worker thread.
//!
//! [`Shell`]: crate::Shell

use codemason_agent::event::{Event, EventBus};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

/// Spawn the task that forwards agent events into the shell's queue.
///
/// Must be called inside a tokio runtime context.
///
/// # Arguments
///
/// * `bus` - the agent event bus to subscribe to.
/// * `ctx` - the egui context to wake with `request_repaint` after each event.
/// * `queue` - the sending half of the queue the [`crate::Shell`] drains.
///
/// # Returns
///
/// The task handle. The task ends when the bus closes or the queue's
/// receiver is dropped (the window closed).
pub fn spawn_event_bridge(
    bus: EventBus,
    ctx: egui::Context,
    queue: UnboundedSender<Event>,
) -> JoinHandle<()> {
    let mut bus_rx = bus.subscribe();
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
                }
                Err(RecvError::Closed) => break,
            }
        }
    })
}
