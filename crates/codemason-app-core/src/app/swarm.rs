//! Swarm decomposition handling for the TUI and GUI.

use codemason_agent::team::{self, MemberStatus};

// State types from app/state.rs
use crate::app::state::{App, LogLevel};

// Helpers

// Re-export status types from theme

impl App {
    /// The working directory the swarm was launched from. All team/task
    /// stores are anchored to the App's cwd, so use that rather than
    /// re-querying the process cwd (which could differ if it ever changes).
    fn swarm_working_dir(&self) -> std::path::PathBuf {
        self.cwd_path.clone()
    }

    /// Load the swarm team's task list from disk, or `None` when the team or
    /// its task store cannot be read. Shared by the status, unblock,
    /// completion, and finalize paths so the load-by-name + open + read chain
    /// lives in one place.
    fn load_swarm_tasks(&self, team_name: &str) -> Option<codemason_agent::team::task::TaskList> {
        let working_dir = self.swarm_working_dir();
        let store = team::TeamStore::load_by_name(team_name, &working_dir).ok()?;
        let ts = team::TaskStore::open(&store.dir).ok()?;
        ts.read().ok()
    }

    pub(crate) async fn execute_swarm_decomposition(
        &mut self,
        decomposition: team::SwarmDecomposition,
    ) {
        use codemason_agent::team::SwarmState;

        let task_count = decomposition.tasks.len();
        if task_count == 0 {
            self.status = "[warn] swarm: LLM returned 0 subtasks".to_string();
            self.append_assistant_text(
                "From: /swarm\n## [warn] No subtasks\n\nThe LLM returned an empty task list.\n",
            );
            return;
        }

        // Create ephemeral team name
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let team_name = format!("swarm-{ts}");
        let working_dir = self.swarm_working_dir();
        let parent_sid = self.session.session_id.clone().unwrap_or_default();

        // The primary is never a team lead: a `team-leader` sub-agent of this
        // session creates the team, leads it and reports back here.
        let Some(agent_manager) = self.session_processor.agent_manager.get().cloned() else {
            self.status =
                "[warn] swarm: team creation failed: no agent manager to run its team leader"
                    .to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "Swarm team creation failed: no agent manager to run its team leader".to_string(),
            );
            return;
        };
        let leader_model: Option<codemason_agent::agent::ModelRef> =
            self.session.selected_model.as_deref().and_then(|s| {
                s.split_once('/')
                    .map(|(pid, mid)| codemason_agent::agent::ModelRef {
                        provider_id: pid.to_string(),
                        model_id: mid.to_string(),
                    })
            });
        let goal = self
            .session
            .swarm_state
            .as_ref()
            .map(|s| s.prompt.clone())
            .unwrap_or_default();
        let leader_task = team::leader::leader_task_for_swarm(&team_name, &goal);

        // Create the team, led by the leader session, and seed its tasks and
        // teammates before the leader's first step.
        let mut seeded: Option<SwarmSeeded> = None;
        let slot = &mut seeded;
        let launched = team::leader::launch_leader_for_team_with_seed(
            &agent_manager,
            &parent_sid,
            &working_dir,
            &team_name,
            &leader_task,
            leader_model,
            |leader| {
                let decomposition = &decomposition;
                let team_name = &team_name;
                async move {
                    *slot = Some(seed_swarm_team(&leader, team_name, decomposition).await?);
                    Ok(())
                }
            },
        )
        .await;
        let seeded = match (launched, seeded) {
            (Ok(_), Some(seeded)) => seeded,
            (Ok(launch), None) => {
                // Unreachable in practice: a successful launch ran the seed.
                team::leader::cancel_leader_for_session(&agent_manager, &launch.leader_session_id)
                    .await;
                self.status =
                    "[warn] swarm: team creation failed: the team was not seeded".to_string();
                return;
            }
            (Err(e), _) => {
                self.status = format!("[warn] swarm: team creation failed: {e:#}");
                self.push_log_no_agent(
                    LogLevel::Warn,
                    format!("Swarm team creation failed: {e:#}"),
                );
                return;
            }
        };

