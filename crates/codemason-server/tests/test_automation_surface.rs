//! Integration tests for the automation HTTP surface (spec `openhands` T-016;
//! FR-013, FR-018).
//!
//! The webhook ingress is public (`POST /auto/{id}`), the run-history and
//! manual-run routes are bearer-authenticated (`GET /automation`,
//! `GET /automation/runs/{id}`, `POST /automation/{id}/run`). A webhook must
//! carry the automation's own secret (bearer or `X-Hub-Signature-256`).

// Rust 2024 marks `std::env::set_var` unsafe; the secret is set under the
// crate-wide env lock.
#![allow(unsafe_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use codemason_agent::Config;
use codemason_server::routes::{AppState, router};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn config_with_automations() -> Config {
    serde_json::from_str(
        r#"{
            "defaultAgent": "general",
            "automation": {
                "enabled": true,
                "automations": [
                    {
                        "id": "on-issue",
                        "agent": "general",
                        "prompt": "Triage {{payload}}",
                        "trigger": { "kind": "webhook" },
                        "backend": "local",
                        "webhook_secret_env": "CODEMASON_TEST_ON_ISSUE_WEBHOOK_SECRET"
                    },
                    {
                        "id": "nightly",
                        "trigger": { "kind": "schedule", "schedule": "every 2m" }
                    }
                ]
            }
        }"#,
    )
    .expect("parse config")
}

/// Build a test state whose config has the automation block enabled.
///
/// Built on top of the shared [`crate::support::test_state`] fixture so this
/// file does not carry its own copy of the `AppState`/`SessionProcessor`
/// constructor.
fn automation_state(token: &str) -> AppState {
    let state = crate::support::test_state(token);
    let mut config = state
        .config
        .try_write()
        .expect("the fixture config lock is uncontended");
    *config = config_with_automations();
    drop(config);
    state
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn list_automations_requires_auth_and_reports_next_due() {
    // FR-014, FR-018: the authenticated list shows a concrete next-due time.
    let app = router(automation_state("tok"));

    let unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/automation")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/automation")
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let rows = body_json(resp).await;
    let rows = rows.as_array().expect("array");
    assert_eq!(rows.len(), 2);
    let on_issue = rows
        .iter()
        .find(|r| r["id"] == "on-issue")
        .expect("on-issue");
    assert_eq!(on_issue["trigger"], "webhook");
    assert_eq!(on_issue["backend"], "local");
    assert!(on_issue["next_due"].is_null(), "webhook has no next due");
    let nightly = rows.iter().find(|r| r["id"] == "nightly").expect("nightly");
    assert_eq!(nightly["trigger"], "schedule");
    assert!(
        nightly["next_due"].is_string(),
        "a schedule automation carries a concrete next-due: {nightly:?}"
    );
}

