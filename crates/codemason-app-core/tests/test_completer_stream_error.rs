//! `CodemasonCompleter` (the one-shot call behind `/swarm` decomposition and
//! `/research`) must treat a provider stream error as a failure, not return
//! the partial text streamed before it as a successful answer.
//!
//! The LLM call goes to an in-process scripted provider registered as
//! `router`; nothing touches the network and nothing is written to disk.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use codemason_agent::llm::{ChatRequest, LlmClient, StreamEvent};
use codemason_agent::message::MessagePart;
use codemason_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use codemason_app_core::app::App;
use codemason_config::{Capabilities, Cost};

#[path = "support/mod.rs"]
mod support;

/// Streams some text, then a provider stream error.
struct TextThenErrorClient;

#[async_trait::async_trait]
impl LlmClient for TextThenErrorClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        Ok(Box::pin(futures::stream::iter([
            StreamEvent::TextDelta {
                text: "{\"tasks\": [".to_string(),
            },
            StreamEvent::Error {
                message: "stream stalled".to_string(),
            },
        ])))
    }
}

/// A provider registered as `router`, which needs no API key.
struct ScriptedProvider;

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "router"
    }

    fn name(&self) -> &'static str {
        "Scripted stream-error provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "scripted-1".to_string(),
            provider_id: "router".to_string(),
            name: "Scripted".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                vision: false,
                tool_use: false,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
            },
            context_window: 8192,
            max_output: Some(1024),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn LlmClient>> {
        Ok(Box::new(TextThenErrorClient))
    }
}

/// Whether any chat message holds text containing `needle`.
fn chat_contains(app: &App, needle: &str) -> bool {
    app.session.messages.iter().any(|m| {
        m.parts
            .iter()
            .any(|p| matches!(p, MessagePart::Text { text } if text.contains(needle)))
    })
}

/// A stream error during `/swarm` decomposition is reported as a
/// "Swarm Error" carrying the provider's message, not handed to the
/// decomposition parser as if the partial text were the full answer.
#[tokio::test]
async fn test_completer_stream_error_surfaces_as_swarm_error() {
    let mut app = support::make_app();
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ScriptedProvider));
    app.provider_registry = Arc::new(registry);
    app.session.selected_model = Some("router/scripted-1".to_string());

    app.execute_slash_command("/swarm Harden auth").await;

    let deadline = Instant::now() + Duration::from_secs(20);
    while app.session.swarm_result.lock().expect("slot").is_none() {
        assert!(
            Instant::now() < deadline,
            "decomposition call never returned"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    app.poll_pending_swarm().await;

    assert!(chat_contains(&app, "Swarm Error"));
    assert!(chat_contains(&app, "LLM stream error: stream stalled"));
    assert!(!chat_contains(&app, "Decomposition Failed"));
}
