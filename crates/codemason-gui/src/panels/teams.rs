//! The Teams panel (the Teams pane): the TUI's Teams window table.
//!
//! The pane lists every team the session runs (one section per team, in team
//! name order). Each section holds the rows of the TUI's Teams window
//! (`layout_teams.rs`): the team title (`Team: name
//! (lead + N teammates)`), a header naming the columns, the lead row, one
//! row per teammate (focus marker, id, `name-id` label, agent type, status
//! word, model or `(inherited)`, elapsed, steps, claimed and done task
//! counts, messages sent and received, and the Space/Delete key hints), a `plan awaiting approval:` line under a
//! member waiting on a plan, the parked-member hint, the empty-state hint,
//! then the key legend. The claimed/done counts and parked plans are read
//! from the team's store each frame, as the TUI reads them.
//!
//! # Keys and clicks
//!
//! app-core runs the window's keys while it is shown (the router sends them
//! to the App, as the TUI's popup takes them): Up/Down and PageUp/PageDown
//! move the cursor, Space resumes a parked teammate or asks to suspend a
//! live one, Delete (or `k`) asks to shut it down (or to resume a blocked
//! one) through `App::row_action_confirm` (drawn by
//! `crate::dialogs::confirm`), Enter focuses it and opens its output, and
//! Esc closes the window. As the TUI's renderer does, this panel publishes
//! the painted row keys (`teams_row_agent_ids`) and the teammates in
//! paint order (`teams_teammate_agent_ids`), both keyed by `(team name, id)`, and a click on a row calls
//! `App::teams_cursor_from_click`. For the paging keys it reports the rows
//! in view as `teams_rows_area`'s height.
//!
//! The rows are virtualised ([`show_lines`]) and the cursor row is kept in
//! view.
//!
//! Dependencies: `codemason-agent` (team members and stores),
//! `codemason-app-core`, `codemason-types` (id and duration formatting),
//! `chrono` and `egui`.

use std::collections::HashMap;

use codemason_agent::team::{
    MemberStatus, PendingPlan, TaskStatus, TaskStore, TeamMember, TeamStore, pending_plans,
};
use codemason_app_core::app::{App, TeamSection};
use codemason_app_core::geometry::Area;
use codemason_app_core::styled::ThemeColor;
use codemason_types::strutil::{format_duration_compact, short_suffix, truncate_chars_no_ellipsis};
use egui::{Color32, Role};

use super::agents::rows_in_view;
use super::rows::{Follow, Line, label_panel, legend_height, show_lines};
use crate::palette::Palette;

/// Accessible label of the teams table.
pub const TEAMS_LABEL: &str = "Teams";

/// The key legend under the table, as in the TUI.
pub const LEGEND: &str = "Up/Down select  PgUp/PgDn page  Space stop/resume  \
     Del shut down (blocked: resume)  Enter open  Esc close  \
     | dialog: Left/Right toggles - Enter on No cancels";

/// The hint shown while the team has no teammates.
pub const EMPTY_HINT: &str = "(no teammates yet - use team_spawn tool or blueprint)";

/// Column widths of the table, as in the TUI.
const MARKER_W: usize = 4;
/// See [`MARKER_W`].
const ID_W: usize = 8;
/// See [`MARKER_W`].
const LABEL_W: usize = 31;
/// See [`MARKER_W`].
const AGENT_TYPE_W: usize = 16;
/// See [`MARKER_W`].
const STATUS_W: usize = 10;
/// See [`MARKER_W`].
const MODEL_W: usize = 17;
/// See [`MARKER_W`].
const ELAPSED_W: usize = 7;
/// See [`MARKER_W`].
const STEPS_W: usize = 5;
/// See [`MARKER_W`].
const CLAIMED_W: usize = 7;
/// See [`MARKER_W`].
const DONE_W: usize = 5;
/// See [`MARKER_W`].
const SENT_W: usize = 5;
/// See [`MARKER_W`].
const RECV_W: usize = 5;
/// See [`MARKER_W`].
const STOP_W: usize = 9;

/// Claimed and done task counts per teammate agent id.
type TaskCounts = HashMap<String, (usize, usize)>;

