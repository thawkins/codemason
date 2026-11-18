//! The Active agents panel (the Active agents pane): the TUI's Agents window
//! table.
//!
//! The rows are the TUI's (`layout_active_agents.rs`): a header naming the
//! columns, the primary agent, an empty-state hint while nothing else would
//! show, the ancestor-indented sub-agent tree (status suffixes, `[C]`/`[T]`
//! badges, waiter counts, the `fg`/`bg`/`detached` contract, elapsed time,
//! steps, and the `[stop]`/`>` and `[x]` indicators), the background shell
//! tasks and the completed history, then the key legend.
//!
//! # Keys and clicks
//!
//! app-core runs the window's keys while it is shown (the router sends them
//! to the App, as the TUI's popup takes them): Up/Down and PageUp/PageDown
//! move the cursor, Space stops or resumes the cursor's sub-agent, Delete
//! (or Shift+K) asks to kill it (`App::row_action_confirm`, drawn by
//! `crate::dialogs::confirm`), Enter opens its output and Esc closes the
//! window. As the TUI's renderer does, this panel publishes the painted row
//! keys (`agents_row_task_ids`) and the sub-agent ids in paint order
//! (`agents_subagent_task_ids`) that those actions resolve the cursor
//! against, and a click on a row calls `App::agents_cursor_from_click`. For
//! the paging keys it reports the rows in view as `active_agents_area`'s
//! height.
//!
//! The rows are virtualised ([`show_lines`]) and the cursor row is kept in
//! view.
//!
//! Dependencies: `codemason-agent` (task entries), `codemason-app-core`,
//! `codemason-types` (id and duration formatting) and `egui`.

use std::collections::{HashMap, HashSet};

use codemason_agent::task::{TaskEntry, TaskStatus};
use codemason_app_core::app::App;
use codemason_app_core::geometry::Area;
use codemason_app_core::styled::ThemeColor;
use codemason_types::strutil::{format_duration_compact, short_suffix};
use egui::{Color32, Role};

use super::rows::{Follow, Line, label_panel, show_lines};
use crate::palette::Palette;

/// Accessible label of the agents table.
pub const AGENTS_LABEL: &str = "Active agents";

/// The key legend under the table, as in the TUI.
pub const LEGEND: &str =
    "Up/Down select  PgUp/PgDn page  Space stop/resume  Del kill  Enter open  Esc close";

/// The hint shown when no sub-agent, background task or history row exists.
pub const EMPTY_HINT: &str = "(no sub-agents yet - start one with /spawn or a background command)";

/// Column widths of the table, as in the TUI.
const ID_W: usize = 10;
/// See [`ID_W`].
const LABEL_W: usize = 31;
/// See [`ID_W`].
const TYPE_W: usize = 9;
/// See [`ID_W`].
const ELAPSED_W: usize = 8;
/// See [`ID_W`].
const STEPS_W: usize = 7;
/// See [`ID_W`].
const STOP_W: usize = 6;
/// Widest background shell command shown.
const BG_COMMAND_MAX_W: usize = 40;

/// The colours of one row: its marker and its label.
#[derive(Clone, Copy)]
struct RowStyle {
    marker: Color32,
    row: Color32,
}

/// The status marker colour, row colour and label suffix of `status`.
fn status_presentation(status: &TaskStatus, palette: &Palette) -> (RowStyle, &'static str) {
    let c = |color| palette.color(color);
    let (color, suffix) = match status {
        TaskStatus::Running => (c(ThemeColor::Warning), ""),
        TaskStatus::Suspended => (c(ThemeColor::Subtle), " [||]"),
        TaskStatus::Terminating => (c(ThemeColor::Error), " ..."),
        TaskStatus::Completed => (c(ThemeColor::Info), ""),
        TaskStatus::Failed => (c(ThemeColor::Error), ""),
        TaskStatus::Cancelled => (c(ThemeColor::Subtle), ""),
        TaskStatus::TimedOut => (c(ThemeColor::Warning), " [timeout]"),
    };
    (
        RowStyle {
            marker: color,
            row: color,
        },
        suffix,
    )
}

/// `text` cut to `budget` characters, the cut marked with `..`.
fn fit_chars(text: &str, budget: usize) -> String {
    if text.chars().count() <= budget {
        return text.to_string();
    }
    match budget {
        0 => String::new(),
        1 | 2 => text.chars().take(budget).collect(),
        _ => format!("{}..", text.chars().take(budget - 2).collect::<String>()),
    }
}

/// The contract word of a sub-agent row.
fn contract(detached: bool, background: bool) -> &'static str {
    if detached {
        "detached"
    } else if background {
        "bg"
    } else {
        "fg"
    }
}

/// Elapsed time since `created_at`, compact.
fn elapsed(created_at: chrono::DateTime<chrono::Utc>) -> String {
    format_duration_compact((chrono::Utc::now() - created_at).num_seconds())
}

