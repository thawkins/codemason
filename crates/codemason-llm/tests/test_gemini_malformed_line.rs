//! Regression tests for the Gemini stream parser's handling of malformed lines
//! (LC-02).
//!
//! An unparseable complete line used to be pushed back into the buffer with
//! its own `\n`, so no later chunk could ever complete it. Every frame behind
//! it was held until three more chunks arrived, and when the stream ended
//! first the remainder was parsed as one JSON value and lost. These tests
//! drive the real `GeminiProvider` against a local mock HTTP server.

use std::collections::HashMap;
use std::sync::Arc;

use codemason_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use codemason_llm::{GeminiProvider, Provider};
use codemason_types::event::FinishReason;
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn make_request() -> ChatRequest {
    ChatRequest {
        model: "gemini-2.0-flash".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: Some(64),
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    }
}

/// Serve `body` once as the streaming response and return the base URL.
async fn spawn_server(body: String) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    Ok(format!("http://{addr}"))
}

/// Run one chat against a server answering `body` and collect every event.
async fn collect_events(body: &str) -> Vec<StreamEvent> {
    let url = spawn_server(body.to_string()).await.expect("server");
    let client = GeminiProvider
        .create_client("", Some(&url), &HashMap::new())
        .await
        .expect("client");
    let mut stream = client.chat(make_request()).await.expect("chat");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    events
}

/// Assert the events are exactly one `TextDelta "hi"` followed by `Finish(Stop)`.
fn assert_hi_then_finish(events: &[StreamEvent]) {
    let texts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["hi"], "events: {events:?}");
    assert!(
        matches!(
            events.last(),
            Some(StreamEvent::Finish {
                reason: FinishReason::Stop
            })
        ),
        "the stream must end with Finish(Stop): {events:?}"
    );
}

const GOOD_FRAME: &str =
    "{\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"hi\"}]},\"finishReason\":\"STOP\"}]}";

#[tokio::test]
async fn test_gemini_malformed_line_does_not_drop_following_frames() {
    let events = collect_events(&format!("garbage\n{GOOD_FRAME}\n")).await;
    assert_hi_then_finish(&events);
}

#[tokio::test]
async fn test_gemini_malformed_line_between_frames_is_skipped() {
    let body = format!(
        "{{\"candidates\":[{{\"content\":{{\"parts\":[{{\"text\":\"\"}}]}}}}]}}\n{{not json\n{GOOD_FRAME}\n"
    );
    let events = collect_events(&body).await;
    assert_hi_then_finish(&events);
}

#[tokio::test]
async fn test_gemini_unterminated_final_frames_are_parsed_line_by_line() {
    // No trailing newline: the leftover buffer holds a malformed line and a
    // complete frame; the end-of-stream handler must parse each line.
    let events = collect_events(&format!("garbage\n{GOOD_FRAME}")).await;
    assert_hi_then_finish(&events);
}
