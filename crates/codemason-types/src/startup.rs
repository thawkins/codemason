//! Startup timing instrumentation.
//!
//! [`StartupTimings`] records the duration of each stage in the codemason startup
//! pipeline (CLI parse -> config load -> storage open -> provider/tool registries
//! -> TUI init -> session create -> code index, etc.).
//!
//! The collected data is displayed via the `/startup` TUI slash command so
//! users can identify which stages contribute most to the perceived startup
//! latency.

use std::time::{Duration, Instant};

/// A single named stage with its measured wall-clock duration.
#[derive(Debug, Clone)]
pub struct StartupStage {
    /// Human-readable stage label (e.g. "Config load").
    pub name: String,
    /// Duration of the stage in milliseconds.
    pub duration_ms: u128,
}

/// Collected timings for every instrumented startup stage.
#[derive(Debug, Clone)]
pub struct StartupTimings {
    /// Ordered list of completed stages.
    stages: Vec<StartupStage>,
    /// Wall-clock instant when timing started (set in [`StartupTimings::new`]).
    global_start: Instant,
    /// Frozen wall-clock total, set by [`StartupTimings::finish`]. When set,
    /// [`StartupTimings::total_elapsed_ms`] reports this value instead of the
    /// still-running clock, so `/startup` stays stable after startup completes.
    finished_total_ms: Option<u128>,
}

impl Default for StartupTimings {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupTimings {
    /// Create a new timings collector, recording the global start instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            global_start: Instant::now(),
            finished_total_ms: None,
        }
    }

    /// Record a completed stage.
    ///
    /// Call this after measuring a stage with `Instant::now()` / `elapsed()`:
    ///
    /// ```ignore
    /// let t0 = Instant::now();
    /// // ... do work ...
    /// timings.record("Config load", t0.elapsed());
    /// ```
    pub fn record(&mut self, name: impl Into<String>, duration: Duration) {
        self.stages.push(StartupStage {
            name: name.into(),
            duration_ms: duration.as_millis(),
        });
    }

    /// Freeze the wall-clock total at its current value.
    ///
    /// Call this once startup is complete (e.g. the primary agent goes ready).
    /// After this, [`Self::total_elapsed_ms`] and [`Self::untracked_ms`] report
    /// the value captured here rather than the still-advancing clock, so
    /// repeated `/startup` displays stay stable instead of growing the
    /// "Untracked" row.
    pub fn finish(&mut self) {
        if self.finished_total_ms.is_none() {
            self.finished_total_ms = Some(self.global_start.elapsed().as_millis());
        }
    }

    /// Total wall-clock elapsed time from [`StartupTimings::new`] to now.
    ///
    /// If [`Self::finish`] has been called, returns the frozen value captured
    /// at that point; otherwise returns the live elapsed time.
    #[must_use]
    pub fn total_elapsed_ms(&self) -> u128 {
        self.finished_total_ms
            .unwrap_or_else(|| self.global_start.elapsed().as_millis())
    }

    /// Sum of all recorded stage durations.
    ///
    /// Compared with [`Self::total_elapsed_ms`], the difference reveals time
    /// spent in uninstrumented sections of the startup pipeline.
    #[must_use]
    pub fn sum_stages_ms(&self) -> u128 {
        self.stages.iter().map(|s| s.duration_ms).sum()
    }

    /// Uninstrumented time: `total_elapsed_ms - sum_stages_ms`.
    #[must_use]
    pub fn untracked_ms(&self) -> u128 {
        self.total_elapsed_ms().saturating_sub(self.sum_stages_ms())
    }

    /// Return a reference to the recorded stages.
    #[must_use]
    pub fn stages(&self) -> &[StartupStage] {
        &self.stages
    }

    /// Drain all recorded stages, leaving `self` empty.
    ///
    /// This is used by `run_tui()` to extract sub-stages recorded inside
    /// `App::new()` and merge them into the main timings collector.
    pub fn drain_stages(&mut self) -> Vec<StartupStage> {
        std::mem::take(&mut self.stages)
    }

    /// Append stages from another `StartupTimings` into `self`.
    ///
    /// The stages are moved (not copied) from `other`, leaving `other` empty.
    /// The global start of `self` is preserved; `other`'s start is discarded.
    pub fn merge_stages(&mut self, other: &mut Self) {
        self.stages.append(&mut other.stages);
    }

    /// Format the timings as a human-readable report string.
    ///
    /// The output is a Markdown table (`Stage`, `Time`) so the GUI's Markdown
    /// viewer lays the rows out as a table and the plain-text renderer's
    /// html2text pass draws it as an ASCII grid.
    #[must_use]
    pub fn format_report(&self) -> String {
        use crate::mdtable;
        let ms = |value: u128| format!("{value} ms");
        let mut rows: Vec<Vec<String>> = self
            .stages
            .iter()
            .map(|stage| vec![mdtable::cell(&stage.name), ms(stage.duration_ms)])
            .collect();
        // Show the sum of instrumented stages alongside the wall-clock total
        // so the uninstrumented gap is immediately visible.
        rows.push(vec!["Sum".to_string(), ms(self.sum_stages_ms())]);
        rows.push(vec!["Untracked".to_string(), ms(self.untracked_ms())]);
        rows.push(vec!["Total".to_string(), ms(self.total_elapsed_ms())]);
        let mut out = String::from("From: /startup\n\n");
        out.push_str(&mdtable::table(&["Stage", "Time"], rows));
        out
    }
}
