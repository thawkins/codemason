//! CO-15: the shared first-byte timeout keeps each provider's wording.

use codemason_llm::providers::http_client::send_within_first_byte_timeout;

/// A request with no response inside the budget fails with the caller's
/// timeout message, verbatim.
#[tokio::test]
async fn test_first_byte_timeout_uses_the_callers_message() {
    let never = std::future::pending::<reqwest::Result<reqwest::Response>>();
    let timeout_secs = 0;
    let err = send_within_first_byte_timeout(
        never,
        timeout_secs,
        "ollama",
        "http://localhost:11434/api/chat",
        || format!("Ollama: initial response timed out after {timeout_secs}s"),
        || "unused".to_string(),
    )
    .await
    .expect_err("an unanswered request must time out");
    assert_eq!(
        err.to_string(),
        "Ollama: initial response timed out after 0s"
    );
}

/// A transport failure carries the caller's context above the reqwest error.
#[tokio::test]
async fn test_first_byte_timeout_adds_the_send_context() {
    // Port 9 (discard) on loopback: nothing listens, so the connect fails fast.
    let url = "http://127.0.0.1:9/";
    let send = reqwest::Client::new().post(url).send();
    let err = send_within_first_byte_timeout(
        send,
        30,
        "openrouter",
        url,
        || "unused".to_string(),
        || format!("Failed to connect to OpenRouter at {url}"),
    )
    .await
    .expect_err("nothing listens on the discard port");
    assert_eq!(
        err.to_string(),
        format!("Failed to connect to OpenRouter at {url}")
    );
    assert!(
        err.chain().count() >= 2,
        "the reqwest error stays as the source"
    );
}