/// The fixed leading columns of a row.
#[allow(clippy::too_many_arguments)]
fn fixed(
    marker: &str,
    style: RowStyle,
    id: &str,
    label: &str,
    type_label: &str,
    elapsed: &str,
    steps: &str,
    palette: &Palette,
) -> Line {
    Line::new()
        .push(marker, style.marker)
        .push(format!("{id:<ID_W$} "), palette.muted)
        .push(format!("{label:<LABEL_W$}"), style.row)
        .push(format!("{type_label:<TYPE_W$}"), palette.muted)
        .push(format!("{elapsed:>ELAPSED_W$} "), palette.muted)
        .push(format!("{steps:>STEPS_W$} "), palette.muted)
}

/// The `[stop]`/`>` and `[x]` indicators of a live row.
fn controls(line: Line, suspended: bool, palette: &Palette) -> Line {
    let (glyph, color) = if suspended {
        (">", palette.color(ThemeColor::Success))
    } else {
        ("[stop]", palette.color(ThemeColor::Warning))
    };
    line.push(format!("{glyph:<STOP_W$} "), color)
        .push("[x]", palette.color(ThemeColor::Error))
}

/// What the sub-agent tree rows read.
struct TreeContext<'a> {
    app: &'a App,
    tasks: HashMap<&'a str, Vec<&'a TaskEntry>>,
    custom: HashSet<String>,
    teammates: HashSet<String>,
    palette: &'a Palette,
}

/// Push the sub-agent rows under `parent`, depth first, with their keys.
fn tree_rows(
    ctx: &TreeContext<'_>,
    parent: &str,
    last_stack: &[bool],
    out: &mut Vec<(Line, String)>,
) {
    let children = ctx.tasks.get(parent).cloned().unwrap_or_default();
    for (index, task) in children.iter().enumerate() {
        let is_last = index + 1 == children.len();
        let (style, status_suffix) = status_presentation(&task.status, ctx.palette);
        let mut prefix: String = last_stack
            .iter()
            .map(|&last| if last { "  " } else { "| " })
            .collect();
        prefix.push_str(if is_last { "`- " } else { "|- " });
        let mut suffix = String::new();
        if ctx.custom.contains(&task.agent_name) {
            suffix.push_str(" [C]");
        }
        if ctx.teammates.contains(&task.child_session_id) {
            suffix.push_str(" [T]");
        }
        suffix.push_str(status_suffix);
        if task.waiter_count > 0 && task.completed_at.is_none() {
            suffix.push_str(&format!(" [w{}]", task.waiter_count));
        }
        let used = prefix.chars().count() + suffix.chars().count();
        let budget = LABEL_W.saturating_sub(used).saturating_sub(1);
        let label = format!("{prefix}{}{suffix}", fit_chars(&task.id, budget));
        let steps = ctx
            .app
            .event_bus
            .current_tool_calls(&task.child_session_id)
            .to_string();
        let mut line = fixed(
            "o ",
            style,
            &short_suffix(&task.id, 8),
            &label,
            contract(task.detached, task.background),
            &elapsed(task.created_at),
            &steps,
            ctx.palette,
        );
        if !task.status.is_terminal() {
            line = controls(line, task.status == TaskStatus::Suspended, ctx.palette);
        }
        out.push((line, task.id.clone()));
        let mut stack = last_stack.to_vec();
        stack.push(is_last);
        tree_rows(ctx, &task.child_session_id, &stack, out);
    }
}

