//! Tests for recording model-step events in the activity log.
//!
//! A model-step event captures how one model response ended (finish reason,
//! token counts, visible text and reasoning sizes, tool-call count). Comparing
//! a step that ended a turn with a normal tool-calling step is how a stalled
//! agent loop is diagnosed, so the fields must survive a write and a read.

#![forbid(unsafe_code)]

use codemason_storage::activity_log::ActivityLog;
use codemason_types::activity::{ACTIVITY_EVENT_SCHEMA_VERSION, EventKind, ModelStep};
use codemason_types::id::RunId;

fn sample_step() -> ModelStep {
    ModelStep {
        step: 7,
        model: "ollama_cloud/deepseek-v4.1-flash:1017".to_string(),
        finish_reason: "stop".to_string(),
        input_tokens: 216_876,
        output_tokens: 273,
        text_bytes: 74,
        reasoning_bytes: 410,
        tool_calls: 0,
        elapsed_ms: 3_041,
    }
}

#[test]
fn test_activity_model_step_round_trips_all_fields() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log.record_model_step(&run, sample_step()).expect("record");
    assert_eq!(event.schema_version, ACTIVITY_EVENT_SCHEMA_VERSION);

    let read = log.read_run(&run).expect("read");
    assert_eq!(read.len(), 1);
    match &read[0].kind {
        EventKind::ModelStep(step) => assert_eq!(step, &sample_step()),
        other => panic!("expected ModelStep, got {other:?}"),
    }
}

#[test]
fn test_activity_model_step_serialises_flat_with_kind_tag() {
    // `/alog export` and direct sqlite reads see one flat JSON object.
    let json = serde_json::to_value(EventKind::ModelStep(sample_step())).expect("json");
    assert_eq!(json["kind"], "model_step");
    assert_eq!(json["finish_reason"], "stop");
    assert_eq!(json["output_tokens"], 273);
    assert_eq!(json["tool_calls"], 0);
}
