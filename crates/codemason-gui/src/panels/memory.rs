//! The Memory panel (the Memory pane): the project's structured memories.
//!
//! app-core keeps the rows (`memory_cache_entries`, `memory_cache_count` and
//! `memory_cache_tags`, refreshed by `App::refresh_memory_cache` only when a
//! memory change marked them dirty) and the cursor (`memory_cursor`). This
//! panel draws what the TUI's memory panel draws: the count and total size,
//! then the memories grouped under a capitalised category heading, each as
//! `#id confidence bytes preview` with its tags and source on indented lines
//! below, and the key hint. With no memories it says what the panel will
//! hold ([`EMPTY_TEXT`], [`EMPTY_HINT`]) instead.
//!
//! # Keys and clicks
//!
//! The keys are the TUI's, run by app-core while the panel is shown (the
//! router sends them to the App): Up/Down move the cursor, Enter on an empty
//! prompt opens the memory (`App::memory_view`), Delete on an empty prompt
//! asks to delete it (`App::pending_memory_delete`, drawn by
//! `crate::dialogs::confirm`), and `/memory clear` asks before clearing
//! every memory. A click on a memory's row moves the cursor there
//! (`App::select_memory_row`).
//!
//! The lines are virtualised ([`show_lines`]); the cursor row is kept in
//! view, and the list follows app-core's `memory_scroll_offset`
//! (Ctrl+PageUp/PageDown, lines from the top).
//!
//! Dependencies: `codemason-app-core`, `codemason-types` (byte truncation)
//! and `egui`.

use std::collections::BTreeMap;

use codemason_app_core::app::App;
use codemason_app_core::styled::ThemeColor;
use codemason_types::strutil::truncate_bytes;
use egui::Role;

use super::rows::{Follow, Line, label_panel, show_lines};
use crate::palette::Palette;

/// Accessible label of the memory list.
pub const MEMORY_LABEL: &str = "Memory";

/// Shown while the project has no memories.
pub const EMPTY_TEXT: &str = "No memories for this project yet";

/// The line under [`EMPTY_TEXT`]: what will fill the panel.
pub const EMPTY_HINT: &str = "The agent saves what it learns here.";

/// The key hint under the list, worded as the Agents window's legend.
pub const KEY_HINT: &str = "Up/Down select  Enter open  Del delete";

/// `category` with its first letter in upper case (`fact` -> `Fact`).
fn heading(category: &str) -> String {
    let mut chars = category.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The panel's lines, with the memory row (cursor index) each line selects.
fn lines(app: &App, palette: &Palette) -> Vec<(Line, Option<usize>)> {
    let entries = &app.memory_cache_entries;
    if entries.is_empty() {
        return vec![
            (Line::new().push(EMPTY_TEXT, palette.text), None),
            (Line::new().push(EMPTY_HINT, palette.muted), None),
        ];
    }
    let bytes: usize = entries.iter().map(|r| r.content.len()).sum();
    let noun = if app.memory_cache_count == 1 {
        "memory"
    } else {
        "memories"
    };
    let mut lines = vec![
        (
            Line::new()
                .push(app.memory_cache_count.to_string(), palette.text)
                .push(format!(" {noun}, {bytes} bytes"), palette.muted),
            None,
        ),
        (Line::new(), None),
    ];
    let magenta = palette.color(ThemeColor::Categorical(4));
    // The cache is sorted by category then id, so the cursor index of a row
    // is its position in the cache.
    let mut by_category: BTreeMap<&str, Vec<(usize, &codemason_storage::MemoryRow)>> =
        BTreeMap::new();
    for (index, row) in entries.iter().enumerate() {
        by_category
            .entry(row.category.as_str())
            .or_default()
            .push((index, row));
    }
    for (category, rows) in &by_category {
        lines.push((
            Line::new().push(format!("{} ({})", heading(category), rows.len()), magenta),
            None,
        ));
        for (index, row) in rows {
            lines.push((
                Line::new()
                    .push(format!("#{}", row.id), palette.muted)
                    .push(" ", palette.text)
                    .push(
                        format!("{:.2}", row.confidence),
                        palette.color(ThemeColor::Warning),
                    )
                    .push(" ", palette.text)
                    .push(format!("{}b", row.content.len()), palette.muted)
                    .push(" ", palette.text)
                    .push(truncate_bytes(&row.content, 80), palette.text),
                Some(*index),
            ));
            if let Some(tags) = app.memory_cache_tags.get(&row.id).filter(|t| !t.is_empty()) {
                lines.push((
                    Line::new()
                        .push("    tags: ", palette.muted)
                        .push(tags.join(", "), palette.color(ThemeColor::Info)),
                    None,
                ));
            }
            if !row.source.is_empty() {
                lines.push((
                    Line::new()
                        .push("    source: ", palette.muted)
                        .push(row.source.clone(), palette.text),
                    None,
                ));
            }
        }
        lines.push((Line::new(), None));
    }
    lines.push((Line::new().push(KEY_HINT, palette.muted), None));
    lines
}

/// Draw the Memory panel into `ui`, refreshing app-core's memory cache first
/// when it is dirty, and move the cursor to a clicked row.
///
/// # Arguments
///
/// * `ui` - the Memory pane's body.
/// * `app` - the App whose memories are drawn; a click selects through
///   `App::select_memory_row`.
/// * `palette` - the colours to draw with.
pub fn memory_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    app.refresh_memory_cache();
    let lines = lines(app, palette);
    let cursor_line = (!app.memory_cache_entries.is_empty())
        .then(|| {
            lines
                .iter()
                .position(|(_, row)| *row == Some(app.memory_cursor))
        })
        .flatten();
    let drawn = show_lines(
        ui,
        "memory_rows",
        lines.len(),
        |index| lines.get(index).map(|(l, _)| l.clone()).unwrap_or_default(),
        cursor_line,
        |index| lines.get(index).is_some_and(|(_, row)| row.is_some()),
        Follow::FromTop(app.memory_scroll_offset),
    );
    label_panel(ui, drawn.rect, "memory_list", Role::List, MEMORY_LABEL);
    if let Some(row) = drawn
        .clicked
        .and_then(|index| lines.get(index))
        .and_then(|(_, row)| *row)
    {
        app.select_memory_row(row);
        ui.ctx().request_repaint();
    }
}