/// Every row of the table with its key (empty for a row the cursor cannot
/// select, `"primary"` for the primary agent, else the sub-agent's task id),
/// and the range of the sub-agent tree rows (the cursor's rows; the
/// background and history rows after them are not).
fn rows(app: &App, palette: &Palette) -> (Vec<(Line, String)>, std::ops::Range<usize>) {
    let dim = palette.muted;
    let primary_session = app.session.session_id.clone().unwrap_or_default();
    let mut tasks: HashMap<&str, Vec<&TaskEntry>> = HashMap::new();
    for task in &app.active_tasks {
        tasks
            .entry(task.parent_session_id.as_str())
            .or_default()
            .push(task);
    }
    let custom: HashSet<String> = app
        .custom_agent_defs
        .iter()
        .map(|def| def.agent_info.name.clone())
        .collect();
    let teammates: HashSet<String> = app
        .session
        .team_members
        .iter()
        .filter_map(|member| member.session_id.clone())
        .collect();
    let mut out: Vec<(Line, String)> = vec![(
        Line::new().push(
            format!(
                "  {:<ID_W$} {:<LABEL_W$}{:<TYPE_W$}{:>ELAPSED_W$} {:>STEPS_W$} {:<STOP_W$} [x]",
                "id", "name", "type", "elapsed", "steps", "[stop]"
            ),
            dim,
        ),
        String::new(),
    )];
    let mut primary_label = app.session.agent_name.clone();
    if custom.contains(&primary_label) {
        primary_label.push_str(" [C]");
    }
    let green = palette.color(ThemeColor::Success);
    out.push((
        fixed(
            "* ",
            RowStyle {
                marker: green,
                row: green,
            },
            "lead",
            &fit_chars(&primary_label, LABEL_W.saturating_sub(1)),
            "primary",
            "-",
            &app.event_bus
                .current_tool_calls(&primary_session)
                .to_string(),
            palette,
        ),
        "primary".to_string(),
    ));
    if !app.agents_window_available() {
        out.push((
            Line::new().push(format!("  {EMPTY_HINT}"), dim),
            String::new(),
        ));
    }
    let start = out.len();
    let ctx = TreeContext {
        app,
        tasks,
        custom,
        teammates,
        palette,
    };
    tree_rows(&ctx, &primary_session, &[], &mut out);
    let tree = start..out.len();
    let cyan = palette.color(ThemeColor::Info);
    if !app.bg_tasks.is_empty() {
        out.push((
            Line::new().push(" Background shell tasks ", cyan),
            String::new(),
        ));
        for task in &app.bg_tasks {
            let color = match task.status.as_str() {
                "running" => palette.color(ThemeColor::Warning),
                "completed" => cyan,
                "failed" | "cancelled" => palette.color(ThemeColor::Error),
                _ => palette.color(ThemeColor::Subtle),
            };
            let style = RowStyle {
                marker: color,
                row: color,
            };
            let command = fit_chars(&task.command, LABEL_W.min(BG_COMMAND_MAX_W));
            let mut line = fixed(
                "o ",
                style,
                &short_suffix(&task.id, 8),
                &command,
                "bg",
                &elapsed(task.created_at),
                "-",
                palette,
            );
            if task.status == "running" {
                line = controls(line, false, palette);
            }
            out.push((line, String::new()));
        }
    }
    if !app.agents_history.is_empty() {
        out.push((
            Line::new().push(" Completed (newest first) ", cyan),
            String::new(),
        ));
        for row in &app.agents_history {
            let (style, _) = status_presentation(&row.status, palette);
            let label = format!("{} [{}]", row.agent_name, row.status.as_str());
            let mut line = fixed(
                "o ",
                style,
                &short_suffix(&row.id, 8),
                &fit_chars(&label, LABEL_W.saturating_sub(1)),
                contract(row.detached, row.background),
                &format_duration_compact(i64::try_from(row.duration_secs()).unwrap_or(i64::MAX)),
                "-",
                palette,
            );
            if let Some(note) = row.durable_copy_absence() {
                let yellow = palette.color(ThemeColor::Warning);
                line = line.push("[memory] ", yellow).push(note, yellow);
            }
            out.push((line, String::new()));
        }
    }
    (out, tree)
}

/// Draw the Active agents panel into `ui`, publish the row keys app-core's
/// cursor actions resolve against, and move the cursor to a clicked row.
///
/// # Arguments
///
/// * `ui` - the Active agents pane's body.
/// * `app` - the App whose tasks are drawn; a click selects through
///   `App::agents_cursor_from_click`.
/// * `palette` - the colours to draw with.
pub fn agents_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    let (rows, tree) = rows(app, palette);
    let keys: Vec<String> = rows.iter().map(|(_, key)| key.clone()).collect();
    // The cursor walks the sub-agent tree rows only, as in the TUI.
    app.agents_subagent_task_ids = keys.get(tree).unwrap_or_default().to_vec();
    app.agents_row_task_ids = keys;
    let cursor_row = app
        .agents_cursor_subagent
        .and_then(|cursor| app.agents_subagent_task_ids.get(cursor))
        .and_then(|id| app.agents_row_task_ids.iter().position(|key| key == id));
    let legend_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    let mut drawn = None;
    ui.vertical(|ui| {
        let list_height = (ui.available_height() - legend_height).max(0.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), list_height), |ui| {
            drawn = Some(show_lines(
                ui,
                "agents_rows",
                rows.len(),
                |index| rows.get(index).map(|(l, _)| l.clone()).unwrap_or_default(),
                cursor_row,
                |index| rows.get(index).is_some_and(|(_, key)| !key.is_empty()),
                Follow::None,
            ));
        });
        ui.weak(LEGEND);
    });
    let Some(drawn) = drawn else {
        return;
    };
    label_panel(ui, drawn.rect, "agents_table", Role::Table, AGENTS_LABEL);
    // app-core pages the cursor by the rows in view.
    let row_height = ui
        .ctx()
        .fonts_mut(|f| f.row_height(&egui::FontId::monospace(crate::styled_text::FONT_SIZE)));
    app.active_agents_area = Area {
        height: rows_in_view(drawn.rect.height(), row_height),
        ..Area::default()
    };
    if let Some(index) = drawn.clicked {
        app.agents_cursor_from_click(index);
        ui.ctx().request_repaint();
    }
}

/// How many rows of `row_height` fit in `height`.
pub(crate) fn rows_in_view(height: f32, row_height: f32) -> u16 {
    let rows = (height / row_height.max(1.0)).floor().max(1.0);
    u16::try_from(rows as usize).unwrap_or(u16::MAX)
}