        // Set up active team state
        self.session.active_team = Some(seeded.config);
        self.session.team_members = seeded.members;
        self.session.team_task_counts.clear();
        self.session.team_task_counts_dirty = true;
        self.show_teams = true;

        // Build display table
        let mut output = format!(
            "From: /swarm\n## [bee] Swarm Created: {team_name}\n\n\
                              **{task_count} subtasks** decomposed and seeded.\n\n\
                              | ID | Title | Agent Type | Dependencies |\n\
                              |----|-------|------------|--------------|\n"
        );
        for st in &decomposition.tasks {
            let deps = if st.depends_on.is_empty() {
                "-".to_string()
            } else {
                st.depends_on.join(", ")
            };
            let agent = st
                .agent_type
                .as_deref()
                .unwrap_or(codemason_agent::team::DEFAULT_AGENT_TYPE);
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                st.id, st.title, agent, deps
            ));
        }
        output.push_str(
            "\nTeammates spawned. A team leader waits for every subtask and reports the \
             combined answer.\n",
        );
        self.append_assistant_text(&output);
        for line in seeded.log {
            self.push_log_no_agent(LogLevel::Info, line);
        }

        // Keep the goal and default agent type `/swarm` recorded before
        // decomposing.
        let (swarm_prompt, default_agent_type) = self
            .session
            .swarm_state
            .as_ref()
            .map(|s| (s.prompt.clone(), s.default_agent_type.clone()))
            .unwrap_or_default();
        self.session.swarm_state = Some(SwarmState {
            team_name: team_name.clone(),
            prompt: swarm_prompt,
            decomposition: decomposition.clone(),
            spawned: true,
            completed: false,
            default_agent_type,
        });

        let ready = decomposition
            .tasks
            .iter()
            .filter(|t| t.depends_on.is_empty())
            .count();
        let blocked = decomposition.tasks.len() - ready;
        self.status = format!("[bee] swarm: {ready} spawning, {blocked} blocked");
    }

    pub(crate) fn handle_swarm_status(&mut self) {
        let Some(ref swarm) = self.session.swarm_state else {
            self.append_assistant_text(
                "From: /swarm status\n\nNo active swarm. Use `/swarm <prompt>` to start one.\n",
            );
            return;
        };

        let mut output = format!(
            "From: /swarm status\n## [bee] Swarm: {}\n\n",
            swarm.team_name
        );

        // Load tasks from disk for current status
        let tasks = self.load_swarm_tasks(&swarm.team_name);

        let total = swarm.decomposition.tasks.len();
        let (completed, in_progress, pending) = if let Some(ref tl) = tasks {
            let c = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::Completed)
                .count();
            let ip = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::InProgress)
                .count();
            let p = tl
                .tasks
                .iter()
                .filter(|t| t.status == team::TaskStatus::Pending)
                .count();
            (c, ip, p)
        } else {
            (0, 0, total)
        };

        // Progress bar: completed/total fraction of the bar. Guard the
        // zero-total case so the bar renders empty instead of dividing by 0.
        let bar_width = 30;
        let filled = (completed * bar_width).checked_div(total).unwrap_or(0);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        output.push_str(&format!(
            "**Progress:** [{bar}] {completed}/{total} ({} in progress, {} pending)\n\n",
            in_progress, pending
        ));

        // Task table
        output.push_str("| ID | Title | Status | Assigned | Dependencies |\n");
        output.push_str("|----|-------|--------|----------|-------------|\n");

        if let Some(ref tl) = tasks {
            for task in &tl.tasks {
                let status_icon = match task.status {
                    team::TaskStatus::Completed => "[ok]",
                    team::TaskStatus::InProgress => "[sync]",
                    team::TaskStatus::Pending => "[wait]",
                    team::TaskStatus::Cancelled => "[err]",
                };
                let assigned = task.assigned_to.as_deref().unwrap_or("-");
                let deps = if task.depends_on.is_empty() {
                    "-".to_string()
                } else {
                    task.depends_on.join(", ")
                };
                // A title and the dependency list are free text (a joined list
                // may carry a `|`), so both go through `mdtable::cell`.
                output.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    crate::app::mdtable::cell(&task.id),
                    crate::app::mdtable::cell(&task.title),
                    status_icon,
                    crate::app::mdtable::cell(assigned),
                    crate::app::mdtable::cell(&deps)
                ));
            }
        } else {
            for st in &swarm.decomposition.tasks {
                let deps = if st.depends_on.is_empty() {
                    "-".to_string()
                } else {
                    st.depends_on.join(", ")
                };
                output.push_str(&format!(
                    "| {} | {} | [wait] | - | {} |\n",
                    crate::app::mdtable::cell(&st.id),
                    crate::app::mdtable::cell(&st.title),
                    crate::app::mdtable::cell(&deps)
                ));
            }
        }

        // Teammate status
        output.push_str("\n**Teammates:**\n");
        if self.session.team_members.is_empty() {
            output.push_str("  (spawning...)\n");
        } else {
            for m in &self.session.team_members {
                let status = format!("{:?}", m.status).to_lowercase();
                output.push_str(&format!(
                    "  * {} - {} ({} agent)\n",
                    m.name, status, m.agent_type
                ));
            }
        }

        if completed == total && total > 0 {
            output.push_str("\n[done] **All tasks complete!** Use `/swarm cancel` to clean up.\n");
        }
        self.append_assistant_text(&output);
    }

    pub(crate) async fn handle_swarm_cancel(&mut self) {
        let Some(swarm) = self.session.swarm_state.take() else {
            self.append_assistant_text("From: /swarm cancel\n\nNo active swarm to cancel.\n");
            return;
        };

        // Reuse the existing team cleanup path
        let team_name = swarm.team_name.clone();

        // A finalising job must not run more hooks for a cancelled swarm.
        self.stop_swarm_autocomplete(&team_name).await;

        // Trigger team cleanup
        self.execute_slash_command(&format!("/team close {team_name}"))
            .await;

        self.append_assistant_text(&format!(
            "From: /swarm cancel\n## [bee] Swarm Cancelled\n\n\
            Swarm **{team_name}** has been shut down.\n"
        ));
        self.status = "swarm: cancelled".to_string();
        self.push_log_no_agent(LogLevel::Info, format!("Swarm cancelled: {team_name}"));
    }

    /// Periodically (every 2s) check whether any swarm teammate is waiting on
    /// a permission prompt and, if so, surface the unblock UI so the lead can
    /// approve/deny without leaving the swarm view.
    #[doc(hidden)]
    pub fn poll_swarm_unblock(&mut self) {
        if self.session.swarm_unblock_last_poll.elapsed() < std::time::Duration::from_secs(2) {
            return;
        }
        self.session.swarm_unblock_last_poll = std::time::Instant::now();
        let Some(ref swarm) = self.session.swarm_state else {
            return;
        };
        if swarm.completed {
            return;
        }

        // Clone what we need from swarm_state to avoid borrow issues
        let team_name = swarm.team_name.clone();
        let decomp_tasks = swarm.decomposition.tasks.clone();

        // Find blocked members
        let blocked_members: Vec<(String, String)> = self
            .session
            .team_members
            .iter()
            .filter(|m| m.status == MemberStatus::Blocked)
            .map(|m| (m.name.clone(), m.agent_id.clone()))
            .collect();

        if blocked_members.is_empty() {
            return;
        }

        // Each swarm task is pre-assigned to its own teammate, so a member's
        // subtask is the task assigned to its agent id. The list is kept so
        // an unblocked member's prompt can carry its predecessors' results.
        let working_dir = self.swarm_working_dir();
        let task_list = self.load_swarm_tasks(&team_name);
        let task_of = |agent_id: &str| swarm_task_of(task_list.as_ref(), agent_id);

        // Build set of completed task IDs from member status.
        let completed_task_ids: std::collections::HashSet<String> = self
            .session
            .team_members
            .iter()
            .filter(|m| matches!(m.status, MemberStatus::Idle | MemberStatus::Stopped))
            .filter_map(|m| task_of(&m.agent_id))
            .collect();

        // Also check TaskStore for explicitly completed tasks.
        let task_completed_ids: std::collections::HashSet<String> = task_list
            .as_ref()
            .map(|tl| tl.completed_ids())
            .unwrap_or_default();

        let all_completed: std::collections::HashSet<String> = completed_task_ids
            .union(&task_completed_ids)
            .cloned()
            .collect();

        // Check each blocked member's dependencies
        let mut unblocked = Vec::new();
        for (member_name, agent_id) in &blocked_members {
            let Some(task_id) = task_of(agent_id) else {
                tracing::debug!(teammate = %member_name, "swarm member has no assigned task");
                continue;
            };
            let task_id = task_id.as_str();
            // Find the task's depends_on from decomposition
            let deps = decomp_tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| &t.depends_on);

            if let Some(deps) = deps {
                let missing: Vec<_> = deps
                    .iter()
                    .filter(|d| !all_completed.contains(*d))
                    .cloned()
                    .collect();
                tracing::debug!(
                    task = %task_id,
                    deps = ?deps,
                    missing = ?missing,
                    completed_ids = ?all_completed,
                    "Checking swarm dependency resolution"
                );
                // `deps.is_empty()` implies `missing.is_empty()`, so a
                // member with no deps unblocks on the same condition (it
                // should have started Spawning instead of Blocked, but
                // unblock anyway rather than stranding it).
                if missing.is_empty() {
                    unblocked.push((member_name.clone(), agent_id.clone(), task_id.to_string()));
                }
            }
        }

        if unblocked.is_empty() {
            return;
        }

        // Transition unblocked members from Blocked -> Spawning
        for (member_name, agent_id, task_id) in &unblocked {
            let prompt = decomp_tasks
                .iter()
                .find(|t| t.id == *task_id)
                .map(|subtask| team::swarm_prompt_with_results(subtask, task_list.as_ref()));
            // Update local state
            if let Some(m) = self
                .session
                .team_members
                .iter_mut()
                .find(|m| m.agent_id == *agent_id)
            {
                m.status = MemberStatus::Spawning;
                if let Some(prompt) = &prompt {
                    m.spawn_prompt = Some(prompt.clone());
                }
            }
            // Update persisted config. The manager reads `spawn_prompt` when it
            // spawns the member, so the predecessor results land in its prompt.
            if let Some(team_dir) = team::find_team_dir(&working_dir, &team_name) {
                // A failed save leaves the persisted member stuck at Blocked
                // while the in-memory copy moved on - surface it so the swarm
                // is not left silently inconsistent.
                assign_swarm_task(&team_dir, task_id, agent_id);
                if let Err(e) = team::TeamStore::update(&team_dir, |config| {
                    if let Some(m) = config.member_by_id_mut(agent_id) {
                        m.status = MemberStatus::Spawning;
                        if let Some(prompt) = &prompt {
                            m.spawn_prompt = Some(prompt.clone());
                        }
                    }
                    Ok(())
                }) {
                    tracing::warn!(error = %e, agent_id, "failed to persist unblocked swarm member");
                }
            }
            // Log with actual deps for debugging
            let dep_info = decomp_tasks
                .iter()
                .find(|t| t.id == *task_id)
                .map(|t| t.depends_on.join(", "))
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Info,
                format!(
                    "[unlocked] Unblocking {} ({}) - deps [{}] all in {:?}",
                    member_name, task_id, dep_info, all_completed
                ),
            );
        } // Trigger reconcile to spawn newly-unblocked members
        if let Some(manager) = self.session_processor.team_managers.get(&team_name) {
            manager.clone().reconcile_spawning_members();
        }

        let remaining_blocked = blocked_members.len() - unblocked.len();
        if remaining_blocked > 0 {
            self.status = format!(
                "[bee] swarm: {} unblocked, {} still blocked",
                unblocked.len(),
                remaining_blocked
            );
        } else {
            self.status = format!("[bee] swarm: all teammates spawned");
        }
    }

    /// Periodically (every 2s) check the active swarm's overall completion
    /// status and update the status line / log panel accordingly.
    ///
    /// Once every teammate is terminal, the tasks still open in the task
    /// store are completed by a job spawned on the runtime
    /// ([`SwarmAutoComplete`]): each `TaskStore::complete` takes the store's
    /// blocking file lock and runs the team's `TaskCompleted` hook (up to
    /// 30 s each), which must not stall the frontend's thread. While the job
    /// runs, a pass starts no second job and reports nothing. The first pass
    /// after it ends (the next housekeeping pass, not the next 2 s check)
    /// reads the tally and finalises the swarm, as the same pass did when
    /// the writes ran inline. A job left over from a swarm that was replaced
    /// or cancelled is dropped.
    #[doc(hidden)]
    pub async fn poll_swarm_completion(&mut self) {
        let landed = match self.session.swarm_autocomplete.as_ref() {
            Some(job) if !job.handle.is_finished() => return,
            Some(job) => {
                let team_name = job.team_name.clone();
                self.session.swarm_autocomplete = None;
                self.session
                    .swarm_state
                    .as_ref()
                    .is_some_and(|swarm| swarm.team_name == team_name)
            }
            None => false,
        };
        if !landed
            && self.session.swarm_completion_last_poll.elapsed() < std::time::Duration::from_secs(2)
        {
            return;
        }
        self.session.swarm_completion_last_poll = std::time::Instant::now();
        let Some(ref swarm) = self.session.swarm_state else {
            return;
        };
        if swarm.completed || !swarm.spawned {
            return;
        }
        let team_name = swarm.team_name.clone();

        let working_dir = self.swarm_working_dir();

        // Check member status - if all non-lead members are terminal (idle/failed/stopped),
        // the swarm is effectively done regardless of task store state.
        let members: Vec<_> = self
            .session
            .team_members
            .iter()
            .filter(|m| m.name != "lead" && !m.name.is_empty())
            .collect();
        let has_members = !members.is_empty();
        let all_members_terminal = has_members
            && members.iter().all(|m| {
                matches!(
                    m.status,
                    MemberStatus::Idle | MemberStatus::Failed | MemberStatus::Stopped
                )
            });

        // If all members are terminal, auto-complete any non-completed tasks
        // in the task store, off this thread; the pass after the job ends
        // reads the tally below.
        if all_members_terminal && !landed {
            if let Ok(store) = team::TeamStore::load_by_name(&team_name, &working_dir) {
                if let Ok(ts) = team::TaskStore::open(&store.dir) {
                    match tokio::runtime::Handle::try_current() {
                        Ok(runtime) => {
                            self.session.swarm_autocomplete = Some(SwarmAutoComplete {
                                team_name,
                                handle: runtime.spawn(complete_open_swarm_tasks(ts)),
                            });
                            return;
                        }
                        // No runtime to hand the job to: complete inline.
                        Err(_) => complete_open_swarm_tasks(ts).await,
                    }
                }
            }
        }

        // Now check task store for final tally
        let tasks = self.load_swarm_tasks(&team_name);

        let Some(ref tl) = tasks else {
            // No task store - fall back to member-only check
            if all_members_terminal {
                self.finalize_swarm_completion(&team_name, 0, 0, 0);
            }
            return;
        };
        let total = tl.tasks.len();
        if total == 0 && all_members_terminal {
            self.finalize_swarm_completion(&team_name, 0, 0, 0);
            return;
        }
        if total == 0 {
            return;
        }

        let completed = tl
            .tasks
            .iter()
            .filter(|t| t.status == team::TaskStatus::Completed)
            .count();
        let cancelled = tl
            .tasks
            .iter()
            .filter(|t| t.status == team::TaskStatus::Cancelled)
            .count();
        let failed_members = members
            .iter()
            .filter(|m| m.status == MemberStatus::Failed)
            .count();

        if completed + cancelled >= total {
            self.finalize_swarm_completion(&team_name, total, completed, cancelled);
        } else if all_members_terminal {
            // Members done but tasks not all completed - report partial completion
            self.finalize_swarm_completion(
                &team_name,
                total,
                completed,
                cancelled + failed_members,
            );
        }
    }

    /// Stop every swarm auto-complete job working on `team_name`, in the
    /// focused and the background sessions, and wait until each has ended.
    ///
    /// Every path that cancels a swarm or closes, clears or removes its team
    /// calls this before it touches the team directory. An aborted job stops
    /// at its next await: a `TaskStore::complete` write in progress finishes
    /// first (its locked read-modify-write holds no await), and a running
    /// hook process is killed when its future is dropped (`kill_on_drop`).
    /// So once this returns nothing of the job writes into the directory
    /// any more, and no further hook starts. Tasks it did not reach stay
    /// open, as they were before the swarm finalised.

    ///
    /// # Arguments
    ///
    /// * `team_name` - the swarm team being cancelled or removed.
    pub async fn stop_swarm_autocomplete(&mut self, team_name: &str) {
        let mut jobs = Vec::new();
        if self
            .session
            .swarm_autocomplete
            .as_ref()
            .is_some_and(|job| job.team_name == team_name)
        {
            jobs.extend(self.session.swarm_autocomplete.take());
        }
        for index in 0..self.background.len() {
            if let Some(view) = self.background.view_mut(index)
                && view
                    .swarm_autocomplete
                    .as_ref()
                    .is_some_and(|job| job.team_name == team_name)
            {
                jobs.extend(view.swarm_autocomplete.take());
            }
        }
        for job in jobs {
            job.stop().await;
        }
    }

    /// Whether a swarm auto-complete job is running (test hook).
    ///
    /// # Returns
    ///
    /// `true` from the pass that spawned the job until it ends.
    #[doc(hidden)]
    #[must_use]
    pub fn swarm_autocomplete_in_flight(&self) -> bool {
        self.session
            .swarm_autocomplete
            .as_ref()
            .is_some_and(|job| !job.handle.is_finished())
    }

    /// Wait until the swarm auto-complete job, if any, has ended (test
    /// hook, in place of waiting for the next pass). The job stays recorded,
    /// so the next `poll_swarm_completion` applies it.
    #[doc(hidden)]
    pub async fn wait_swarm_autocomplete_for_tests(&mut self) {
        if let Some(job) = self.session.swarm_autocomplete.as_mut() {
            // A job that panicked has ended too; the next pass drops it.
            let _ = (&mut job.handle).await; // INTENTIONAL: only the end matters
        }
    }

    pub(crate) fn finalize_swarm_completion(
        &mut self,
        team_name: &str,
        total: usize,
        completed: usize,
        cancelled: usize,
    ) {
        let mut output = format!(
            "From: /swarm\n## [done] Swarm Complete: {team_name}\n\n\
            All **{total}** subtasks have finished ({completed} completed, {cancelled} failed/cancelled).\n\n"
        );

        // Include task table if we have tasks
        if total > 0 {
            if let Some(tl) = self.load_swarm_tasks(team_name) {
                output.push_str(
                    "| ID | Title | Status | Result |\n|----|-------|--------|--------|\n",
                );
                for task in &tl.tasks {
                    let icon = match task.status {
                        team::TaskStatus::Completed => "[ok]",
                        team::TaskStatus::Cancelled => "[err]",
                        _ => "[warn]",
                    };
                    // First line only: the full result stays in `tasks.json`.
                    let result = task
                        .result
                        .as_deref()
                        .and_then(|r| r.lines().find(|l| !l.trim().is_empty()))
                        .unwrap_or("-");
                    output.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        task.id,
                        codemason_types::mdtable::cell(&task.title),
                        icon,
                        codemason_types::mdtable::cell(result)
                    ));
                }
                output.push('\n');
            }
        }

        // The swarm's team leader synthesises the combined answer and cleans
        // the team up; the frontend only reports the tally.
        output.push_str(
            "The team leader combines the results into one answer and cleans up the team.\n",
        );

        self.append_assistant_text(&output);
        self.status = format!("[done] swarm complete: {team_name}");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("Swarm complete: {team_name} - {completed}/{total} tasks done"),
        );

        if let Some(ref mut s) = self.session.swarm_state {
            s.completed = true;
        }
    }
}

