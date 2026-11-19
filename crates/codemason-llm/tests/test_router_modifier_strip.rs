//! FR-020: a tier modifier routes the request but never reaches the model
//! (deep-refactor D-15, owner decision O-06 on #11).
//!
//! The downstream client here records the messages it was sent, so the tests
//! assert on what the selected model actually received.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::StreamExt;
use serde_json::Value;

use codemason_llm::llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, LlmClient, StreamEvent,
};
use codemason_llm::provider::{ModelInfo, Provider, ProviderRegistry};
use codemason_llm::providers::router_client::RouterClient;
use codemason_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
use codemason_llm::providers::router_modifiers::strip_modifiers_from_messages;

/// The messages each downstream call received, in call order.
type Seen = Arc<Mutex<Vec<Vec<ChatMessage>>>>;

/// A downstream client that records the request messages it is sent.
struct RecordingClient {
    seen: Seen,
}

#[async_trait::async_trait]
impl LlmClient for RecordingClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.seen
            .lock()
            .unwrap()
            .push(request.messages.as_ref().clone());
        Ok(Box::pin(futures::stream::empty()))
    }
}

/// A provider whose clients record into one shared `Seen` log.
struct RecordingProvider {
    seen: Seen,
}

#[async_trait::async_trait]
impl Provider for RecordingProvider {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn name(&self) -> &'static str {
        "Fake Provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(RecordingClient {
            seen: Arc::clone(&self.seen),
        }))
    }
}

/// One `fake` model per tier.
fn config() -> RouterConfig {
    let mut config = RouterConfig::default();
    for tier in Tier::all() {
        config.tiers.insert(
            tier.to_string(),
            TierConfig {
                models: vec![TierEntry {
                    provider_id: "fake".to_string(),
                    model: format!("model-{tier}"),
                }],
                timeout_ms: None,
            },
        );
    }
    config
}

fn router() -> (RouterClient, Seen) {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(RecordingProvider {
        seen: Arc::clone(&seen),
    }));
    (
        RouterClient::new(config(), Some(Arc::new(registry)), None),
        seen,
    )
}

fn text(role: &str, body: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Text(body.to_string()),
    }
}

fn request(messages: Vec<ChatMessage>) -> ChatRequest {
    ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

fn body(message: &ChatMessage) -> &str {
    match &message.content {
        ChatContent::Text(t) => t,
        ChatContent::Parts(_) => panic!("expected text content"),
    }
}

#[tokio::test]
async fn test_router_forwards_the_prompt_without_its_modifier() {
    let (client, seen) = router();
    let stream = client
        .chat(request(vec![text("user", "/simple what is 2+2")]))
        .await
        .expect("chat");
    let _ = stream.collect::<Vec<_>>().await;

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(body(&seen[0][0]), "what is 2+2");
}

#[tokio::test]
async fn test_router_strips_modifiers_from_earlier_turns_too() {
    let (client, seen) = router();
    let history = vec![
        text("user", "[complex] refactor the parser"),
        text("assistant", "/simple is how I would not start"),
        text("user", "deep mode: now prove it terminates"),
    ];
    let _ = client.chat(request(history)).await.expect("chat");

    let seen = seen.lock().unwrap();
    let sent = &seen[0];
    assert_eq!(body(&sent[0]), "refactor the parser");
    // Only user messages are routing input; assistant text is never edited.
    assert_eq!(body(&sent[1]), "/simple is how I would not start");
    assert_eq!(body(&sent[2]), "now prove it terminates");
}

#[test]
fn test_strip_keeps_a_modifier_only_message_and_plain_prompts() {
    let mut messages = Arc::new(vec![
        text("user", "/max"),
        text("user", "deep models are interesting"),
        text("user", "/unknown do this"),
    ]);
    let before = Arc::clone(&messages);
    strip_modifiers_from_messages(&mut messages);
    assert_eq!(body(&messages[0]), "/max");
    assert_eq!(body(&messages[1]), "deep models are interesting");
    assert_eq!(body(&messages[2]), "/unknown do this");
    // Nothing to strip: the shared history is not copied.
    assert!(Arc::ptr_eq(&before, &messages));
}

#[test]
fn test_strip_edits_the_first_text_part_of_a_multipart_message() {
    let mut messages = Arc::new(vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![
            ContentPart::Text {
                text: "[simple] describe this image".to_string(),
            },
            ContentPart::Text {
                text: "[simple] second part".to_string(),
            },
        ]),
    }]);
    strip_modifiers_from_messages(&mut messages);
    let ChatContent::Parts(parts) = &messages[0].content else {
        panic!("expected parts");
    };
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|p| match p {
            ContentPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["describe this image", "[simple] second part"]);
}
