//! The Context panel (the Context pane): what fills the model's context.
//!
//! `Alt+C` (app-core's `ToggleContextPanel`) sets `App::show_context_panel`
//! and hides the other side panels; the dock then shows this pane. Each
//! frame it reads `App::context_effective_snapshot` (the background-refreshed
//! cache, so a frame never does disk or database work) and draws the lines
//! `codemason_app_core::context_panel::context_panel_lines` builds: the
//! context window, the tokens sent on the last turn, every partition with
//! its share of the window, the message count, the total, the free headroom
//! and the stream I/O counters, as the TUI's context panel draws them.
//!
//! Drawing never touches the conversation (FR-016). The plain text of the
//! lines is cached on `App::context_content_lines`, the copy source of the
//! panel's text selection, as the TUI cached it. The list follows app-core's
//! `context_scroll_offset` (lines from the top).
//!
//! Dependencies: `codemason-app-core` and `egui`.

use codemason_app_core::app::App;
use codemason_app_core::context_panel::context_panel_lines;
use codemason_app_core::styled::StyledLine;
use egui::Role;

use super::rows::{Follow, Line, label_panel, show_lines};
use crate::palette::Palette;

/// Accessible label of the Context panel's list.
pub const CONTEXT_LABEL: &str = "Context";

/// Cells in each percentage bar (the TUI's widest bar).
const BAR_WIDTH: usize = 10;

/// `line` in `palette`'s colours; an uncoloured span takes the text colour.
fn to_line(line: &StyledLine, palette: &Palette) -> Line {
    line.spans.iter().fold(Line::new(), |drawn, span| {
        let color = span.style.fg.map_or(palette.text, |c| palette.color(c));
        drawn.push(span.text.clone(), color)
    })
}

/// Draw the Context panel into `ui`.
///
/// # Arguments
///
/// * `ui` - the Context pane's body.
/// * `app` - the App whose context snapshot is drawn; its
///   `context_content_lines` copy cache is refreshed.
/// * `palette` - the colours to draw with.
pub fn context_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    let snapshot = app.context_effective_snapshot();
    let styled = context_panel_lines(
        &snapshot,
        app.session.stream_out_bytes,
        app.session.stream_in_bytes,
        BAR_WIDTH,
    );
    app.context_content_lines = styled.iter().map(StyledLine::text).collect();
    let lines: Vec<Line> = styled.iter().map(|l| to_line(l, palette)).collect();
    let drawn = show_lines(
        ui,
        "context_rows",
        lines.len(),
        |index| lines.get(index).cloned().unwrap_or_default(),
        None,
        |_| false,
        Follow::FromTop(app.context_scroll_offset),
    );
    label_panel(ui, drawn.rect, "context_list", Role::List, CONTEXT_LABEL);
}
