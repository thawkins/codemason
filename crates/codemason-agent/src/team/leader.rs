//! Launching a `team-leader` sub-agent for a team an entry point creates.
//!
//! `/team create`, `/swarm` and other non-tool entry points never make the
//! primary session a team lead. They call [`launch_leader_for_team`], which
//! creates a `team-leader` child session of the primary, creates the team
//! with that session as lead, binds the team's [`TeamManager`] in the
//! processor's registry and starts the leader as an ordinary background
//! (non-detached) task, so its summary reaches the primary like any
//! background `new_agent` result and `list_agents` / `wait_agents` see it.
//!
//! Depends on [`crate::task::AgentManager`] (the background spawn path),
//! [`super::store::TeamStore`] and [`super::manager::TeamManager`].

use std::path::Path;

use crate::agent::ModelRef;
use crate::task::AgentManager;

use super::manager::TeamManager;
use super::store::TeamStore;

/// Name of the built-in agent a leader launch runs.
const TEAM_LEADER_AGENT: &str = "team-leader";

/// The outcome of [`launch_leader_for_team`].
#[derive(Debug, Clone)]
pub struct LeaderLaunch {
    /// The leader's child session id; also the team's `lead_session_id`.
    pub leader_session_id: String,
    /// The background task id the parent session waits on.
    pub task_id: String,
    /// The created team's name.
    pub team_name: String,
}

/// Create `team_name` with a new `team-leader` sub-agent as its lead and
/// start that leader in the background.
///
/// The leader's child session (`SessionKind::Subagent`, parent
/// `parent_session_id`) is created first; the team is then created in
/// `working_dir` (project-local) with that session as lead and its manager is
/// bound, all before the leader's first step, so a seed or swarm that follows
/// can spawn teammates at once. The leader then runs `task`.
///
/// # Arguments
///
/// * `agent_manager` - the background task registry the leader runs in.
/// * `parent_session_id` - the session that receives the leader's result.
/// * `working_dir` - the project directory the team and leader live in.
/// * `team_name` - the team to create; a taken name is refused.
/// * `task` - the leader's task prompt.
/// * `model` - optional model for the leader and its teammates.
///
/// # Errors
///
/// The background concurrency-limit refusal, a session-creation failure, or
/// the team-creation failure (for example a taken name). On a team-creation
/// failure no task is registered and the leader session is deleted.
pub async fn launch_leader_for_team(
    agent_manager: &AgentManager,
    parent_session_id: &str,
    working_dir: &Path,
    team_name: &str,
    task: &str,
    model: Option<ModelRef>,
) -> anyhow::Result<LeaderLaunch> {
    let model_override = model
        .as_ref()
        .map(|m| format!("{}/{}", m.provider_id, m.model_id));
    let processor = agent_manager.processor();
    let entry = agent_manager
        .spawn_background_with_setup(
            parent_session_id,
            TEAM_LEADER_AGENT,
            task,
            model_override.as_deref(),
            working_dir,
            None,
            |leader_session_id| {
                let store = TeamStore::create(team_name, leader_session_id, working_dir, true)?;
                let mut manager = TeamManager::new(
                    store.config.name.clone(),
                    leader_session_id,
                    store.dir.clone(),
                    std::sync::Arc::clone(processor),
                    processor.event_bus.clone(),
                );
                manager.active_model = model.clone();
                let manager = manager.into_arc();
                // The team was just created exclusively, so no live manager
                // can be bound for it; a refusal would mean another runtime
                // already drives it, and that one is kept.
                if processor
                    .team_managers
                    .bind(std::sync::Arc::clone(&manager))
                    .is_ok()
                {
                    manager.start_watchdog();
                }
                Ok(())
            },
        )
        .await?;
    Ok(LeaderLaunch {
        leader_session_id: entry.child_session_id.clone(),
        task_id: entry.id.clone(),
        team_name: team_name.to_string(),
    })
}

/// The leader task for a team an entry point has already created and seeded.
///
/// The leader must not call `team_create`; it goes straight to `team_wait`,
/// follows up on stuck or incomplete work, and returns a summary. It leaves
/// `team_cleanup` to the entry point that created the team (the user for
/// `/team create`, the swarm's finalize step for `/swarm`).
#[must_use]
pub fn leader_task_for_existing_team(team_name: &str, context: &str) -> String {
    format!(
        "You lead team '{team_name}'. It already exists and is being seeded with its \
         teammates, so do NOT call `team_create`.\n\n\
         1. Call `team_wait` to block until every teammate is idle. If it reports no \
         teammates yet, check `team_status` and call `team_wait` again: seeding may \
         still be running.\n\
         2. Read `team_status` and the teammates' outputs.\n\
         3. Re-assign work or shut down teammates that are stuck or incomplete, then \
         wait again.\n\n\
         Do not call `team_cleanup`: whoever created the team cleans it up. Your final \
         message is a concise summary: the results, the files written, and \
         any unresolved items.\n\nContext: {context}"
    )
}
