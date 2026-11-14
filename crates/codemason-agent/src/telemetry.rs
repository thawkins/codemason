//! Telemetry integration for the codemason agent crate.
//!
//! This module re-exports the public telemetry API from `codemason-telemetry` so
//! that `codemason-agent` can record metrics without depending on a Cargo feature
//! flag. The actual OTEL implementation is enabled by default in
//! `codemason-telemetry`; when disabled at compile time, all recorders are
//! zero-overhead no-ops.

pub use codemason_telemetry::logs::{ExportCell, TracingLogsLayer};
pub use codemason_telemetry::shutdown::{ShutdownGuard, flush_on_signal_arc};
pub use codemason_telemetry::{
    InstrumentRegistry, LlmRecorder, OtelConfig, OtelProtocol, ProfileRecorder, SessionRecorder,
    TelemetryConfig, TelemetryState, TelemetrySubsystem, ToolRecorder,
};
