//! Launching a `team-leader` sub-agent for a team an entry point creates.
//!
//! `/team create`, `/swarm` and other non-tool entry points never make the
//! primary session a team lead. They call [`launch_leader_for_team_with_seed`]
//! (or [`launch_leader_for_team`]), which creates a `team-leader` child
//! session of the primary, creates the team with that session as lead, binds
//! the team's [`TeamManager`] in the processor's registry, runs the entry
//! point's seeding, and only then starts the leader as an ordinary background
//! (non-detached) task, so its summary reaches the primary like any
//! background `new_agent` result and `list_agents` / `wait_agents` see it.
//!
//! Depends on [`crate::task::AgentManager`] (the background spawn path),
//! [`super::store::TeamStore`], [`super::manager::TeamManager`] and
//! [`super::swarm::SYNTHESIS_SYSTEM_PROMPT`].

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// What an entry point's seeding step receives: the team exists, led by the
/// leader session, and its manager is bound; the leader has not started.
#[derive(Clone)]
pub struct LeaderSeed {
    /// The leader's child session id (the team's lead).
    pub leader_session_id: String,
    /// The created team's directory.
    pub team_dir: PathBuf,
    /// The team's bound runtime, for spawning teammates.
    pub manager: Arc<TeamManager>,
}

impl std::fmt::Debug for LeaderSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LeaderSeed")
            .field("leader_session_id", &self.leader_session_id)
            .field("team_dir", &self.team_dir)
            .field("team_name", &self.manager.team_name)
            .finish()
    }
}

/// Create `team_name` with a new `team-leader` sub-agent as its lead and
/// start that leader in the background, with no seeding step.
///
/// See [`launch_leader_for_team_with_seed`].
///
/// # Errors
///
/// As [`launch_leader_for_team_with_seed`].
pub async fn launch_leader_for_team(
    agent_manager: &AgentManager,
    parent_session_id: &str,
    working_dir: &Path,
    team_name: &str,
    task: &str,
    model: Option<ModelRef>,
) -> anyhow::Result<LeaderLaunch> {
    launch_leader_for_team_with_seed(
        agent_manager,
        parent_session_id,
        working_dir,
        team_name,
        task,
        model,
        |_| std::future::ready(Ok(())),
    )
    .await
}

