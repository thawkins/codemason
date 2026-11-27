//! `TeamManagers` keeps one live manager per team name, so several teams can
//! run on one processor without evicting each other.

use std::path::PathBuf;
use std::sync::Arc;

use codemason_agent::event::EventBus;
use codemason_agent::team::{TeamManager, TeamManagers, TeamStore};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::stub_processor;

/// A fresh workspace under `target/temp/test_team_registry/<test>`.
fn workspace(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp/test_team_registry")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".codemason")).expect("workspace");
    dir
}

/// A manager over a newly created team `name` in `workspace`.
fn manager(workspace: &std::path::Path, name: &str) -> Arc<TeamManager> {
    let bus = Arc::new(EventBus::new(16));
    let processor = stub_processor(bus.clone());
    let store = TeamStore::create(name, "lead-sess", workspace, true).expect("create team");
    TeamManager::new(name, "lead-sess", store.dir.clone(), processor, bus).into_arc()
}

#[tokio::test]
async fn test_team_registry_binds_two_teams_independently() {
    let ws = workspace("binds_two");
    let reg = TeamManagers::default();
    assert!(reg.bind(manager(&ws, "alpha")).is_ok());
    assert!(reg.bind(manager(&ws, "beta")).is_ok());
    assert!(reg.get("alpha").is_some());
    assert!(reg.get("beta").is_some());
    assert_eq!(reg.all().len(), 2);
}

#[tokio::test]
async fn test_team_registry_refuses_second_live_manager_same_team() {
    let ws = workspace("refuses_second");
    let reg = TeamManagers::default();
    let first = manager(&ws, "alpha");
    assert!(reg.bind(Arc::clone(&first)).is_ok());
    let dup = manager_over(&first, &ws);
    assert!(reg.bind(dup).is_err());
}

/// A second manager over the same team dir as `first`.
fn manager_over(first: &Arc<TeamManager>, _ws: &std::path::Path) -> Arc<TeamManager> {
    let bus = Arc::new(EventBus::new(16));
    let processor = stub_processor(bus.clone());
    TeamManager::new(
        first.team_name.clone(),
        "lead-sess",
        first.team_dir.clone(),
        processor,
        bus,
    )
    .into_arc()
}

#[tokio::test]
async fn test_team_registry_replaces_stale_manager() {
    let ws = workspace("replaces_stale");
    let reg = TeamManagers::default();
    let first = manager(&ws, "alpha");
    assert!(reg.bind(Arc::clone(&first)).is_ok());
    std::fs::remove_file(first.team_dir.join("config.json")).expect("remove config");
    let replacement = manager_over(&first, &ws);
    assert!(reg.bind(Arc::clone(&replacement)).is_ok());
    assert!(Arc::ptr_eq(&reg.get("alpha").expect("bound"), &replacement));
}

#[tokio::test]
async fn test_team_registry_prune_keeps_live_teams() {
    let ws = workspace("prune_keeps_live");
    let reg = TeamManagers::default();
    let alpha = manager(&ws, "alpha");
    let beta = manager(&ws, "beta");
    assert!(reg.bind(Arc::clone(&alpha)).is_ok());
    assert!(reg.bind(Arc::clone(&beta)).is_ok());
    std::fs::remove_file(alpha.team_dir.join("config.json")).expect("remove config");
    let pruned = reg.prune_stale();
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0].team_name, "alpha");
    assert!(reg.get("alpha").is_none());
    let kept = reg.get("beta").expect("beta still bound");
    assert!(Arc::ptr_eq(&kept, &beta));
}

#[tokio::test]
async fn test_team_registry_remove_if_ignores_other_manager() {
    let ws = workspace("remove_if_other");
    let reg = TeamManagers::default();
    let bound = manager(&ws, "alpha");
    assert!(reg.bind(Arc::clone(&bound)).is_ok());
    let other = manager_over(&bound, &ws);
    assert!(!reg.remove_if(&other));
    assert!(reg.get("alpha").is_some());
    assert!(reg.remove_if(&bound));
    assert!(reg.get("alpha").is_none());
}