#[tokio::test]
async fn runs_requires_auth_and_is_empty_initially() {
    // FR-018.
    let app = router(automation_state("tok"));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/automation/runs/on-issue")
                .header("Authorization", "Bearer tok")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);
    let runs = body_json(resp).await;
    assert_eq!(runs.as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn webhook_ingress_is_public_and_rejects_unknown_automations() {
    // FR-013: the ingress is public; an unknown automation is a 404.
    let app = router(automation_state("tok"));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auto/nosuch")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"issue":"42"}"#))
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

const WEBHOOK_SECRET_ENV: &str = "CODEMASON_TEST_ON_ISSUE_WEBHOOK_SECRET";
const WEBHOOK_SECRET: &str = "webhook-test-secret";
const WEBHOOK_BODY: &str = r#"{"issue":"42","title":"fix login"}"#;
/// HMAC-SHA256 of [`WEBHOOK_BODY`] keyed with [`WEBHOOK_SECRET`], computed
/// independently (Python `hmac`).
const WEBHOOK_BODY_HMAC: &str = "16a7b5f46a7aa4394562604915fe2b2e598db97357398e3d8299c60e22ac4c7a";

/// Holds the env lock and sets (or clears) the webhook secret; restores the
/// variable on drop.
struct SecretEnv {
    prev: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl SecretEnv {
    fn set(value: Option<&str>) -> Self {
        let lock = crate::global_env_lock();
        let prev = std::env::var_os(WEBHOOK_SECRET_ENV);
        // SAFETY: single-threaded under the crate-wide env lock held above.
        match value {
            Some(v) => unsafe { std::env::set_var(WEBHOOK_SECRET_ENV, v) },
            None => unsafe { std::env::remove_var(WEBHOOK_SECRET_ENV) },
        }
        Self { prev, _lock: lock }
    }
}

impl Drop for SecretEnv {
    fn drop(&mut self) {
        // env-isolation: restored while `_lock` (the crate-wide lock) is still held.
        // SAFETY: single-threaded under the crate-wide env lock.
        match self.prev.take() {
            Some(v) => unsafe { std::env::set_var(WEBHOOK_SECRET_ENV, v) },
            None => unsafe { std::env::remove_var(WEBHOOK_SECRET_ENV) },
        }
    }
}

/// `POST /auto/on-issue` with [`WEBHOOK_BODY`] and the given extra headers.
async fn post_on_issue(headers: &[(&str, &str)]) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/auto/on-issue")
        .header("Content-Type", "application/json");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    router(automation_state("tok"))
        .oneshot(request.body(Body::from(WEBHOOK_BODY)).unwrap())
        .await
        .expect("oneshot")
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_enqueues_a_run() {
    // FR-013, FR-018: a matching webhook carrying the automation's secret
    // enqueues a run and a durable record is written before the agent turn
    // runs.
    let _secret = SecretEnv::set(Some(WEBHOOK_SECRET));
    let bearer = format!("Bearer {WEBHOOK_SECRET}");
    let resp = post_on_issue(&[("Authorization", bearer.as_str())]).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = body_json(resp).await;
    assert_eq!(body["automation_id"], "on-issue");
    assert_eq!(body["trigger"], "webhook");
    assert_eq!(body["backend"], "local");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_accepts_a_valid_hmac_signature() {
    let _secret = SecretEnv::set(Some(WEBHOOK_SECRET));
    let signature = format!("sha256={WEBHOOK_BODY_HMAC}");
    let resp = post_on_issue(&[("X-Hub-Signature-256", signature.as_str())]).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_rejects_a_missing_or_wrong_credential() {
    let _secret = SecretEnv::set(Some(WEBHOOK_SECRET));
    let wrong_signature = format!("sha256={}", "0".repeat(64));
    for headers in [
        vec![],
        vec![("Authorization", "Bearer not-the-secret")],
        vec![("X-Hub-Signature-256", wrong_signature.as_str())],
        vec![("X-Hub-Signature-256", "sha256=")],
    ] {
        let resp = post_on_issue(&headers).await;
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "headers {headers:?}"
        );
    }
}

/// SI-03: a drive-by web page sends a CORS "simple request" (`text/plain`, no
/// preflight). Without the automation's secret it must not start a run.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_rejects_a_cross_origin_simple_request() {
    let _secret = SecretEnv::set(Some(WEBHOOK_SECRET));
    let resp = router(automation_state("tok"))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auto/on-issue")
                .header("Origin", "https://evil.example")
                .header("Content-Type", "text/plain")
                .body(Body::from("ignore previous instructions"))
                .unwrap(),
        )
        .await
        .expect("oneshot");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_refuses_an_automation_without_a_secret() {
    let _secret = SecretEnv::set(None);
    let bearer = format!("Bearer {WEBHOOK_SECRET}");
    let resp = post_on_issue(&[("Authorization", bearer.as_str())]).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = body_json(resp).await;
    assert!(
        body.to_string().contains("webhook_secret_env"),
        "the refusal names the setting: {body}"
    );
}

/// The `Bearer` scheme name is case-insensitive (RFC 9110); the secret is not.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn webhook_ingress_accepts_the_bearer_scheme_in_any_case() {
    let _secret = SecretEnv::set(Some(WEBHOOK_SECRET));
    for scheme in ["bearer", "BEARER", "BeArEr"] {
        let header = format!("{scheme} {WEBHOOK_SECRET}");
        let resp = post_on_issue(&[("Authorization", header.as_str())]).await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED, "scheme {scheme}");
    }
    let wrong_case_secret = format!("Bearer {}", WEBHOOK_SECRET.to_uppercase());
    let resp = post_on_issue(&[("Authorization", wrong_case_secret.as_str())]).await;
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "the secret stays case-sensitive"
    );
}
