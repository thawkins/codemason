//! The background housekeeping pass both frontends run.
//!
//! Background jobs (compaction, rollback, `/swarm`, `/bench`, sub-agent
//! launches, store fetches, background sessions, ...) deposit their results
//! into shared slots on the [`App`]; nothing applies them until a poll reads
//! the slot. [`App::housekeeping`] is that set of polls, in one fixed order,
//! so the terminal frontend (from its event loop) and the GUI (every frame)
//! apply results the same way. It moved here verbatim from the TUI event
//! loop: same calls, same order, same `HOUSEKEEPING_INTERVAL` gate.
//!
//! Depends on `codemason_agent`'s `SessionProcessor` for the MCP client
//! adoption step.

use codemason_agent::session::processor::SessionProcessor;

use super::App;

impl App {
    /// Run one housekeeping pass: the periodic polls that apply background
    /// results, then the per-wake tail.
    ///
    /// The polls run at most once per [`super::HOUSEKEEPING_INTERVAL`] (see
    /// [`App::run_housekeeping_if_due`]); each keeps its own coarser cadence
    /// inside. The tail - copying the off-thread memory count into the
    /// status-bar cache and the debounced history flush - runs on every call.
    ///
    /// Most awaits are short: the MCP and connector steps use `try_read` on
    /// the client lock and skip when it is held. Finalising a swarm
    /// (`poll_swarm_completion`, also run per background session by
    /// `poll_background_sessions`) completes the open tasks in a job spawned
    /// on the runtime, because each `TaskStore::complete` takes the task
    /// store's blocking exclusive file lock, which teammate processes
    /// contend for, and runs the team's `TaskCompleted` hook (30 s timeout,
    /// `HOOK_TIMEOUT`); the pass after the job ends applies the tally.
    /// Cancelling the swarm or closing, clearing or removing its team stops
    /// and awaits the job first (`App::stop_swarm_autocomplete`); closing
    /// its session or exiting aborts it (its running hook is killed), which
    /// leaves the tasks it had not reached open.
    /// Applying a finished `/swarm` decomposition (`poll_pending_swarm` ->
    /// `TaskStore::add_task`) still writes inline: the team was created in
    /// the same step, so no teammate holds its lock yet and it has no hooks
    /// configured, so no `TaskCreated` hook runs there.
    ///
    /// # Arguments
    ///
    /// * `session_processor` - the processor whose MCP client is adopted into
    ///   the `/mcp` list (the TUI passes the one it was started with).
    pub async fn housekeeping(&mut self, session_processor: &SessionProcessor) {
        // PERF-045: run the cheap periodic polls/refreshes at most once per
        // `HOUSEKEEPING_INTERVAL`.  A wake that merely drained one keystroke
        // or one streamed token no longer re-probes every job; each job keeps
        // its own internal cadence, and the frontend keeps a wake scheduled
        // for the end of the interval so a quiet period is still polled
        // (stat refreshes, expiry transitions, spinner reaping).
        // The background code-index startup result is applied as soon as it
        // arrives, not on the housekeeping cadence.
        self.poll_code_index_startup();
        if self.run_housekeeping_if_due() {
            // Stream /new scaffold progress and surface the finished summary.
            self.poll_newproj_result();

            // Stream /spec govcreate staged progress and surface the report.
            self.poll_govcreate_result();

            // Fill the open plugin-store / connector-catalogue browser from the
            // store cache: a landed index, streamed manifest updates, a failure.
            self.poll_store_cache();

            // Apply a completed off-loop plugin-store install (T-009/FR-011).
            self.poll_plugin_store_install_result();

            // Surface the `/plugins stores --check` availability probe.
            self.poll_plugin_store_probe_result();

            // Apply a completed off-loop connector-catalogue install.
            self.poll_connector_catalogue_install_result();

            // Check for completed off-thread codeindex graph builds / reindexes.
            self.poll_codeindex_bg_result();

            // Surface the loop-rollback outcome (status + message window).
            self.poll_rollback_result();

            // Surface the /websearch test engine-diagnostic outcome.
            self.poll_websearch_test_result();

            // Check for completed compaction runs.
            self.poll_compaction_result();

            // Surface OTLP metric-export failures ("Agent Notice" + auto-disable
            // after repeated failures). The exporter records outcomes on the
            // tokio runtime, so this poll is the only path that reaches the UI.
            self.poll_telemetry_export_health();

            // Adopt background Context panel snapshot refreshes (T-013/FR-015).
            self.poll_context_snapshot_refresh();

            // Check for completed /swarm LLM decomposition results.
            self.poll_pending_swarm().await;

            // Check for completed /bench background runs.
            self.poll_pending_bench();

            // Write startup notices produced off the event loop to the log.
            self.poll_startup_notices();

            // Drain any completed shared sub-agent launch (D3).
            self.poll_subagent_launch_results();

            // Reap spinner latches that outlived their staleness caps (dropped
            // `*Finished` events after a broadcast Lagged burst).
            self.poll_stale_spinners();

            // Re-sync the Agents panel with the task registry after a broadcast
            // lag burst (dropped SubagentStart/Complete events).
            self.poll_active_tasks_reconcile();
            self.close_finished_subagent_views();

            // The App's wall-clock timers, shared with the GUI: statuses go
            // to "ready" after a grace period, the run-cost banner dismisses
            // itself after 15 seconds, and a timed question answers its
            // default option once its deadline passes.
            self.poll_timers();

            // Unblock swarm tasks whose dependencies are satisfied.
            self.poll_swarm_unblock();

            // Check if active swarm has completed all tasks.
            self.poll_swarm_completion().await;

            // Fire any pending autopilot continuation.
            self.poll_autopilot_continue();

            // Run the same session-scoped polls for background sessions so
            // their results land while they are not focused (MULTISESSION M3).
            self.poll_background_sessions().await;

            // Refresh pull/merge request counts for the sidebar (spec
            // `projects`); the fetch itself runs on a background task.
            self.poll_request_counts();

            // Focus sessions whose branch worktrees finished being created.
            self.poll_pending_project_sessions();

            // Refresh cached stats on their throttled intervals.
            self.refresh_code_index_stats();
            self.refresh_memory_stats();

            // Adopt the startup MCP connect loop's client the first time it
            // appears. The frontend adopts it up front, but the loop can still
            // be connecting; without this retry `/mcp` would keep showing the
            // seeded `disabled` for a late-finishing server.
            self.adopt_mcp_client_state(session_processor).await;

            // Publish the connector status snapshot the same way, so
            // `/connectors list` reports `connected` and real tool counts for a
            // bridged server the startup loop connected (FR-009).
            self.refresh_connector_statuses().await;
        }

        // Copy the latest off-thread memory count into the status bar cache
        // so it stays visible even when no events are arriving.
        self.memory_entry_count = self
            .memory_entry_count_pending
            .load(std::sync::atomic::Ordering::Relaxed);

        // Flush dirty history to disk (non-blocking, debounced).
        self.flush_history_if_due();
    }
}
