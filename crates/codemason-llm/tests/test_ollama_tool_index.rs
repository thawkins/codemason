//! Ollama streams key a tool call by its id when the server omits `index`
//! (#19): parallel calls must not be merged into one, and a server that sends
//! neither index nor id on continuation deltas still streams one call.

use std::collections::HashMap;
use std::sync::Arc;

use codemason_llm::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use codemason_llm::{OllamaProvider, Provider};
use futures::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serve one SSE response on a local port and return its base URL.
async fn serve(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Drain the request so closing does not reset the connection.
        let mut buf = [0u8; 8192];
        let _ = socket.read(&mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

async fn events_for(body: &'static str) -> Vec<StreamEvent> {
    let url = serve(body).await;
    let client = OllamaProvider::with_url(&url)
        .create_client("", None, &HashMap::new())
        .await
        .expect("client");
    let request = ChatRequest {
        model: "llama3:latest".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("weather in London and Paris".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: Some(5),
        thinking: None,
    };
    client.chat(request).await.expect("chat").collect().await
}

/// `(id, name)` of every started call, in stream order.
fn starts(events: &[StreamEvent]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallStart { id, name } => Some((id.clone(), name.clone())),
            _ => None,
        })
        .collect()
}

/// The concatenated argument deltas for one call id.
fn args_for(events: &[StreamEvent], call: &str) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallDelta { id, args_json } if id == call => Some(args_json.as_str()),
            _ => None,
        })
        .collect()
}

fn ends(events: &[StreamEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::ToolCallEnd { id } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn test_ollama_parallel_calls_without_index_stay_separate() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[",
        "{\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"London\\\"}\"}},",
        "{\"id\":\"call_b\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Paris\\\"}\"}}",
        "]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    let started = starts(&events);
    assert_eq!(
        started,
        [
            ("call_a".to_string(), "get_weather".to_string()),
            ("call_b".to_string(), "get_weather".to_string()),
        ]
    );
    assert_eq!(args_for(&events, "call_a"), "{\"location\":\"London\"}");
    assert_eq!(args_for(&events, "call_b"), "{\"location\":\"Paris\"}");
    assert_eq!(ends(&events), ["call_a", "call_b"]);
}

#[tokio::test]
async fn test_ollama_call_without_index_or_id_on_deltas_streams_as_one() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"loc\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"ation\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(starts(&events).len(), 1);
    assert_eq!(args_for(&events, "call_x"), "{\"location\":\"London\"}");
    assert_eq!(ends(&events), ["call_x"]);
}

#[tokio::test]
async fn test_ollama_indexed_start_then_id_only_delta_stays_one_call() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"loc\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_x\",\"function\":{\"arguments\":\"ation\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(starts(&events).len(), 1);
    assert_eq!(args_for(&events, "call_x"), "{\"location\":\"London\"}");
}

#[tokio::test]
async fn test_ollama_id_only_start_then_index_only_delta_stays_one_call() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"loc\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"ation\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(starts(&events).len(), 1);
    assert_eq!(args_for(&events, "call_x"), "{\"location\":\"London\"}");
}

#[tokio::test]
async fn test_ollama_anonymous_fragment_with_two_open_calls_is_skipped() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[",
        "{\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"London\\\"}\"}},",
        "{\"id\":\"call_b\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Paris\\\"}\"}}",
        "]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"garbage\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(starts(&events).len(), 2);
    assert_eq!(args_for(&events, "call_a"), "{\"location\":\"London\"}");
    assert_eq!(args_for(&events, "call_b"), "{\"location\":\"Paris\"}");
}

/// The `(id, args)` of every started call, in start order.
fn calls(events: &[StreamEvent]) -> Vec<(String, String)> {
    starts(events)
        .into_iter()
        .map(|(id, _)| {
            let args = args_for(events, &id);
            (id, args)
        })
        .collect()
}

#[tokio::test]
async fn test_ollama_distinct_indexes_with_a_duplicate_id_stay_separate() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[",
        "{\"index\":0,\"id\":\"dup\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"London\\\"}\"}},",
        "{\"index\":1,\"id\":\"dup\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Paris\\\"}\"}}",
        "]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    let calls = calls(&events);
    assert_eq!(calls.len(), 2, "two starts expected: {calls:?}");
    assert_ne!(calls[0].0, calls[1].0, "each call needs its own output id");
    assert_eq!(calls[0].1, "{\"location\":\"London\"}");
    assert_eq!(calls[1].1, "{\"location\":\"Paris\"}");
}

#[tokio::test]
async fn test_ollama_second_call_header_before_its_name_stays_separate() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_b\",\"type\":\"function\"}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"location\\\":\\\"Paris\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(
        calls(&events),
        [
            (
                "call_a".to_string(),
                "{\"location\":\"London\"}".to_string()
            ),
            ("call_b".to_string(), "{\"location\":\"Paris\"}".to_string()),
        ]
    );
}

#[tokio::test]
async fn test_ollama_continuation_repeating_the_name_stays_one_call() {
    let events = events_for(concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"loc\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"name\":\"get_weather\",\"arguments\":\"ation\\\":\\\"London\\\"}\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n"
    ))
    .await;

    assert_eq!(
        calls(&events),
        [(
            "call_x".to_string(),
            "{\"location\":\"London\"}".to_string()
        )]
    );
}
