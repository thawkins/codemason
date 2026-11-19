//! The log panel (the Log pane): the tracing records and App log entries in
//! `App::log_entries`.
//!
//! The shell drains the tracing log channel into the App each
//! `Shell::logic` pass (redacted, as in the TUI); this panel only draws
//! `App::log_entries`, one row per entry, as the TUI's log panel formats
//! them: `HH:MM:SS LEVEL [agent] message`, with the level a fixed-width word
//! coloured from the [`Palette`] (`info`, `tool`, `warn`, `error`, and
//! `compact` for compaction start/end messages), so every message starts in
//! the same column. Characters the font cannot draw are drawn as ASCII
//! stand-ins ([`drawable`]). With no entries it says what the panel will
//! hold ([`EMPTY_TEXT`], [`EMPTY_HINT`]) in the same font, filling the pane as a list does.
//!
//! The list is virtualised with `ScrollArea::show_rows` (a multi-line
//! message is drawn on one row), so only the visible entries are laid out,
//! and it sticks to the bottom so it follows the tail until the user scrolls
//! up.
//!
//! Dependencies: `codemason-app-core` (log entries), `egui` and
//! [`crate::palette`].

use codemason_app_core::app::{App, LogEntry, LogLevel};
use codemason_app_core::styled::ThemeColor;
use egui::text::LayoutJob;
use egui::{Color32, FontId, Role, ScrollArea, TextFormat, WidgetInfo};

use super::rows::{Follow, Line, show_lines};
use crate::palette::Palette;
use crate::styled_text::{FONT_SIZE, drawable};

/// Accessible label of the log list.
pub const LOG_LABEL: &str = "Log";

/// Shown while there are no entries.
pub const EMPTY_TEXT: &str = "No log entries yet";

/// The line under [`EMPTY_TEXT`]: what will fill the panel.
pub const EMPTY_HINT: &str = "Tool calls and errors appear here.";

/// Width of the level column, the longest word (`compact`) plus a space.
const LEVEL_W: usize = 8;

/// The level tag and its colour, as the TUI's log panel draws them.
fn level_tag(entry: &LogEntry, palette: &Palette) -> (&'static str, Color32) {
    let lower = entry.message.to_lowercase();
    let compaction = lower.contains("compaction")
        && (lower.contains("started")
            || lower.contains("completed")
            || lower.contains("triggered"));
    if compaction {
        return ("compact", palette.color(ThemeColor::Success));
    }
    match entry.level {
        LogLevel::Info => ("info", palette.color(ThemeColor::Accent)),
        LogLevel::Tool => ("tool", palette.color(ThemeColor::Info)),
        LogLevel::Warn => ("warn", palette.color(ThemeColor::Warning)),
        LogLevel::Error => ("error", palette.color(ThemeColor::Error)),
    }
}

/// One entry laid out as a single row of coloured sections.
fn entry_job(ctx: &egui::Context, entry: &LogEntry, palette: &Palette) -> LayoutJob {
    let font = FontId::monospace(FONT_SIZE);
    let format = |color: Color32| TextFormat::simple(font.clone(), color);
    let mut job = LayoutJob::default();
    job.append(
        &format!("{} ", entry.timestamp.format("%H:%M:%S")),
        0.0,
        format(palette.muted),
    );
    let (tag, colour) = level_tag(entry, palette);
    job.append(&format!("{tag:<LEVEL_W$}"), 0.0, format(colour));
    if let Some(agent) = &entry.agent_id {
        job.append(
            &format!("[{agent}] "),
            0.0,
            format(palette.color(ThemeColor::Categorical(4))),
        );
    }
    let message: String = entry
        .message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    job.append(&drawable(ctx, &font, &message), 0.0, format(palette.text));
    job
}

/// Draw the log panel into `ui`.
///
/// # Arguments
///
/// * `ui` - the Log pane's body.
/// * `app` - the App whose `log_entries` are drawn.
/// * `palette` - the colours to draw with.
pub fn log_ui(ui: &mut egui::Ui, app: &App, palette: &Palette) {
    let rect = if app.log_entries.is_empty() {
        show_lines(
            ui,
            "log_rows",
            2,
            |index| {
                if index == 0 {
                    Line::new().push(EMPTY_TEXT, palette.text)
                } else {
                    Line::new().push(EMPTY_HINT, palette.muted)
                }
            },
            None,
            |_| false,
            Follow::None,
        )
        .rect
    } else {
        let row_height = ui
            .ctx()
            .fonts_mut(|fonts| fonts.row_height(&FontId::monospace(FONT_SIZE)));
        ScrollArea::both()
            .id_salt("log_rows")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show_rows(ui, row_height, app.log_entries.len(), |ui, range| {
                for entry in app.log_entries.get(range).unwrap_or_default() {
                    ui.add(egui::Label::new(entry_job(ui.ctx(), entry, palette)).extend());
                }
            })
            .inner_rect
    };
    let list = ui.interact(rect, ui.scope_id().with("log_list"), egui::Sense::hover());
    list.widget_info(|| WidgetInfo::labeled(Role::Log, true, LOG_LABEL));
}
