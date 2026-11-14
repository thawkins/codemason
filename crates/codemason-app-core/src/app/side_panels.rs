//! Data refresh for the Tasks and Memory side panels.
//!
//! Both panels show rows read from SQLite and cached on the [`App`]: the
//! session's task list (`session.tasks_cache_rows`) and the project's
//! structured memories (`memory_cache_entries`). Reading them every frame
//! would issue a query per repaint, so each cache carries a dirty flag that
//! the events which change it set (task tool results and task events; memory
//! tool results, a panel delete, `/memory clear`), and a frontend calls the
//! refresh here before drawing the panel. Both frontends share it, so the
//! terminal and the GUI read the same rows.
//!
//! Dependencies: `codemason-storage` (the task and memory tables) and
//! `tracing` for refresh failures.

use super::App;

/// Most memories the Memory panel lists.
const MEMORY_PANEL_LIMIT: usize = 100;

impl App {
    /// Re-read the Tasks panel rows when the cache is dirty.
    ///
    /// The rows belong to the selected agent session, or the primary session
    /// when none is selected (the log panel's resolution). On a storage error
    /// the stale rows are kept and the cache stays dirty, so the panel does
    /// not blank out and the next call retries.
    pub fn refresh_tasks_cache(&mut self) {
        if !self.session.tasks_cache_dirty {
            return;
        }
        let session_id = self
            .session
            .selected_agent_session_id
            .clone()
            .or_else(|| self.session.session_id.clone());
        let rows = match &session_id {
            Some(sid) => self
                .storage
                .list_tasks(sid, None)
                .map_err(|e| e.to_string()),
            None => Ok(Vec::new()),
        };
        match rows {
            Ok(rows) => {
                self.session.tasks_cache_rows = rows;
                self.session.tasks_cache_dirty = false;
            }
            Err(e) => {
                tracing::warn!("tasks panel cache refresh failed: {e}");
                self.session.tasks_cache_dirty = true;
            }
        }
    }

    /// Re-read the Memory panel rows when the cache is dirty, then keep the
    /// panel cursor on a row.
    ///
    /// Lists up to 100 memories of the launch project (`cwd_path`, the real
    /// path the memories table keys on, not the `~`-collapsed `cwd`), sorted
    /// by category then id so the drawn order and the cursor index agree.
    /// Their tags are read in one batch (`memory_cache_tags`).
    /// `memory_row_count` is the number of listed rows (one selectable row
    /// per memory), and `memory_cursor` is clamped to it.
    pub fn refresh_memory_cache(&mut self) {
        if self.memory_cache_dirty {
            let project_dir = self.cwd_path.clone();
            let count = self
                .storage
                .count_memories_for_project(&project_dir)
                .unwrap_or(0); // no-error-ok: an unreadable count shows 0, as before
            let mut entries = self
                .storage
                .list_memories_for_project(&project_dir, MEMORY_PANEL_LIMIT)
                .unwrap_or_default(); // no-error-ok: an unreadable list shows the empty state
            entries.sort_by(|a, b| a.category.cmp(&b.category).then(a.id.cmp(&b.id)));
            let ids: Vec<i64> = entries.iter().map(|row| row.id).collect();
            self.memory_cache_tags = self
                .storage
                .get_memory_tags_batched(&ids)
                .unwrap_or_default(); // no-error-ok: rows without readable tags show none
            self.memory_cache_count = count;
            self.memory_cache_entries = entries;
            self.memory_cache_dirty = false;
        }
        self.memory_row_count = self.memory_cache_entries.len();
        self.memory_cursor = self
            .memory_cursor
            .min(self.memory_row_count.saturating_sub(1));
    }

    /// Put the Memory panel cursor on row `index` (a click on that row), as
    /// Up/Down move it; an index past the last row selects the last one.
    ///
    /// # Arguments
    ///
    /// * `index` - the row, in the panel's category-then-id order.
    pub fn select_memory_row(&mut self, index: usize) {
        self.memory_cursor = index.min(self.memory_row_count.saturating_sub(1));
        self.needs_redraw = true;
    }
}
