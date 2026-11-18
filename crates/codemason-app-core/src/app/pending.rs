//! Whether the housekeeping pass has anything to apply: the GUI's idle check.
//!
//! Background jobs deposit their results into shared slots on the [`App`]
//! and wake nothing; [`App::housekeeping`] applies them. The terminal
//! frontend wakes for the pass every `HOUSEKEEPING_INTERVAL` regardless; the
//! GUI asks [`App::has_pending_work`] after each pass and keeps that wake
//! only while something is outstanding, otherwise it sleeps until input, an
//! agent event (the event bridge wakes the window) or the next timer
//! (`App::next_timer_due`, [`App::next_periodic_due`]).
//!
//! A result slot is outstanding while a job holds it or it holds a result:
//! every job is handed a clone of its slot's `Arc` and keeps it until it has
//! deposited, so `Arc::strong_count(slot) > 1` is exactly "a job will still
//! deposit here", with no per-job in-flight flag to keep in step. A slot the
//! App itself shares long-term would read as always outstanding, which only
//! costs the old 200 ms wake, never a lost result.
//!
//! Not counted (they land on the next wake of any kind): the OTLP exporter's
//! failure notices, the periodic memory and code-index stat refreshes, and
//! the connector status snapshot. They report external state and have no
//! job to wait for.
//!
//! Depends on nothing beyond the App's own state.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::App;
use super::session_view::SessionView;

/// Whether a job still holds `slot`, or `slot` holds a result
/// (`has_result`) the pass has not applied yet. A poisoned lock counts as
/// outstanding, so the pass runs and recovers it.
fn slot_outstanding<T>(slot: &Arc<Mutex<T>>, has_result: impl FnOnce(&T) -> bool) -> bool {
    Arc::strong_count(slot) > 1 || slot.lock().map_or(true, |value| has_result(&value))
}

/// [`slot_outstanding`] for an `Option` result slot.
fn option_outstanding<T>(slot: &Arc<Mutex<Option<T>>>) -> bool {
    slot_outstanding(slot, Option::is_some)
}

/// [`slot_outstanding`] for a `Vec` queue of results.
fn queue_outstanding<T>(slot: &Arc<Mutex<Vec<T>>>) -> bool {
    slot_outstanding(slot, |queue| !queue.is_empty())
}

/// What a session's own polls (run for the focused session and, through
/// `poll_background_sessions`, for every background one) still wait for.
fn session_outstanding(view: &SessionView) -> bool {
    option_outstanding(&view.rollback_result)
        || option_outstanding(&view.compact_result)
        || option_outstanding(&view.context_snapshot_result)
        || option_outstanding(&view.swarm_result)
        || view
            .swarm_state
            .as_ref()
            .is_some_and(|swarm| !swarm.completed)
        || view.swarm_autocomplete.is_some()
        || view.autopilot_pending_continue.is_some()
}

impl App {
    /// Whether the housekeeping pass has anything to apply or watch: a
    /// background job that will deposit a result, a deposited result not yet
    /// applied, or a state the pass advances on its own (an active swarm, a
    /// pending autopilot continuation, a running `/bench`, a spinner latch it
    /// reaps, the Agents panel's reconcile, an unadopted MCP client, an
    /// unflushed history).
    ///
    /// # Returns
    ///
    /// `true` while the frontend should keep waking for the pass every
    /// `HOUSEKEEPING_INTERVAL`; `false` when it may sleep until input, an
    /// agent event or the next timer.
    #[must_use]
    pub fn has_pending_work(&self) -> bool {
        let jobs = queue_outstanding(&self.newproj_progress)
            || option_outstanding(&self.newproj_result)
            || queue_outstanding(&self.govcreate_progress)
            || option_outstanding(&self.govcreate_result)
            || option_outstanding(&self.plugin_store_result)
            || queue_outstanding(&self.store_enrich_result)
            || option_outstanding(&self.plugin_store_install_result)
            || option_outstanding(&self.plugin_store_probe_result)
            || option_outstanding(&self.connector_catalogue_result)
            || queue_outstanding(&self.connector_enrich_result)
            || option_outstanding(&self.connector_catalogue_install_result)
            || option_outstanding(&self.code_index_bg_result)
            || option_outstanding(&self.websearch_test_result)
            || option_outstanding(&self.active_tasks_reconcile_result)
            || queue_outstanding(&self.subagent_launch_results)
            || queue_outstanding(&self.startup_notices)
            || queue_outstanding(&self.pending_project_sessions);
        let sessions = std::iter::once(&self.session)
            .chain(self.background.iter())
            .any(session_outstanding);
        let watched = self.active_bench_task_id.is_some()
            || self.govcreate_running
            || self.code_index_busy
            || self.code_index_graph_spawned
            || self.code_index_reindex_spawned
            || self.model_loading_state.is_some()
            || self.model_download_state.is_some()
            || self.periodic_animation_active()
            || !self.active_tasks.is_empty()
            || self.active_tasks_reconcile_inflight
            || self
                .tui_event_lag
                .load(std::sync::atomic::Ordering::Relaxed)
                != self.seen_event_lag
            || (!self.mcp_client_adopted && self.session_processor.mcp_client.get().is_some())
            || self.history_dirty;
        jobs || sessions || watched
    }

    /// When a periodic housekeeping step is next due while nothing is
    /// pending: the sidebar's pull/merge request count sweep (every
    /// `REQUEST_SWEEP_INTERVAL` while the sidebar is shown).
    ///
    /// # Returns
    ///
    /// The instant the next pass must run (never before the pass's own
    /// `HOUSEKEEPING_INTERVAL` gate opens), or `None` when no periodic step
    /// is armed.
    #[must_use]
    pub fn next_periodic_due(&self) -> Option<Instant> {
        if !self.sidebar.visible {
            return None;
        }
        let sweep = self.last_request_sweep.map_or_else(Instant::now, |at| {
            at + super::projects::REQUEST_SWEEP_INTERVAL
        });
        // The step runs inside a pass, which runs no sooner than its gate.
        Some(sweep.max(self.housekeeping_due_at()))
    }
}
