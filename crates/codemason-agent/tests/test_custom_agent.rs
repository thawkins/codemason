//! External tests for `tests` from `crates/codemason-agent/src/agent/custom.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use codemason_agent::agent::custom::*;
use codemason_agent::agent::oasf::{CODEMASON_MODULE_TYPE, OasfAgentRecord, OasfModule};
use codemason_types::{ThinkingConfig, ThinkingLevel};
use serde_json::json;
use std::path::Path;

#[test]
fn test_record_to_agent_info_parses_thinking_defaults() {
    let record = OasfAgentRecord {
        name: "reasoner".to_string(),
        description: "Reasoning custom agent".to_string(),
        version: "1.0.0".to_string(),
        schema_version: "0.7.0".to_string(),
        authors: Vec::new(),
        created_at: None,
        skills: Vec::new(),
        domains: Vec::new(),
        locators: Vec::new(),
        modules: vec![OasfModule {
            module_type: CODEMASON_MODULE_TYPE.to_string(),
            payload: json!({
                "system_prompt": "You reason carefully.",
                "thinking": {
                    "enabled": true,
                    "level": "high"
                }
            }),
        }],
    };

    let agent = record_to_agent_info(&record, Path::new("/tmp/reasoner.json")).expect("agent");
    assert_eq!(
        agent.thinking,
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );
}