/// What [`seed_swarm_team`] leaves for the frontend to show.
struct SwarmSeeded {
    /// The team config after seeding.
    config: team::TeamConfig,
    /// The recorded teammates, one per subtask.
    members: Vec<team::TeamMember>,
    /// One log line per teammate.
    log: Vec<String>,
}

/// Seed a swarm team inside its leader's launch, before the leader's first
/// step: turn auto-claim off, add one task per subtask, record one teammate
/// per subtask (dependent ones `Blocked`), give ready tasks to their
/// teammates and start the ready teammates through the team's manager.
///
/// Each swarm task belongs to its own teammate (pre-assigned at spawn or
/// unblock). With auto-claim on, a teammate that went idle would take the
/// next ready task with a bare "task X is yours" turn - no description, no
/// dependency results - and strand that task's own teammate.
///
/// # Errors
///
/// Any failed write to the team's config or task list; the launch then
/// fails and the team is removed.
async fn seed_swarm_team(
    leader: &team::leader::LeaderSeed,
    team_name: &str,
    decomposition: &team::SwarmDecomposition,
) -> anyhow::Result<SwarmSeeded> {
    let team_dir = leader.team_dir.as_path();
    team::TeamStore::update(team_dir, |config| {
        config.settings.auto_claim_tasks = false;
        Ok(())
    })
    .map_err(|e| anyhow::anyhow!("turning off auto-claim for {team_name}: {e}"))?;
    let task_store = team::TaskStore::open(team_dir)?;
    for st in &decomposition.tasks {
        let mut task = team::task::Task::new(&st.id, &st.title);
        task.description = st.description.clone();
        task.depends_on = st.depends_on.clone();
        task_store
            .add_task(task)
            .await
            .map_err(|e| anyhow::anyhow!("adding task {}: {e}", st.id))?;
    }

    let mut members = Vec::new();
    let mut log = Vec::new();
    for subtask in &decomposition.tasks {
        // Per-subtask agent type (classification fallback already applied).
        let agent_type = subtask
            .agent_type
            .as_deref()
            .unwrap_or(codemason_agent::team::DEFAULT_AGENT_TYPE)
            .to_string();
        let teammate_model: Option<codemason_agent::agent::ModelRef> =
            subtask.model.as_deref().and_then(|s| {
                s.split_once('/')
                    .or_else(|| s.split_once(':'))
                    .map(|(p, m)| codemason_agent::agent::ModelRef {
                        provider_id: p.to_string(),
                        model_id: m.to_string(),
                    })
            });
        // Tasks with unresolved dependencies start as Blocked; others as Spawning.
        let has_deps = !subtask.depends_on.is_empty();
        let initial_status = if has_deps {
            MemberStatus::Blocked
        } else {
            MemberStatus::Spawning
        };
        // Dependent subtasks get this prompt replaced on unblock, once their
        // predecessors' results exist (see `poll_swarm_unblock`).
        let prompt = team::build_swarm_task_prompt(subtask, &[]);
        // The roster read, id allocation and insert are one atomic
        // `TeamStore::update`, so a concurrent status write is not lost.
        let recorded = team::TeamStore::update(team_dir, |config| {
            // A readable `<agent-type>-<adjective>-<surname>` name, unique in
            // the team; the subtask is linked to it by assignment, not name.
            let teammate_name =
                codemason_agent::readable_names::readable_member_name(&agent_type, |name| {
                    config.member_by_name(name).is_some()
                });
            let agent_id = config.next_agent_id();
            let mut member = team::TeamMember::new(&teammate_name, &agent_id, &agent_type);
            member.spawn_prompt = Some(prompt.clone());
            member.model_override = teammate_model.clone();
            member.status = initial_status;
            config.members.push(member.clone());
            Ok(Some(member))
        })
        .map_err(|e| anyhow::anyhow!("recording swarm teammate for subtask {}: {e}", subtask.id))?;
        if let Some(member) = recorded {
            // A task with no dependencies is given to its teammate now. A
            // dependent one is reserved for it (assignee set, still pending)
            // and claimed on unblock, so the task list names whose subtask
            // it is from the start.
            if has_deps {
                reserve_swarm_task(team_dir, &subtask.id, &member.agent_id);
            } else {
                assign_swarm_task(team_dir, &subtask.id, &member.agent_id);
            }
            members.push(member);
        }
        let status_label = if has_deps {
            "blocked (deps)"
        } else {
            "spawning"
        };
        let teammate_name = members.last().map_or("-", |m| m.name.as_str());
        log.push(format!(
            "[bee] Swarm teammate: {} ({}) - {} ({} agent)",
            teammate_name, subtask.id, status_label, agent_type
        ));
    }

    // The manager picks up Spawning members and spawns them; Blocked members
    // are skipped until they unblock.
    std::sync::Arc::clone(&leader.manager).reconcile_spawning_members();
    let config = team::TeamStore::load(team_dir)?.config;
    Ok(SwarmSeeded {
        config,
        members,
        log,
    })
}