/// A message count, `-` for none.
fn count_cell(n: u32) -> String {
    if n == 0 {
        "-".to_string()
    } else {
        n.to_string()
    }
}

/// The colour of a member's status word, as in the TUI.
fn status_color(status: &MemberStatus, palette: &Palette) -> Color32 {
    palette.color(match status {
        MemberStatus::Spawning => ThemeColor::Warning,
        MemberStatus::Working => ThemeColor::Info,
        MemberStatus::Idle => ThemeColor::Success,
        MemberStatus::PlanPending => ThemeColor::Categorical(4),
        MemberStatus::Blocked => ThemeColor::Error,
        MemberStatus::Suspended => ThemeColor::Subtle,
        MemberStatus::ShuttingDown => ThemeColor::Warning,
        MemberStatus::Stopped => ThemeColor::Muted,
        MemberStatus::Failed => ThemeColor::Error,
    })
}

/// Whether a member in `status` has finished (no controls).
fn is_terminal(status: &MemberStatus) -> bool {
    matches!(
        status,
        MemberStatus::ShuttingDown | MemberStatus::Stopped | MemberStatus::Failed
    )
}

/// The team's store, if it can be read.
fn team_store(app: &App, team_name: &str) -> Option<TeamStore> {
    TeamStore::load_by_name(team_name, &app.cwd_path).ok()
}

/// Claimed and done task counts per teammate, from the team's task list.
fn task_counts(store: Option<&TeamStore>) -> TaskCounts {
    let mut counts = TaskCounts::new();
    let list = store
        .and_then(|store| TaskStore::open(&store.dir).ok())
        .and_then(|tasks| tasks.read().ok());
    for task in list.iter().flat_map(|list| list.tasks.iter()) {
        let Some(agent) = task.assigned_to.as_ref() else {
            continue;
        };
        let entry = counts.entry(agent.clone()).or_default();
        entry.0 += 1;
        if task.status == TaskStatus::Completed {
            entry.1 += 1;
        }
    }
    counts
}

/// The parked plans by teammate agent id.
fn plans(store: Option<&TeamStore>) -> HashMap<String, PendingPlan> {
    store
        .map(|store| {
            pending_plans(store)
                .into_iter()
                .map(|plan| (plan.agent_id.clone(), plan))
                .collect()
        })
        .unwrap_or_default()
}

