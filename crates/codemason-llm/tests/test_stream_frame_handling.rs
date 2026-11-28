//! Stream frames the providers used to mishandle (audit batch 4).
//!
//! - CO-05: an Anthropic `event: error` frame ends the stream with an Error.
//! - CO-06: index-less parallel tool calls stay separate (keyed by id).
//! - CO-12: the Ollama Cloud done frame is logged as metadata only.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use codemason_llm::Provider;
use codemason_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serve one HTTP response with `body`, then close; returns the base URL.
async fn serve_once(content_type: &'static str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request first so the client never sees a reset.
        let mut request = [0u8; 16384];
        let _ = socket.read(&mut request).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

fn request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
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

fn model_options(model: &str) -> HashMap<String, serde_json::Value> {
    HashMap::from([("model_id".to_string(), serde_json::json!(model))])
}

#[tokio::test]
async fn test_stream_frame_handling_anthropic_error_frame_yields_an_error() {
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":3}}}\n\n",
        "event: error\n",
        "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n",
    );
    let url = serve_once("text/event-stream", body.to_string()).await;
    let client = codemason_llm::provider::anthropic::AnthropicProvider
        .create_client("key", Some(&url), &model_options("claude-x"))
        .await
        .expect("client");

    let events: Vec<StreamEvent> = client
        .chat(request("claude-x"))
        .await
        .expect("chat")
        .collect()
        .await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Error { message } if message == "Overloaded")),
        "events: {events:?}"
    );
}

#[tokio::test]
async fn test_stream_frame_handling_index_less_parallel_calls_stay_separate() {
    let frame =
        |delta: &str| format!("data: {{\"choices\":[{{\"index\":0,\"delta\":{delta}}}]}}\n\n");
    let body = [
        frame(r#"{"tool_calls":[{"id":"call_a","function":{"name":"Read","arguments":"{\"path\":\"a\"}"}}]}"#),
        frame(r#"{"tool_calls":[{"id":"call_b","function":{"name":"Grep","arguments":"{\"pattern\":\"b\"}"}}]}"#),
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n".to_string(),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    let url = serve_once("text/event-stream", body).await;
    let client = codemason_llm::provider::huggingface::HuggingFaceProvider
        .create_client("key", Some(&url), &model_options("org/model"))
        .await
        .expect("client");

    let events: Vec<StreamEvent> = client
        .chat(request("org/model"))
        .await
        .expect("chat")
        .collect()
        .await;

    let starts: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        vec!["Read".to_string(), "Grep".to_string()],
        "events: {events:?}"
    );
}

/// A `tracing` writer that appends to a shared buffer.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_stream_frame_handling_ollama_cloud_done_frame_logs_metadata_only() {
    let capture = Capture::default();
    let writer = capture.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let body = concat!(
        "{\"message\":{\"role\":\"assistant\",\"content\":\"partial \"},\"response\":\"partial \",\"done\":false}\n",
        "{\"message\":{\"role\":\"assistant\",\"content\":\"SECRET-FRAGMENT\"},\"response\":\"SECRET-FRAGMENT\",",
        "\"context\":[101,202,303],\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":5,\"eval_count\":2}\n",
    );
    let url = serve_once("application/x-ndjson", body.to_string()).await;
    let client = codemason_llm::provider::ollama_cloud::OllamaCloudProvider::new()
        .create_client("key", Some(&url), &model_options("llama"))
        .await
        .expect("client");
    let _: Vec<StreamEvent> = client
        .chat(request("llama"))
        .await
        .expect("chat")
        .collect()
        .await;

    let log = String::from_utf8_lossy(&capture.0.lock().expect("capture")).into_owned();
    assert!(log.contains("done frame received"), "log: {log}");
    assert!(
        !log.contains("SECRET-FRAGMENT"),
        "the done frame's text was logged: {log}"
    );
    assert!(
        !log.contains("101"),
        "the done frame's context ids were logged: {log}"
    );
}
