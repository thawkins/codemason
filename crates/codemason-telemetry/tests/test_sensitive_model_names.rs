//! Model-name attributes keep Ollama `name:tag` ids while secrets stay
//! redacted (FR-034; deep-refactor D-11, owner decision O-06 on #11).

#![cfg(feature = "telemetry")]

use codemason_telemetry::InstrumentRegistry;
use codemason_telemetry::cardinality::CardinalityCache;
use codemason_telemetry::sensitive::{
    REDACTED, is_model_attr_key, looks_sensitive, looks_sensitive_model_name, sanitize_attr_for_key,
};
use opentelemetry::KeyValue;

/// Ollama-style ids whose two colon parts are both dense and 4+ chars long,
/// which the generic `key:secret` rule treats as credentials.
const NAME_TAG_IDS: &[&str] = &["llama3:latest", "gpt-oss:120b", "mistral:instruct"];

const SECRETS: &[&str] = &[
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
