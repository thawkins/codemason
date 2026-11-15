//! Custom [`tracing_subscriber::Layer`] that captures log records and forwards
//! them to the app's log panel via a channel instead of writing to
//! stdout/stderr.
//!
//! Records are buffered in a [`std::sync::mpsc`] channel (see
//! [`crate::log_channel`]) and drained by the frontend on each frame.
//!
//! Depends on `tracing` and `tracing-subscriber`.

use tracing_subscriber::Layer;

pub use crate::log_channel::{TuiLogReceiver, TuiLogRecord, TuiLogSender, tui_log_channel};

/// A [`tracing_subscriber::Layer`] that sends log records to the log panel.
///
/// Install this layer via [`tracing_subscriber::registry()`] instead of the
/// default `fmt()` subscriber when the interactive frontend is active, so that
/// all tracing output is routed to the log panel rather than stdout.
pub struct TuiTracingLayer {
    tx: TuiLogSender,
}

impl TuiTracingLayer {
    /// Create a new layer that forwards records over `tx`.
    ///
    /// # Arguments
    /// * `tx` - the sending end of [`tui_log_channel`].
    pub fn new(tx: TuiLogSender) -> Self {
        Self { tx }
    }
}

/// Visitor that extracts the `message` field and endpoint-related metadata
/// from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
    endpoint: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "endpoint" | "url" | "api_base" | "base_url" => self.endpoint = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{:?}", value),
            "endpoint" | "url" | "api_base" | "base_url" => {
                self.endpoint = Some(format!("{:?}", value));
            }
            _ => {}
        }
    }
}

impl<S> Layer<S> for TuiTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = *event.metadata().level();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // Include the target (module path) for context when it adds value.
        let target = event.metadata().target();
        let mut message = if target.starts_with("codemason") || visitor.message.is_empty() {
            visitor.message
        } else {
            format!("[{}] {}", target, visitor.message)
        };

        // Append endpoint info if present - this is how we show the full
        // Azure AI Foundry URL in the log panel.
        if let Some(endpoint) = visitor.endpoint {
            message.push_str(&format!(" -> {}", endpoint));
        }

        // Non-blocking send - drop the record if the channel is full rather
        // than stalling the async runtime.
        let _ = self.tx.try_send(TuiLogRecord { level, message }); // INTENTIONAL: channel send on a closed receiver is benign
    }
}
