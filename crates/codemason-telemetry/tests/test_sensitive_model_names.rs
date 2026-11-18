//! Model-name attributes keep Ollama `name:tag` ids while secrets stay
//! redacted (FR-034; deep-refactor D-11, owner decision O-06 on #11).

#![cfg(feature = "telemetry")]

use codemason_telemetry::InstrumentRegistry;
use codemason_telemetry::cardinality::CardinalityCache;
use codemason_telemetry::logs::{LogEvent, convert_event};
use codemason_telemetry::sensitive::{
    REDACTED, is_model_attr_key, is_ollama_model_id, looks_sensitive, looks_sensitive_model_name,
    sanitize_attr_for_key,
};
use opentelemetry::KeyValue;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::{Context, SubscriberExt as _};

/// Ollama-style ids whose two colon parts are both dense and 4+ chars long,
/// which the generic `key:secret` rule treats as credentials.
const NAME_TAG_IDS: &[&str] = &["llama3:latest", "gpt-oss:120b", "mistral:instruct"];

/// Credential-shaped values that must stay redacted even under a model key.
const CREDENTIALS: &[&str] = &[
    "admin:password1234",
    "user:secretpassword123",
    "keyid:secretvalue1234",
];

const SECRETS: &[&str] = &[
    "admin:password1234",
    "sk-proj-abc123def456",
    "Bearer abc123def456",
    "ghp_abcdefghijklmnop",
    "line1\nline2",
    "token=abcdefghijklmnopqrstuvwxyz",
];

fn attr_value(kv: &KeyValue) -> String {
    kv.value.as_str().into_owned()
}

#[test]
fn test_model_name_check_keeps_name_tag_ids_the_generic_check_redacts() {
    for id in NAME_TAG_IDS {
        assert!(looks_sensitive(id), "generic rule premise: {id}");
        assert!(!looks_sensitive_model_name(id), "model name redacted: {id}");
    }
}

#[test]
fn test_model_name_check_still_redacts_secrets() {
    for secret in SECRETS {
        assert!(
            looks_sensitive_model_name(secret),
            "secret kept: {secret:?}"
        );
    }
    assert!(looks_sensitive_model_name(&"m".repeat(300)));
}

#[test]
fn test_model_attr_keys() {
    for key in ["model", "selected_model", "gen_ai.request.model"] {
        assert!(is_model_attr_key(key), "{key}");
    }
    for key in ["component", "provider", "modelling", "session.id"] {
        assert!(!is_model_attr_key(key), "{key}");
    }
}

#[test]
fn test_sanitize_attr_for_key_only_relaxes_model_keys() {
    assert_eq!(
        sanitize_attr_for_key("model", "llama3:latest"),
        "llama3:latest"
    );
    assert_eq!(
        sanitize_attr_for_key("component", "llama3:latest"),
        REDACTED
    );
    assert_eq!(
        sanitize_attr_for_key("component", "admin:password1234"),
        REDACTED
    );
    assert_eq!(
        sanitize_attr_for_key("model", "sk-proj-abc123def456"),
        REDACTED
    );
}

#[test]
fn test_attr_model_and_cardinality_keep_model_names() {
    let cache = CardinalityCache::new(100);
    for id in NAME_TAG_IDS {
        let kv = InstrumentRegistry::attr_model(id);
        assert_eq!(attr_value(&kv), *id);
        let resolved = cache.resolve("codemason.llm.requests", &[kv]);
        assert_eq!(
            attr_value(&resolved[0]),
            *id,
            "cardinality cache redacted {id}"
        );
    }
    let kv = InstrumentRegistry::attr_model("sk-proj-abc123def456");
    assert_eq!(attr_value(&kv), REDACTED);
}

#[test]
fn test_cardinality_still_redacts_credentials_under_other_keys() {
    let cache = CardinalityCache::new(100);
    let resolved = cache.resolve(
        "codemason.errors.total",
        &[KeyValue::new("component", "admin:password1234")],
    );
    assert_eq!(attr_value(&resolved[0]), REDACTED);
}

#[test]
fn test_ollama_model_id_shape() {
    for id in [
        "llama3:latest",
        "gpt-oss:120b",
        "qwen2.5:1.5b",
        "qwen2.5-coder:32b-instruct-q4_K_M",
        "mistral:instruct",
        "library/llama3:8b",
    ] {
        assert!(is_ollama_model_id(id), "{id}");
    }
    for value in
        CREDENTIALS
            .iter()
            .copied()
            .chain(["llama3", "a:b:c", ":latest", "llama3:", "name:8bit"])
    {
        assert!(!is_ollama_model_id(value), "{value}");
    }
}

#[test]
fn test_credentials_under_model_keys_are_still_redacted() {
    let cache = CardinalityCache::new(100);
    for credential in CREDENTIALS {
        assert_eq!(sanitize_attr_for_key("model", credential), REDACTED);
        assert_eq!(
            sanitize_attr_for_key("selected_model", credential),
            REDACTED
        );
        let kv = InstrumentRegistry::attr_model(credential);
        assert_eq!(attr_value(&kv), REDACTED, "attr_model kept {credential}");
        let resolved = cache.resolve(
            "codemason.llm.requests",
            &[KeyValue::new("model", (*credential).to_string())],
        );
        assert_eq!(
            attr_value(&resolved[0]),
            REDACTED,
            "cache kept {credential}"
        );
    }
}

/// Converts every event it sees and forwards the result.
struct CaptureLayer {
    tx: std::sync::mpsc::Sender<LogEvent>,
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let _ = self.tx.send(convert_event(event)); // INTENTIONAL: receiver outlives the emit
    }
}

/// The exported value of the `model` field of one event logged with `model`.
fn logged_model_value(model: &str) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer { tx });
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(model = model, "llm request");
    });
    rx.recv()
        .expect("one event")
        .attributes
        .into_iter()
        .find(|(name, _)| name == "model")
        .map(|(_, value)| value)
}

#[test]
fn test_log_events_keep_model_ids_and_redact_credentials() {
    for id in NAME_TAG_IDS {
        assert_eq!(logged_model_value(id).as_deref(), Some(*id));
    }
    for credential in CREDENTIALS {
        assert_eq!(logged_model_value(credential).as_deref(), Some(REDACTED));
    }
}
