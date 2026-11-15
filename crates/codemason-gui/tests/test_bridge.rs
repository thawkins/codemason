//! Tests for the agent event bridge (`codemason_gui::bridge`): a broadcast
//! lag is counted in the App's shared lag counter, so the Agents panel
//! reconcile poll repairs the dropped subagent events.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use codemason_agent::event::{Event, EventBus};
use codemason_gui::bridge::spawn_event_bridge;

/// A notice event for the given index.
fn notice(i: usize) -> Event {
    Event::AgentNotice {
        session_id: "s".to_string(),
        message: format!("notice {i}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn test_bridge_lagged_events_increment_lag_counter() {
    let bus = EventBus::new(4);
    let rx = bus.subscribe();
    // The bridge task cannot run until this test yields, so 9 events on a
    // 4-slot bus overflow it by 5 before the first receive.
    for i in 0..9 {
        bus.publish(notice(i));
    }
    let lag = Arc::new(AtomicU64::new(0));
    let (queue_tx, mut queue_rx) = tokio::sync::mpsc::unbounded_channel();
    let task = spawn_event_bridge(rx, egui::Context::default(), queue_tx, Arc::clone(&lag));

    let first = queue_rx.recv().await.expect("an event after the lag");
    assert!(
        matches!(&first, Event::AgentNotice { message, .. } if message == "notice 5"),
        "got {first:?}"
    );
    assert_eq!(lag.load(Ordering::Relaxed), 5);
    task.abort();
}
