//! The Profile panel (the Profile pane): the agent-loop profiler's table.
//!
//! app-core turns the profiler on when the panel is shown (`Alt+P`,
//! `ToggleProfile`) and off when it is hidden; this panel only reads its
//! snapshot (`agent_loop_profiler().snapshot()`) each frame and draws what
//! the TUI's profile panel draws: the uptime and sample count, the number of
//! operations, a column header, then one row per operation (count, average,
//! total, self, max and last milliseconds, name), sorted by self time. While
//! the profiler is off it says so, and before any agent-loop activity it says
//! it is waiting.
//!
//! The rows are virtualised ([`show_lines`]); the list follows app-core's
//! `profile_scroll_offset` (Ctrl+PageUp/PageDown, lines from the bottom). The
//! panel asks for a repaint twice a second while shown, so the uptime and
//! timings stay current.
//!
//! Dependencies: `codemason-agent` (the profiler), `codemason-app-core` and
//! `egui`.

use codemason_agent::session::profiler::{ProfileSnapshot, agent_loop_profiler};
use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use egui::Role;

use super::rows::{Follow, Line, label_panel, show_lines};
use crate::palette::Palette;

/// Accessible label of the profiler table.
pub const PROFILE_LABEL: &str = "Profile";

/// Shown while the profiler is off, as in the TUI.
pub const DISABLED_TEXT: &str = "Profiler is disabled";

/// Shown before any agent-loop activity, as in the TUI.
pub const WAITING_TEXT: &str = "Waiting for agent loop activity...";

/// The column header, as the TUI draws it.
const HEADER: &str = "count     avg ms    total ms     self ms     max ms    last ms  operation";

/// How often the panel repaints while shown, so the timings move.
const REPAINT_EVERY: std::time::Duration = std::time::Duration::from_millis(500);

/// The panel's lines for `snapshot`, as the TUI's profile panel builds them.
fn lines(snapshot: &ProfileSnapshot, palette: &Palette) -> Vec<Line> {
    if !snapshot.enabled {
        return vec![Line::new().push(DISABLED_TEXT, palette.muted)];
    }
    let mut lines = vec![
        Line::new()
            .push("uptime ", palette.muted)
            .push(
                format!("{:.1}s", snapshot.running_for_ms as f64 / 1000.0),
                palette.text,
            )
            .push("  samples ", palette.muted)
            .push(snapshot.total_samples.to_string(), palette.text),
        Line::new()
            .push("ops ", palette.muted)
            .push(snapshot.operations.len().to_string(), palette.text)
            .push("  sorted by self time", palette.muted),
        Line::new().push(HEADER, palette.color(ThemeColor::Warning)),
    ];
    if snapshot.operations.is_empty() {
        lines.push(Line::new().push(WAITING_TEXT, palette.muted));
    }
    lines.extend(snapshot.operations.iter().map(|op| {
        Line::new().push(
            format!(
                "{:>5}  {:>10.2}  {:>10.2}  {:>10.2}  {:>9.2}  {:>9.2}  {}",
                op.count, op.avg_ms, op.total_ms, op.self_total_ms, op.max_ms, op.last_ms, op.name
            ),
            palette.text,
        )
    }));
    lines
}

/// Draw the Profile panel into `ui`.
///
/// # Arguments
///
/// * `ui` - the Profile pane's body.
/// * `app` - the App whose `profile_scroll_offset` the list follows.
/// * `palette` - the colours to draw with.
pub fn profile_ui(ui: &mut egui::Ui, app: &App, palette: &Palette) {
    let snapshot = agent_loop_profiler().snapshot();
    let lines = lines(&snapshot, palette);
    let drawn = show_lines(
        ui,
        "profile_rows",
        lines.len(),
        |index| lines.get(index).cloned().unwrap_or_default(),
        None,
        |_| false,
        Follow::FromBottom(app.profile_scroll_offset),
    );
    label_panel(ui, drawn.rect, "profile_table", Role::Table, PROFILE_LABEL);
    if snapshot.enabled {
        ui.ctx().request_repaint_after(REPAINT_EVERY);
    }
}
