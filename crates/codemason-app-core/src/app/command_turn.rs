//! Running a slash command's prompt as an agent turn.
//!
//! A command that injects a prompt (a plugin prompt command, a user-defined
//! command) marks its session busy before the turn starts, so the turn must
//! always end with a terminal event that clears the busy state. The processor
//! publishes `AgentError` itself for most failures (an unknown provider, a
//! missing API key, a failed model call) but returns a bare error for others
//! (a storage failure before the turn starts). [`run_command_turn`] gives the
//! report one owner: it publishes `AgentError` only when the processor did not.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codemason_agent::agent::AgentInfo;
use codemason_agent::event::{Event, EventBus};
use codemason_agent::session::processor::SessionProcessor;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

/// Runs `prompt` as a turn of `session_id` and guarantees one terminal error
/// event when the turn fails.
///
/// The bus is watched while the turn runs. If the turn returns an error and no
/// `AgentError` for this session was seen, one is published with `label` as
/// its prefix; if the processor already reported the failure, nothing more is
/// published, because a second terminal error would end whatever turn the
/// first one allowed the input queue to start.
pub async fn run_command_turn(
    processor: Arc<SessionProcessor>,
    event_bus: Arc<EventBus>,
    session_id: String,
    prompt: String,
    agent: AgentInfo,
    cancel_flag: Arc<AtomicBool>,
    label: &'static str,
) {
    let is_own_error = |event: &Event| matches!(event, Event::AgentError { session_id: sid, .. } if *sid == session_id);
    let mut events = event_bus.subscribe();
    let mut reported = false;
    let result = {
        let turn = processor.process_message(&session_id, &prompt, &agent, cancel_flag);
        tokio::pin!(turn);
        let mut bus_open = true;
        loop {
            tokio::select! {
                result = &mut turn => break result,
                event = events.recv(), if bus_open => match event {
                    Ok(event) => reported |= is_own_error(&event),
                    // Skipped events cannot be inspected; keep watching.
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => bus_open = false,
                },
            }
        }
    };
    let Err(error) = result else {
        return;
    };
    // Events published just before the turn returned are still buffered.
    loop {
        match events.try_recv() {
            Ok(event) => reported |= is_own_error(&event),
            Err(TryRecvError::Lagged(_)) => {}
            Err(TryRecvError::Empty | TryRecvError::Closed) => break,
        }
    }
    tracing::warn!(%error, label, "command turn failed");
    if !reported {
        event_bus.publish(Event::AgentError {
            session_id,
            error: format!("{label} failed: {error}"),
        });
    }
}