/// Elapsed time since the lead's session was created, or `-`.
fn lead_elapsed(app: &App, lead_session: &str) -> String {
    app.storage
        .get_session(lead_session)
        .ok()
        .flatten()
        .and_then(|session| chrono::DateTime::parse_from_rfc3339(&session.created_at).ok())
        .map(|created| elapsed(created.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|| "-".to_string())
}

/// Elapsed time since `created_at`, compact.
fn elapsed(created_at: chrono::DateTime<chrono::Utc>) -> String {
    format_duration_compact((chrono::Utc::now() - created_at).num_seconds())
}

/// The model column: the override, or `(inherited)` when it matches the
/// global model or none is set.
fn model_column(member: &TeamMember, global: Option<&str>, palette: &Palette) -> (String, Color32) {
    let Some(model) = member.model_override.as_ref() else {
        return ("(inherited)".to_string(), palette.muted);
    };
    let rendered = format!("{}/{}", model.provider_id, model.model_id);
    if global == Some(rendered.as_str()) {
        return ("(inherited)".to_string(), palette.muted);
    }
    (
        truncate_chars_no_ellipsis(&rendered, MODEL_W),
        palette.color(ThemeColor::Categorical(4)),
    )
}

/// The parked-member hint, while any member is blocked or suspended.
fn parked_hint(members: &[TeamMember]) -> Option<String> {
    let names = |status: MemberStatus| -> Vec<&str> {
        members
            .iter()
            .filter(|m| m.status == status)
            .map(|m| m.name.as_str())
            .collect()
    };
    let (blocked, suspended) = (names(MemberStatus::Blocked), names(MemberStatus::Suspended));
    let mut parts = Vec::new();
    if !blocked.is_empty() {
        parts.push(format!("Del resumes blocked {}", blocked.join(", ")));
    }
    if !suspended.is_empty() {
        parts.push(format!("Space resumes suspended {}", suspended.join(", ")));
    }
    (!parts.is_empty()).then(|| format!("({})", parts.join("; ")))
}

/// The cells after the model column: elapsed, steps, claimed, done, sent,
/// received.
#[allow(clippy::too_many_arguments)]
fn counters(
    line: Line,
    elapsed: &str,
    steps: &str,
    claimed: Option<usize>,
    done: Option<usize>,
    sent: Option<u32>,
    recv: Option<u32>,
    palette: &Palette,
) -> Line {
    let hint = palette.muted;
    let lit = |on: bool, color: ThemeColor| if on { palette.color(color) } else { hint };
    let claimed_text = claimed.map_or("-".to_string(), |n| n.to_string());
    let done_text = done.map_or("-".to_string(), |n| n.to_string());
    let sent_text = sent.map_or("-".to_string(), count_cell);
    let recv_text = recv.map_or("-".to_string(), count_cell);
    line.push(format!(" {elapsed:>ELAPSED_W$}"), hint)
        .push(format!(" {steps:>STEPS_W$}"), hint)
        .push(
            format!(" {claimed_text:>CLAIMED_W$}"),
            lit(claimed.unwrap_or(0) > 0, ThemeColor::Warning),
        )
        .push(
            format!(" {done_text:>DONE_W$}"),
            lit(done.unwrap_or(0) > 0, ThemeColor::Success),
        )
        .push(
            format!(" {sent_text:>SENT_W$}"),
            lit(sent.unwrap_or(0) > 0, ThemeColor::Info),
        )
        .push(
            format!(" {recv_text:>RECV_W$}"),
            lit(recv.unwrap_or(0) > 0, ThemeColor::Info),
        )
}

/// The team title, as the TUI's table border names it.
fn title(section: &TeamSection) -> String {
    let count = section.members.len();
    let name = if section.name.is_empty() {
        "team"
    } else {
        section.name.as_str()
    };
    format!(
        "Team: {name}  (lead + {count} teammate{})",
        if count == 1 { "" } else { "s" }
    )
}

/// Every row of the pane with its `(team name, key)`: one section per team.
/// The key is empty for a row the cursor cannot select, `"lead"` for a
/// lead, else the teammate's agent id.
///
/// # Arguments
///
/// * `app` - the App whose teams are listed.
/// * `palette` - the colours to draw with.
pub fn team_rows(app: &App, palette: &Palette) -> Vec<(Line, (String, String))> {
    let global = app.active_model_ref_string();
    app.team_sections()
        .iter()
        .flat_map(|section| {
            section_rows(app, palette, section, global.as_deref())
                .into_iter()
                .map(|(line, key)| (line, (section.name.clone(), key)))
        })
        .collect()
}

/// One team's rows: its title, the column header, the lead, the teammates
/// and the hints, each with its key (see [`team_rows`]).
fn section_rows(
    app: &App,
    palette: &Palette,
    section: &TeamSection,
    global: Option<&str>,
) -> Vec<(Line, String)> {
    let hint = palette.muted;
    let store = team_store(app, &section.name);
    let counts = task_counts(store.as_ref());
    let parked = plans(store.as_ref());
    let lead_session = section.lead_session_id.clone();
    let is_active = app
        .session
        .active_team
        .as_ref()
        .is_some_and(|team| team.name == section.name);
    let mut out = vec![(
        Line::new().push(title(section), palette.color(ThemeColor::Accent)),
        String::new(),
    )];
    out.push((
        Line::new().push(
            format!(
                "{:MARKER_W$}{:<ID_W$} {:<LABEL_W$} {:<AGENT_TYPE_W$} {:<STATUS_W$} \
                 {:<MODEL_W$} {:>ELAPSED_W$} {:>STEPS_W$} {:>CLAIMED_W$} {:>DONE_W$} \
                 {:>SENT_W$} {:>RECV_W$}  {:<STOP_W$}  del",
                "",
                "id",
                "name",
                "agent type",
                "status",
                "model",
                "elapsed",
                "steps",
                "claimed",
                "done",
                "sent",
                "recv",
                "space"
            ),
            hint,
        ),
        String::new(),
    ));
    let green = palette.color(ThemeColor::Success);
    let name = if is_active {
        app.session.agent_name.as_str()
    } else {
        "lead"
    };
    let (lead_sent, lead_recv) = if is_active {
        app.session
            .team_message_counts
            .get(name)
            .or_else(|| app.session.team_message_counts.get("lead"))
            .copied()
            .unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    let lead = Line::new()
        .push(format!("{:<MARKER_W$}", "* "), green)
        .push(format!("{:<ID_W$} ", short_suffix(&lead_session, 8)), green)
        .push(
            format!("{:<LABEL_W$} ", truncate_chars_no_ellipsis(name, LABEL_W)),
            green,
        )
        .push(
            format!(
                "{:<AGENT_TYPE_W$} ",
                truncate_chars_no_ellipsis(name, AGENT_TYPE_W)
            ),
            green,
        )
        .push(format!("{:<STATUS_W$} ", "active"), green)
        .push(format!("{:<MODEL_W$}", "lead"), hint);
    let lead_steps = app.event_bus.current_tool_calls(&lead_session).to_string();
    out.push((
        counters(
            lead,
            &lead_elapsed(app, &lead_session),
            &lead_steps,
            None,
            None,
            Some(lead_sent),
            Some(lead_recv),
            palette,
        )
        .hover(name),
        "lead".to_string(),
    ));
    let members = &section.members;
    for (index, member) in members.iter().enumerate() {
        let short = short_suffix(&member.agent_id, 8);
        let focused = app
            .session
            .focused_teammate
            .as_ref()
            .is_some_and(|(team, id)| *team == section.name && *id == member.agent_id);
        let connector = if index + 1 == members.len() {
            "`- "
        } else {
            "|- "
        };
        let focus = if focused { ">" } else { " " };
        let (marker_color, name_color) = if focused {
            let yellow = palette.color(ThemeColor::Warning);
            (yellow, yellow)
        } else {
            (palette.color(ThemeColor::Accent), palette.text)
        };
        let (model, model_color) = model_column(member, global, palette);
        let label = truncate_chars_no_ellipsis(&format!("{}-{short}", member.name), LABEL_W);
        let steps = member
            .session_id
            .as_deref()
            .map_or(0, |sid| app.event_bus.current_tool_calls(sid));
        let (claimed, done) = counts.get(&member.agent_id).copied().unwrap_or((0, 0));
        // A `lead` agent id never has its messages counted (the event
        // handlers skip that key), so its cells stay `-`, as in the TUI.
        let (sent, recv) = if member.agent_id == "lead" {
            (None, None)
        } else {
            // The counters follow the session's active team only.
            let (sent, recv) = is_active
                .then(|| app.session.team_message_counts.get(&member.agent_id))
                .flatten()
                .copied()
                .unwrap_or((0, 0));
            (Some(sent), Some(recv))
        };
        let line = Line::new()
            .push(format!("{focus}{connector}"), marker_color)
            .push(format!("{short:<ID_W$} "), palette.color(ThemeColor::Info))
            .push(format!("{label:<LABEL_W$} "), name_color)
            .push(
                format!(
                    "{:<AGENT_TYPE_W$} ",
                    truncate_chars_no_ellipsis(&member.agent_type, AGENT_TYPE_W)
                ),
                palette.text,
            )
            .push(
                format!("{:<STATUS_W$} ", member.status.display_label()),
                status_color(&member.status, palette),
            )
            .push(format!("{model:<MODEL_W$}"), model_color);
        // The columns cut names to fixed widths; hovering shows them whole.
        let full_model = member.model_override.as_ref().map_or_else(
            || "(inherited)".to_string(),
            |m| format!("{}/{}", m.provider_id, m.model_id),
        );
        let mut line = counters(
            line,
            &elapsed(member.created_at),
            &steps.to_string(),
            Some(claimed),
            Some(done),
            sent,
            recv,
            palette,
        )
        .hover(format!(
            "{}-{short}\nType: {}\nModel: {full_model}",
            member.name, member.agent_type
        ));
        if !is_terminal(&member.status) {
            // Key hints, muted plain words rather than bracketed
            // button-like text: the keys act, a click only selects the row.
            // A plan waiting for approval is answered with a slash command
            // (its own line below says so); Space and Delete act as on any
            // live member.
            let left = if member.status == MemberStatus::Suspended {
                "resume"
            } else {
                "stop"
            };
            let right = "shut down";
            line = line
                .push(format!("  {left:<STOP_W$}  "), palette.muted)
                .push(right, palette.muted);
        }
        out.push((line, member.agent_id.clone()));
        if member.status == MemberStatus::PlanPending {
            let excerpt = parked
                .get(&member.agent_id)
                .and_then(|plan| plan.plan.as_deref())
                .and_then(|text| text.lines().map(str::trim).find(|l| !l.is_empty()))
                .unwrap_or("(plan request not found)")
                .to_string();
            out.push((
                Line::new()
                    .push(format!("{:MARKER_W$}", ""), hint)
                    .push(
                        "plan awaiting approval: ",
                        palette.color(ThemeColor::Categorical(4)),
                    )
                    .push(excerpt, palette.text),
                String::new(),
            ));
        }
    }
    if let Some(parked_hint) = parked_hint(members) {
        out.push((
            Line::new()
                .push(
                    "  parked - Del resumes a blocked member, Space resumes a suspended one",
                    palette.color(ThemeColor::Warning),
                )
                .push(format!(" {parked_hint}"), hint),
            String::new(),
        ));
    }
    if members.is_empty() {
        out.push((
            Line::new().push(format!("  {EMPTY_HINT}"), hint),
            String::new(),
        ));
    }
    out
}

/// Draw the Teams panel into `ui`, publish the row keys app-core's cursor
/// actions resolve against, and move the cursor to a clicked row.
///
/// # Arguments
///
/// * `ui` - the Teams pane's body.
/// * `app` - the App whose team is drawn; a click selects through
///   `App::teams_cursor_from_click`.
/// * `palette` - the colours to draw with.
pub fn teams_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    app.refresh_team_member_session_ids();
    let rows = team_rows(app, palette);
    let keys: Vec<(String, String)> = rows.iter().map(|(_, key)| key.clone()).collect();
    // The cursor is an ordinal over the teammate rows only (titles, plan
    // lines and hints are not rows of their own), across every team.
    app.teams_teammate_agent_ids = app
        .team_sections()
        .into_iter()
        .flat_map(|section| {
            let team = section.name;
            section
                .members
                .into_iter()
                .map(move |member| (team.clone(), member.agent_id))
        })
        .collect();
    app.teams_row_agent_ids = keys;
    let cursor_row = app
        .teams_cursor_teammate
        .and_then(|cursor| app.teams_teammate_agent_ids.get(cursor))
        .and_then(|id| app.teams_row_agent_ids.iter().position(|key| key == id));
    // The legend wraps in a narrow pane; the list leaves room for all of it.
    let legend_height = legend_height(ui, LEGEND);
    let mut drawn = None;
    ui.vertical(|ui| {
        let list_height = (ui.available_height() - legend_height).max(0.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), list_height), |ui| {
            drawn = Some(show_lines(
                ui,
                "teams_rows",
                rows.len(),
                |index| rows.get(index).map(|(l, _)| l.clone()).unwrap_or_default(),
                cursor_row,
                |index| rows.get(index).is_some_and(|(_, (_, key))| !key.is_empty()),
                Follow::None,
            ));
        });
        ui.weak(LEGEND);
    });
    let Some(drawn) = drawn else {
        return;
    };
    label_panel(ui, drawn.rect, "teams_table", Role::Table, TEAMS_LABEL);
    let row_height = ui
        .ctx()
        .fonts_mut(|f| f.row_height(&egui::FontId::monospace(crate::styled_text::FONT_SIZE)));
    app.teams_rows_area = Area {
        height: rows_in_view(drawn.rect.height(), row_height),
        ..Area::default()
    };
    if let Some(index) = drawn.clicked {
        app.teams_cursor_from_click(index);
        ui.ctx().request_repaint();
    }
}
