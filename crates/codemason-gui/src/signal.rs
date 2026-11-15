//! Graceful shutdown on Ctrl+C and SIGTERM.
//!
//! The terminal frontend ran its full teardown when the process received
//! SIGINT or SIGTERM (Ctrl+C on Windows). The window does the same: a task on
//! the runtime waits for the signal, sets a shared flag and wakes egui;
//! [`crate::Shell::logic`] sees the flag and quits through the normal quit
//! path (history save, MCP child kill, layout and geometry save, with the
//! 3 s exit safety net armed). The repaint makes the flag seen even while
//! the window is idle or hidden.
//!
//! Dependencies: `tokio::signal` (the `signal` feature) and `egui`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::task::JoinHandle;

/// Spawn the task that waits for a shutdown signal.
///
/// Must be called inside a tokio runtime context.
///
/// # Arguments
///
/// * `ctx` - the egui context to wake once the signal arrives.
///
/// # Returns
///
/// The flag the task sets when SIGINT or SIGTERM (Ctrl+C on Windows)
/// arrives, for [`crate::Shell::with_shutdown_signal`], and the task handle.
pub fn spawn_shutdown_signal_task(ctx: egui::Context) -> (Arc<AtomicBool>, JoinHandle<()>) {
    let flag = Arc::new(AtomicBool::new(false));
    let set = Arc::clone(&flag);
    let task = tokio::spawn(async move {
        if !wait_for_signal().await {
            return;
        }
        set.store(true, Ordering::Relaxed);
        ctx.request_repaint();
    });
    (flag, task)
}

/// Wait for SIGINT or SIGTERM.
///
/// # Returns
///
/// `true` once a signal arrived; `false` when the handlers could not be
/// installed (logged), so the process keeps the default disposition.
#[cfg(unix)]
async fn wait_for_signal() -> bool {
    use tokio::signal::unix::{SignalKind, signal};
    let handlers = signal(SignalKind::interrupt())
        .and_then(|int| signal(SignalKind::terminate()).map(|term| (int, term)));
    let (mut sigint, mut sigterm) = match handlers {
        Ok(handlers) => handlers,
        Err(e) => {
            tracing::warn!(error = %e, "could not install shutdown signal handlers");
            return false;
        }
    };
    tokio::select! {
        _ = sigint.recv() => tracing::info!("SIGINT received, initiating graceful shutdown"),
        _ = sigterm.recv() => tracing::info!("SIGTERM received, initiating graceful shutdown"),
    }
    true
}

/// Wait for Ctrl+C.
///
/// # Returns
///
/// `true` once Ctrl+C arrived; `false` when the handler could not be
/// installed (logged).
#[cfg(not(unix))]
async fn wait_for_signal() -> bool {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Ctrl+C received, initiating graceful shutdown");
            true
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not install the Ctrl+C handler");
            false
        }
    }
}
