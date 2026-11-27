//! Tests for `AgentRole` and the built-in `team-leader` agent.

use std::path::PathBuf;

use codemason_agent::Config;
use codemason_agent::agent::custom::load_custom_agents;
use codemason_agent::agent::{AgentMode, AgentRole, resolve_agent};
use codemason_agent::permission::{PermissionAction, PermissionChecker};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn test_agent_team_leader_builtin_exists() {
    let agent = resolve_agent("team-leader", &Config::default()).expect("resolve");
    assert_eq!(agent.role, AgentRole::TeamLeader);
    assert_eq!(agent.mode, AgentMode::Subagent);
}

#[test]
fn test_agent_team_leader_denies_new_agent_and_edit() {
    let agent = resolve_agent("team-leader", &Config::default()).expect("resolve");
    let checker = PermissionChecker::new(agent.permission.clone());
    assert_eq!(
        checker.check("agent:spawn", "tool:new_agent"),
        PermissionAction::Deny
    );
    assert_eq!(checker.check("edit", "src/lib.rs"), PermissionAction::Deny);
    assert_eq!(checker.check("bash", "ls"), PermissionAction::Deny);
    assert_eq!(checker.check("read", "src/lib.rs"), PermissionAction::Allow);
    for category in ["team:manage", "team:communicate", "team:tasks", "team:read"] {
        assert_eq!(
            checker.check(category, "tool:team_x"),
            PermissionAction::Allow,
            "{category}"
        );
    }
    // team_wait shares the `agent:spawn` category with new_agent.
    assert_eq!(
        checker.check("agent:spawn", "tool:team_wait"),
        PermissionAction::Allow
    );
}

#[test]
fn test_agent_team_leader_custom_md_role() {
    let _guard = crate::global_env_lock();
    let root = scratch_dir("team_leader_custom_md");
    let agents = root.join(".codemason").join("agents");
    std::fs::create_dir_all(&agents).expect("create agents dir");
    std::fs::write(
        agents.join("review-leader.md"),
        "---\n{\"name\":\"review-leader\",\"description\":\"Leads reviews\",\
         \"role\":\"team-leader\"}\n---\nLead the review team.\n",
    )
    .expect("write agent");
    let (defs, diagnostics) = load_custom_agents(&root);
    let def = defs
        .iter()
        .find(|d| d.agent_info.name == "review-leader")
        .unwrap_or_else(|| panic!("agent not loaded: {diagnostics:?}"));
    assert_eq!(def.agent_info.role, AgentRole::TeamLeader);
    let checker = PermissionChecker::new(def.agent_info.permission.clone());
    assert_eq!(
        checker.check("agent:spawn", "tool:new_agent"),
        PermissionAction::Deny
    );
}

#[test]
fn test_agent_team_leader_config_prompt_override() {
    let config: Config =
        serde_json::from_str(r#"{"agent":{"team-leader":{"prompt":"X"}}}"#).expect("config");
    let agent = resolve_agent("team-leader", &config).expect("resolve");
    assert_eq!(agent.prompt.as_deref(), Some("X"));
    assert_eq!(agent.role, AgentRole::TeamLeader);
}

#[test]
fn test_agent_team_leader_default_role_standard() {
    let agent = resolve_agent("explore", &Config::default()).expect("resolve");
    assert_eq!(agent.role, AgentRole::Standard);
}
