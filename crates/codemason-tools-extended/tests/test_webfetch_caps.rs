//! TL-08: the legacy `webfetch` tool must clamp the model-supplied
//! `max_length` and `timeout` to fixed ceilings so a huge value cannot turn
//! the streaming download cap into an unbounded read.

#![allow(unsafe_code)]
// The crate-wide env lock is a std mutex held across the body on purpose:
// the test flips the process-global SSRF bypass. See `tests/all.rs`.
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;

use codemason_tools_extended::webfetch::{MAX_LENGTH_CEILING, WebFetchTool};
use codemason_tools_extended::{Tool, ToolContext};
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: Arc::new(codemason_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: None,
        allowed_roots: Vec::new(),
    }
}

#[tokio::test]
async fn test_webfetch_huge_max_length_is_clamped() {
    let _env_lock = crate::global_env_lock();
    // SAFETY: process-global; serialised by the crate-wide env lock above.
    unsafe { std::env::set_var("CODEMASON_TOOLS_EXTENDED_TEST_NO_SSRF", "1") };

    let server = MockServer::start().await;
    let body = "a".repeat(MAX_LENGTH_CEILING + 2 * 1024 * 1024);
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain")
                .set_body_string(body),
        )
        .mount(&server)
        .await;

    let out = WebFetchTool
        .execute(
            json!({
                "url": format!("{}/big", server.uri()),
                "max_length": 1_000_000_000_000_000u64,
                "timeout": u64::MAX,
            }),
            &ctx(),
        )
        .await;
    // SAFETY: as above.
    unsafe { std::env::remove_var("CODEMASON_TOOLS_EXTENDED_TEST_NO_SSRF") };

    let out = out.expect("fetch should succeed");
    assert!(
        out.content.len() <= MAX_LENGTH_CEILING + 64,
        "content not bounded: {} bytes",
        out.content.len()
    );
    assert!(out.content.ends_with("[Content truncated]"));
}
