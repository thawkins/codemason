//! TL-11: GitHub/GitLab error paths must read a bounded slice of the
//! response body, so a hostile or broken server cannot force an unbounded
//! buffer into an error message.

use codemason_tools_vcs::github::GitHubClient;
use codemason_tools_vcs::gitlab::GitLabClient;
use codemason_tools_vcs::gitlab::auth::validate_token;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Bound on an error message built from a 5 MiB body.
const MESSAGE_BOUND: usize = 8 * 1024;

async fn huge_500() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_body_string("x".repeat(5 * 1024 * 1024)))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn test_error_body_cap_github_get() {
    let server = huge_500().await;
    let client = GitHubClient::with_base_url(server.uri(), "t".to_string());
    let err = client.get("/repos/a/b").await.expect_err("500 must fail");
    let msg = err.to_string();
    assert!(msg.contains("GitHub API error 500"), "got: {}", &msg[..80]);
    assert!(msg.len() <= MESSAGE_BOUND, "unbounded: {} bytes", msg.len());
}

#[tokio::test]
async fn test_error_body_cap_github_get_bytes() {
    let server = huge_500().await;
    let client = GitHubClient::with_base_url(server.uri(), "t".to_string());
    let err = client.get_bytes("/x").await.expect_err("500 must fail");
    let msg = err.to_string();
    assert!(msg.contains("GitHub API error 500"), "got: {}", &msg[..80]);
    assert!(msg.len() <= MESSAGE_BOUND, "unbounded: {} bytes", msg.len());
}

#[tokio::test]
async fn test_error_body_cap_gitlab_get() {
    let server = huge_500().await;
    let client = GitLabClient::with_credentials(server.uri(), "t".to_string());
    let err = client.get("/projects/1").await.expect_err("500 must fail");
    let msg = err.to_string();
    assert!(msg.contains("GitLab API error 500"), "got: {}", &msg[..80]);
    assert!(msg.len() <= MESSAGE_BOUND, "unbounded: {} bytes", msg.len());
}

#[tokio::test]
async fn test_error_body_cap_gitlab_text_tail() {
    let server = huge_500().await;
    let client = GitLabClient::with_credentials(server.uri(), "t".to_string());
    let err = client
        .get_text_tail("/projects/1/jobs/2/trace", 1024)
        .await
        .expect_err("500 must fail");
    let msg = err.to_string();
    assert!(msg.len() <= MESSAGE_BOUND, "unbounded: {} bytes", msg.len());
}

#[tokio::test]
async fn test_error_body_cap_gitlab_validate_token() {
    let server = huge_500().await;
    let err = validate_token(&server.uri(), "t")
        .await
        .expect_err("500 must fail");
    let msg = err.to_string();
    assert!(msg.contains("GitLab API error"), "got: {}", &msg[..80]);
    assert!(msg.len() <= MESSAGE_BOUND, "unbounded: {} bytes", msg.len());
}
