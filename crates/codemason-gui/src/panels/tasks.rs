//! The Tasks panel (the Tasks pane): the session's task list.
//!
//! app-core keeps the rows (`session.tasks_cache_rows`, refreshed by
//! `App::refresh_tasks_cache` only when a task event marked it dirty, and
//! forced dirty when `Alt+I` shows the panel). This panel draws them as the
//! TUI's tasks panel orders them: `status #id title`. The status is a short
//! word in a fixed-width column (`todo` yellow, `active` cyan, `done` green,
//! `blocked` red) so every title starts in the same column; the title is
//! body text (muted once done), followed by the owner in brackets, a red
//! `blocked by #id, ...` suffix when a dependency is still open
//! (`codemason_storage::compute_task_dag`), and an `-> active form` line
//! under a task in progress, aligned with its title. With no rows it says
//! what the panel will hold ([`EMPTY_TEXT`], [`EMPTY_HINT`]).
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

/// Shown while the session has no tasks.
pub const EMPTY_TEXT: &str = "No tasks yet";

/// The line under [`EMPTY_TEXT`]: what will fill the panel.
pub const EMPTY_HINT: &str = "The agent lists its plan here.";

/// Width of the status column, the longest word (`blocked`) plus a space.
const STATUS_W: usize = 8;

/// The short word and colour for a task in `status` (as stored, lower case).
fn status_presentation(status: &str, palette: &Palette) -> (String, Color32) {
    match status {
        "pending" => ("todo".to_string(), palette.color(ThemeColor::Warning)),
        "in_progress" => ("active".to_string(), palette.color(ThemeColor::Info)),
        "completed" | "done" => ("done".to_string(), palette.color(ThemeColor::Success)),
        "blocked" => ("blocked".to_string(), palette.color(ThemeColor::Error)),
        other => (other.replace('_', " "), palette.muted),
    }
}

/// The panel's lines for the cached task rows.
fn lines(app: &App, palette: &Palette) -> Vec<Line> {
    let rows = &app.session.tasks_cache_rows;
    if rows.is_empty() {
        return vec![
            Line::new().push(EMPTY_TEXT, palette.text),
            Line::new().push(EMPTY_HINT, palette.muted),
        ];
    }
    let dag = codemason_storage::compute_task_dag(rows);
    let red = palette.color(ThemeColor::Error);
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let blocked = dag.get(&row.id).is_some_and(|d| d.is_blocked);
        let (word, color) = if blocked {
            ("blocked".to_string(), red)
        } else {
            status_presentation(&row.status.to_lowercase(), palette)
        };
        let done = matches!(row.status.as_str(), "completed" | "done");
        let id = format!("#{} ", row.id);
        let lead = STATUS_W + id.chars().count();
        let mut line = Line::new()
            .push(format!("{word:<STATUS_W$}"), color)
            .push(id, palette.muted)
            .push(
                row.title.clone(),
                if done { palette.muted } else { palette.text },
            );
        if let Some(owner) = row.owner.as_deref().filter(|o| !o.is_empty()) {
            line = line.push(format!(" ({owner})"), palette.muted);
        }
        if blocked {
            let deps: Vec<String> = row.blocked_by.iter().map(|id| format!("#{id}")).collect();
            line = line.push(format!("  blocked by {}", deps.join(", ")), red);
        }
        lines.push(line);
        if row.status == "in_progress"
            && let Some(active) = row.active_form.as_deref().filter(|a| !a.is_empty())
        {
            lines.push(Line::new().push(
                format!("{:lead$}-> {active}", ""),
                palette.color(ThemeColor::Info),
            ));
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