/// Create `team_name` with a new `team-leader` sub-agent as its lead, run
/// `seed`, then start that leader in the background.
///
/// The leader's child session (`SessionKind::Subagent`, parent
/// `parent_session_id`) is created first; the team is then created in
/// `working_dir` (project-local) with that session as lead, its manager is
/// bound, and `seed` runs (blueprint seeding, swarm members and tasks), all
/// before the leader's first step, so the leader's first `team_wait` already
/// sees its teammates. The leader then runs `task`.
///
/// # Arguments
///
/// * `agent_manager` - the background task registry the leader runs in.
/// * `parent_session_id` - the session that receives the leader's result.
/// * `working_dir` - the project directory the team and leader live in.
/// * `team_name` - the team to create; a taken name is refused.
/// * `task` - the leader's task prompt.
/// * `model` - optional model for the leader and its teammates.
/// * `seed` - the entry point's seeding step.
///
/// # Errors
///
/// The background concurrency-limit refusal, a session-creation failure, the
/// team-creation failure (for example a taken name), or the seeding error.
/// On a team-creation or seeding failure no task is registered and the leader
/// session is deleted; a team whose seeding failed is shut down, unbound and
/// removed.
pub async fn launch_leader_for_team_with_seed<S, Fut>(
    agent_manager: &AgentManager,
    parent_session_id: &str,
    working_dir: &Path,
    team_name: &str,
    task: &str,
    model: Option<ModelRef>,
    seed: S,
) -> anyhow::Result<LeaderLaunch>
where
    S: FnOnce(LeaderSeed) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
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
            |leader_session_id| async move {
                let store = TeamStore::create(team_name, &leader_session_id, working_dir, true)?;
                let mut manager = TeamManager::new(
                    store.config.name.clone(),
                    leader_session_id.clone(),
                    store.dir.clone(),
                    Arc::clone(processor),
                    processor.event_bus.clone(),
                );
                manager.active_model = model;
                let manager = manager.into_arc();
                // The team was just created exclusively, so no live manager
                // can be bound for it yet.
                processor
                    .team_managers
                    .bind(Arc::clone(&manager))
                    .map_err(|_| {
                        anyhow::anyhow!("team '{team_name}' already has a live runtime bound")
                    })?;
                manager.clone().start_watchdog();
                let seeded = seed(LeaderSeed {
                    leader_session_id,
                    team_dir: store.dir.clone(),
                    manager: Arc::clone(&manager),
                })
                .await;
                if let Err(e) = seeded {
                    discard_unseeded_team(processor, &manager, &store.dir).await;
                    return Err(e.context(format!("seeding team '{team_name}' failed")));
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

/// Tear down a team whose seeding failed: stop its teammates, unbind its
/// manager and remove its directory. Failures are logged; the seeding error
/// is what the caller reports.
async fn discard_unseeded_team(
    processor: &crate::session::processor::SessionProcessor,
    manager: &Arc<TeamManager>,
    team_dir: &Path,
) {
    if let Err(e) = manager.shutdown_all().await {
        tracing::warn!(team = %manager.team_name, error = %e,
            "failed to stop the teammates of an unseeded team");
    }
    processor.team_managers.remove_if(manager);
    if let Err(e) = std::fs::remove_dir_all(team_dir) {
        tracing::warn!(team = %manager.team_name, error = %e,
            "failed to remove the directory of an unseeded team");
    }
}

/// Cancel the running leader task that leads a team, found by the team's
/// `lead_session_id`.
///
/// # Returns
///
/// `true` when a non-terminal task owning `lead_session_id` was found and
/// cancelled.
pub async fn cancel_leader_for_session(
    agent_manager: &AgentManager,
    lead_session_id: &str,
) -> bool {
    let task_id = agent_manager
        .tasks_snapshot()
        .await
        .into_iter()
        .find(|t| t.child_session_id == lead_session_id && !t.status.is_terminal())
        .map(|t| t.id);
    let Some(task_id) = task_id else {
        return false;
    };
    match agent_manager.cancel_agent(&task_id).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(task = %task_id, error = %e, "failed to cancel a team leader");
            false
        }
    }
}

/// The leader task for a team an entry point has already created and seeded.
///
/// The leader must not call `team_create`; it goes straight to `team_wait`,
/// follows up on stuck or incomplete work, calls `team_cleanup` and returns
/// a summary.
#[must_use]
pub fn leader_task_for_existing_team(team_name: &str, context: &str) -> String {
    format!(
        "You lead team '{team_name}'. It already exists and its teammates are seeded, \
         so do NOT call `team_create`.\n\n\
         1. Call `team_wait` to block until every teammate is idle.\n\
         2. Read `team_status` and the teammates' outputs.\n\
         3. Re-assign work or shut down teammates that are stuck or incomplete, then \
         wait again.\n\
         4. When you have what your summary needs, call `team_cleanup`.\n\n\
         Your final message is a concise summary: the results, the files written, and \
         any unresolved items.\n\nContext: {context}"
    )
}

/// The leader task for a `/swarm` team: a passive supervisor that waits for
/// every subtask, synthesises one answer to the goal, and cleans up.
///
/// The swarm's frontend keeps unblocking dependent members, so the leader
/// must not re-assign work or shut teammates down; it carries the synthesis
/// guidance of [`super::swarm::SYNTHESIS_SYSTEM_PROMPT`].
#[must_use]
pub fn leader_task_for_swarm(team_name: &str, goal: &str) -> String {
    format!(
        "You lead swarm team '{team_name}'. It already exists: each teammate owns one \
         pre-assigned subtask, so do NOT call `team_create`, do NOT re-assign tasks and \
         do NOT call `team_shutdown_teammate`. A `Blocked` or parked teammate is waiting \
         on its dependencies; it is started automatically when they finish.\n\n\
         1. Call `team_wait`, then check `team_task_list` and `team_status`. Keep calling \
         `team_wait` until every swarm task is completed or cancelled.\n\
         2. Synthesise the final answer from the subtask results, following this \
         guidance:\n{synthesis}\n\
         3. Call `team_cleanup`.\n\n\
         Your final message is that answer.\n\nGoal: {goal}",
        synthesis = super::swarm::SYNTHESIS_SYSTEM_PROMPT
    )
}