/// Reserve dependent swarm task `task_id` for its teammate `agent_id`: the
/// assignee is recorded while the task stays pending, so only that teammate
/// may claim it once its dependencies complete. A failure is logged; the task
/// is then assigned when it unblocks.
fn reserve_swarm_task(team_dir: &std::path::Path, task_id: &str, agent_id: &str) {
    let reserved = team::TaskStore::open(team_dir).and_then(|store| {
        store.update_task(task_id, |task| {
            task.assigned_to = Some(agent_id.to_string())
        })
    });
    if let Err(e) = reserved {
        tracing::warn!(task = %task_id, agent = %agent_id, error = %e,
            "failed to reserve swarm task for its teammate");
    }
}

/// The swarm subtask held by teammate `agent_id`: the task assigned to it.
fn swarm_task_of(
    task_list: Option<&codemason_agent::team::task::TaskList>,
    agent_id: &str,
) -> Option<String> {
    task_list?
        .tasks
        .iter()
        .find(|task| task.assigned_to.as_deref() == Some(agent_id))
        .map(|task| task.id.clone())
}

/// Give swarm task `task_id` to its teammate `agent_id` (`pre_assign_task`:
/// `in_progress`, assigned). A failure is logged, not fatal: the teammate's
/// prompt tells it to claim the task itself if completion reports it
/// unclaimed.
fn assign_swarm_task(team_dir: &std::path::Path, task_id: &str, agent_id: &str) {
    let assigned =
        team::TaskStore::open(team_dir).and_then(|store| store.pre_assign_task(task_id, agent_id));
    if let Err(e) = assigned {
        tracing::warn!(task = %task_id, agent = %agent_id, error = %e,
            "failed to pre-assign swarm task to its teammate");
    }
}

