//! The Tasks panel (the Tasks pane): the session's task list.
//!
//! app-core keeps the rows (`session.tasks_cache_rows`, refreshed by
//! `App::refresh_tasks_cache` only when a task event marked it dirty, and
//! forced dirty when `Alt+I` shows the panel). This panel draws them as the
//! TUI's tasks panel does: `[STATUS] id title`, coloured by status
//! (pending yellow, in progress cyan, completed green, blocked red), with
//! the owner in brackets, a red `[blocked by #id, ...]` suffix when a
//! dependency is still open (`codemason_storage::compute_task_dag`), and an
//! indented `-> active form` line under a task in progress. With no rows it
//! shows `No tasks`.
//!
//! The lines are virtualised ([`show_lines`]); the list follows app-core's
//! `tasks_scroll_offset` (Ctrl+PageUp/PageDown, lines from the bottom).
//!
//! Dependencies: `codemason-app-core`, `codemason-storage` (the DAG) and
//! `egui`.

use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use egui::{Color32, Role};

use super::rows::{Follow, Line, label_panel, show_lines};
use crate::palette::Palette;

/// Accessible label of the task list.
pub const TASKS_LABEL: &str = "Tasks";

/// Shown while the session has no tasks, as in the TUI.
pub const EMPTY_TEXT: &str = "No tasks";

/// The colour of a task in `status` (upper case), as in the TUI.
fn status_color(status: &str, palette: &Palette) -> Color32 {
    match status {
        "PENDING" => palette.color(ThemeColor::Warning),
        "IN_PROGRESS" => palette.color(ThemeColor::Info),
        "COMPLETED" | "DONE" => palette.color(ThemeColor::Success),
        "BLOCKED" => palette.color(ThemeColor::Error),
        _ => palette.muted,
    }
}

/// The panel's lines for the cached task rows.
fn lines(app: &App, palette: &Palette) -> Vec<Line> {
    let rows = &app.session.tasks_cache_rows;
    if rows.is_empty() {
        return vec![Line::new().push(EMPTY_TEXT, palette.muted)];
    }
    let dag = codemason_storage::compute_task_dag(rows);
    let red = palette.color(ThemeColor::Error);
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let status = row.status.to_uppercase();
        let blocked = dag.get(&row.id).is_some_and(|d| d.is_blocked);
        let color = if blocked {
            red
        } else {
            status_color(&status, palette)
        };
        let mut line = Line::new()
            .push(format!("[{status}] "), color)
            .push(format!("{} ", row.id), palette.muted)
            .push(row.title.clone(), color);
        if let Some(owner) = row.owner.as_deref().filter(|o| !o.is_empty()) {
            line = line.push(format!(" ({owner})"), palette.muted);
        }
        if blocked {
            let deps: Vec<String> = row.blocked_by.iter().map(|id| format!("#{id}")).collect();
            line = line.push(format!(" [blocked by {}]", deps.join(", ")), red);
        }
        lines.push(line);
        if row.status == "in_progress"
            && let Some(active) = row.active_form.as_deref().filter(|a| !a.is_empty())
        {
            lines.push(Line::new().push(format!("  -> {active}"), palette.color(ThemeColor::Info)));
        }
    }
    lines
}

/// Draw the Tasks panel into `ui`, refreshing app-core's task cache first
/// when it is dirty.
///
/// # Arguments
///
/// * `ui` - the Tasks pane's body.
/// * `app` - the App whose task rows are drawn.
/// * `palette` - the colours to draw with.
pub fn tasks_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    app.refresh_tasks_cache();
    let lines = lines(app, palette);
    let drawn = show_lines(
        ui,
        "tasks_rows",
        lines.len(),
        |index| lines.get(index).cloned().unwrap_or_default(),
        None,
        |_| false,
        Follow::FromBottom(app.tasks_scroll_offset),
    );
    label_panel(ui, drawn.rect, "tasks_list", Role::List, TASKS_LABEL);
}
