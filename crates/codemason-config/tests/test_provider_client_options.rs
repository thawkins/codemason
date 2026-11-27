//! `Config::provider_client_options` forwards a provider's configured options
//! to its client (audit CO-14).

use codemason_config::Config;
use serde_json::{Value, json};

#[test]
fn test_provider_client_options_forwards_every_option_and_the_model() {
    let cfg: Config = serde_json::from_value(json!({
        "provider": {
            "bedrock": {
                "options": {
                    "region": "eu-west-1",
                    "profile": "work",
                    "endpoint_url": "https://vpce.example",
                    "model_id": "stale-config-value"
                }
            }
        }
    }))
    .expect("config parses");

    let options = cfg.provider_client_options("bedrock", "anthropic.claude-x");

    assert_eq!(options.get("region"), Some(&json!("eu-west-1")));
    assert_eq!(options.get("profile"), Some(&json!("work")));
    assert_eq!(
        options.get("endpoint_url"),
        Some(&json!("https://vpce.example"))
    );
    assert_eq!(
        options.get("model_id"),
        Some(&Value::String("anthropic.claude-x".to_string())),
        "the requested model wins over a configured one"
    );
}

#[test]
fn test_provider_client_options_without_provider_config_passes_the_model() {
    let options = Config::default().provider_client_options("bedrock", "m");
    assert_eq!(options.len(), 1);
    assert_eq!(options.get("model_id"), Some(&json!("m")));
}
