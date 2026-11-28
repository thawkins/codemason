//! AG-11: the remote execution backend must read error (and session) bodies
//! through a capped reader, so a hostile or broken server cannot force an
//! unbounded buffer into a `RemoteError` message.

use codemason_agent::backend::{RemoteBackend, RemoteErrorKind};
use codemason_config::BackendConfig;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Bound on a `RemoteError` message built from a 5 MiB body.
const MESSAGE_BOUND: usize = 8 * 1024;

fn descriptor(url: &str, id: &str) -> BackendConfig {
    BackendConfig {
        id: id.to_string(),
        name: Some("Body cap remote".to_string()),
        kind: "remote".to_string(),
        image: None,
        workspace: None,
        url: Some(url.to_string()),
        api_key: Some("test-key".to_string()),
        credentials: Vec::new(),
    }
}

fn huge() -> String {
    "x".repeat(5 * 1024 * 1024)
}

#[tokio::test]
async fn test_remote_backend_body_cap_create_session_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions"))
        .respond_with(ResponseTemplate::new(500).set_body_string(huge()))
        .mount(&server)
        .await;
    let backend = RemoteBackend::from_descriptor(&descriptor(&server.uri(), "body-cap-create"));
    let dir = std::env::temp_dir();
    let err = backend
        .create_session("body-cap-local-1", &dir)
        .await
        .expect_err("500 must fail");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    assert_eq!(err.status(), Some(500));
    assert!(
        err.message().len() <= MESSAGE_BOUND,
        "unbounded: {} bytes",
        err.message().len()
    );
}

#[tokio::test]
async fn test_remote_backend_body_cap_open_turn_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/sessions/r1/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string(huge()))
        .mount(&server)
        .await;
    let backend = RemoteBackend::from_descriptor(&descriptor(&server.uri(), "body-cap-turn"));
    let err = backend
        .open_turn("r1", "hello")
        .await
        .expect_err("500 must fail");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
    assert!(
        err.message().len() <= MESSAGE_BOUND,
        "unbounded: {} bytes",
        err.message().len()
    );
}

#[tokio::test]
async fn test_remote_backend_body_cap_oversized_session_body_refused() {
    let server = MockServer::start().await;
    let body = format!("{{\"id\":\"s1\",\"pad\":\"{}\"}}", huge());
    Mock::given(method("POST"))
        .and(path("/sessions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    let backend = RemoteBackend::from_descriptor(&descriptor(&server.uri(), "body-cap-ok"));
    let dir = std::env::temp_dir();
    let err = backend
        .create_session("body-cap-local-2", &dir)
        .await
        .expect_err("an oversized session body must be refused");
    assert_eq!(err.kind(), RemoteErrorKind::Protocol);
}
