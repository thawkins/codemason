//! Locks CO-01 (deep-refactor audit 2026-11-27): `Config::merge` used to drop
//! the `stream`, `memory`, `storage`, `agent_perf`, `trigger` and
//! `security_analyzer` sections, so every load (which starts from defaults and
//! merges each file in) discarded them - even from the user-global file.
//!
//! A trusted layer now carries each section; a project overlay may set the
//! benign ones (`stream`, `agent_perf`, `trigger`) but not `storage`
//! (retention deletes messages across every session), `memory` (cross-project
//! sharing crosses a data boundary) or `security_analyzer` (a security
//! control).

use codemason_config::Config;

/// A config file that sets every one of the six sections away from its default.
fn every_section() -> Config {
    serde_json::from_str(
        r#"{
            "stream": { "initial_response_timeout_secs": 77, "timeout_secs": 55 },
            "memory": { "enabled": false },
            "storage": { "retention": { "enabled": true, "message_days": 9 } },
            "agent_perf": { "profiling": true, "step_budget_secs": 33 },
            "trigger": { "enabled": false, "poll_interval_secs": 7, "max_rules": 3 },
            "security_analyzer": { "enabled": true, "timeout_secs": 5 }
        }"#,
    )
    .expect("parse six-section config")
}

#[test]
fn test_merge_carries_all_six_sections_from_a_trusted_layer() {
    let file = every_section();
    let merged = Config::merge(Config::default(), every_section());

    assert_eq!(merged.stream, file.stream);
    assert!(
        !merged.memory.enabled,
        "memory.enabled must survive the merge"
    );
    assert_eq!(merged.storage, file.storage);
    assert_eq!(merged.agent_perf, file.agent_perf);
    assert!(!merged.trigger.enabled);
    assert_eq!(merged.trigger.poll_interval_secs, 7);
    assert_eq!(merged.trigger.max_rules, 3);
    assert_eq!(merged.security_analyzer, file.security_analyzer);
}

#[test]
fn test_merge_keeps_the_lower_layer_when_the_overlay_omits_the_sections() {
    let file = every_section();
    let merged = Config::merge(every_section(), Config::default());

    assert_eq!(merged.stream, file.stream);
    assert!(!merged.memory.enabled);
    assert_eq!(merged.storage, file.storage);
    assert_eq!(merged.agent_perf, file.agent_perf);
    assert_eq!(merged.trigger.poll_interval_secs, 7);
    assert_eq!(merged.security_analyzer, file.security_analyzer);
}

#[test]
fn test_project_overlay_sets_the_benign_sections() {
    let file = every_section();
    let merged = Config::merge_project(Config::default(), every_section());

    assert_eq!(merged.stream, file.stream);
    assert_eq!(merged.agent_perf, file.agent_perf);
    assert_eq!(merged.trigger.poll_interval_secs, 7);
}

#[test]
fn test_project_overlay_cannot_set_storage_memory_or_security_analyzer() {
    let user = every_section();
    let project: Config = serde_json::from_str(
        r#"{
            "memory": { "enabled": true, "cross_project": { "enabled": true } },
            "storage": { "retention": { "enabled": true, "message_days": 1 } },
            "security_analyzer": { "enabled": false }
        }"#,
    )
    .expect("parse project overlay");

    let merged = Config::merge_project(every_section(), project);

    assert!(!merged.memory.enabled, "a project may not re-enable memory");
    assert_eq!(
        merged.storage, user.storage,
        "a project may not set retention"
    );
    assert_eq!(
        merged.security_analyzer, user.security_analyzer,
        "a project may not disable or redirect the security analyzer"
    );
}