/// A finalising swarm's background job, running on the runtime: the task
/// auto-completion (see [`App::poll_swarm_completion`]). The handle doubles
/// as the in-flight flag and the result slot: the job's only
/// results are the store writes and their warnings, so its end is what the
/// next pass waits for.
///
/// Dropping it aborts the job without waiting: a session that is closed or
/// deleted, or the App going away at exit, ends its job (and kills a hook
/// it is running) rather than leaving it detached. Paths that then touch
/// the team directory use [`App::stop_swarm_autocomplete`], which also
/// waits for the job to end.
#[derive(Debug)]
pub(super) struct SwarmAutoComplete {
    /// The swarm the job completes tasks for.
    team_name: String,
    /// The spawned job.
    handle: tokio::task::JoinHandle<()>,
}

impl SwarmAutoComplete {
    /// Abort the job and wait until it has ended.
    async fn stop(mut self) {
        self.handle.abort();
        // A cancelled (or panicked) job has ended too; only the end matters.
        let _ = (&mut self.handle).await; // INTENTIONAL: only the end matters
    }
}

impl Drop for SwarmAutoComplete {
    fn drop(&mut self) {
        // No-op for a job that has already ended.
        self.handle.abort();
    }
}

/// Complete every task of `ts` that is neither completed nor cancelled, on
/// behalf of its assignee (or `"swarm"`), as a finalising swarm does once
/// every teammate has stopped. Each completion takes the store's exclusive
/// file lock for its read-modify-write only, then runs the team's
/// `TaskCompleted` hook; failures are logged and the next task still runs.
async fn complete_open_swarm_tasks(ts: team::TaskStore) {
    let Ok(tl) = ts.read() else {
        return;
    };
    for task in &tl.tasks {
        if task.status != team::TaskStatus::Completed && task.status != team::TaskStatus::Cancelled
        {
            let agent_id = task.assigned_to.as_deref().unwrap_or("swarm");
            if let Err(e) = ts.complete(&task.id, agent_id).await {
                tracing::warn!(task = %task.id, error = %e, "failed to auto-complete swarm task");
            }
        }
    }
}
