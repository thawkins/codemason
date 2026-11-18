//! Diffs: the inline diff under an edit in the transcript, and the Diff
//! pane.
//!
//! [`inline_diff_ui`] draws a [`FileDiff`] in the transcript: the path with
//! its added and removed counts and an [`OPEN_DIFF`] button, then the first
//! hunk, and how many hunks follow. The button opens the Diff pane
//! (`App::diff_pane`, [`diff_pane_ui`]) on that file.
//!
//! The Diff pane shows one file at a time, unified or split (old on the
//! left, new on the right), with buttons for the previous and next hunk and
//! file, the layout and the reviewed mark. A file marked reviewed is drawn
//! collapsed to its header. While the pane holds the keyboard focus (a click
//! in it), `]` and `[` move between hunks, `n` and `p` between files, `v`
//! switches the layout and `m` toggles the reviewed mark
//! ([`take_pane_keys`]); everywhere else those keys type as usual.
//!
//! Lines are drawn in the theme's `diff.*` colours: added and removed rows
//! on their backgrounds, line numbers and hunk headers in
//! `diff.line_number`.
//!
//! Dependencies: `egui`, `codemason-app-core` (the diff model and pane
//! state), [`crate::palette`] and [`crate::metrics`].

use codemason_app_core::app::App;
use codemason_app_core::diff_view::{DiffHunk, DiffLayout, DiffLine, DiffLineKind, FileDiff};
use egui::{Id, RichText, Role, Sense, WidgetInfo};

use crate::metrics::{radius, space, text_size};
use crate::palette::Palette;

/// The button that opens a transcript diff in the Diff pane.
pub const OPEN_DIFF: &str = "Open diff";

/// Accessible label of the Diff pane's content.
pub const DIFF_PANE_LABEL: &str = "Diff";

/// The Diff pane's buttons.
pub const PREV_HUNK: &str = "Previous hunk";
/// See [`PREV_HUNK`].
pub const NEXT_HUNK: &str = "Next hunk";
/// See [`PREV_HUNK`].
pub const PREV_FILE: &str = "Previous file";
/// See [`PREV_HUNK`].
pub const NEXT_FILE: &str = "Next file";
/// See [`PREV_HUNK`].
pub const SPLIT: &str = "Split";
/// See [`PREV_HUNK`].
pub const MARK_REVIEWED: &str = "Reviewed";

/// What a file marked reviewed shows instead of its hunks.
pub const REVIEWED_NOTE: &str = "Marked reviewed; press m or Reviewed to show it again";

/// Accessible labels of the split layout's two columns.
pub const OLD_COLUMN: &str = "Old";
/// See [`OLD_COLUMN`].
pub const NEW_COLUMN: &str = "New";

/// Most lines of the first hunk an inline diff shows.
const INLINE_MAX_LINES: usize = 40;

/// The id that holds the keyboard focus while the Diff pane does.
#[must_use]
pub fn pane_focus_id() -> Id {
    Id::unique("codemason_diff_pane_focus")
}

/// A click-only button: it never holds the keyboard focus, so it cannot
/// take an Enter meant for the prompt.
fn button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).size(text_size::SMALL)).sense(Sense::CLICK)
}

/// What a whole-file write shows instead of a removed count: the file's
/// earlier contents are not known.
pub const WHOLE_FILE_NOTE: &str = "replaces the whole file; earlier contents not shown";

/// `+a -r` counts, in the diff colours; for a whole-file write, `+a` and
/// [`WHOLE_FILE_NOTE`].
fn counts(ui: &mut egui::Ui, diff: &FileDiff, palette: &Palette) {
    ui.label(RichText::new(format!("+{}", diff.added)).color(palette.diff_added));
    if diff.whole_file {
        ui.label(RichText::new(WHOLE_FILE_NOTE).color(palette.muted));
    } else {
        ui.label(RichText::new(format!("-{}", diff.removed)).color(palette.diff_removed));
    }
}

/// A line number, blank when the line has none on that side.
fn number(no: Option<u32>) -> String {
    no.map_or_else(|| " ".repeat(4), |n| format!("{n:>4}"))
}

/// One diff row: its line numbers, its mark and text, on its background.
/// `numbers` picks which line numbers are shown (old, new).
fn line_ui(ui: &mut egui::Ui, line: &DiffLine, palette: &Palette, numbers: (bool, bool)) {
    let (mark, fg, bg) = match line.kind {
        DiffLineKind::Context => (' ', palette.diff_context, None),
        DiffLineKind::Added => ('+', palette.diff_added, Some(palette.diff_added_bg)),
        DiffLineKind::Removed => ('-', palette.diff_removed, Some(palette.diff_removed_bg)),
    };
    let mut frame = egui::Frame::NONE;
    if let Some(bg) = bg {
        frame = frame.fill(bg);
    }
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::XS;
            let mut gutter = String::new();
            if numbers.0 {
                gutter.push_str(&number(line.old_no));
            }
            if numbers.1 {
                if !gutter.is_empty() {
                    gutter.push(' ');
                }
                gutter.push_str(&number(line.new_no));
            }
            ui.label(
                RichText::new(gutter)
                    .monospace()
                    .color(palette.diff_line_number),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{mark} {}", line.text))
                        .monospace()
                        .color(fg),
                )
                .extend(),
            );
        });
    });
}

/// A hunk, unified: its header, then every line.
fn unified_hunk_ui(ui: &mut egui::Ui, hunk: &DiffHunk, palette: &Palette, max_lines: usize) {
    ui.label(
        RichText::new(&hunk.header)
            .monospace()
            .color(palette.diff_line_number),
    );
    for line in hunk.lines.iter().take(max_lines) {
        line_ui(ui, line, palette, (true, true));
    }
    if hunk.lines.len() > max_lines {
        ui.label(
            RichText::new(format!("... {} more lines", hunk.lines.len() - max_lines))
                .color(palette.subtle),
        );
    }
}

/// The rows of a hunk side by side: each run of removed lines next to the
/// added lines that follow it, context on both sides.
fn split_rows(hunk: &DiffHunk) -> Vec<(Option<&DiffLine>, Option<&DiffLine>)> {
    let mut rows = Vec::new();
    let mut lines = hunk.lines.iter().peekable();
    while let Some(line) = lines.next() {
        match line.kind {
            DiffLineKind::Context => rows.push((Some(line), Some(line))),
            DiffLineKind::Removed | DiffLineKind::Added => {
                let mut removed = Vec::new();
                let mut added = Vec::new();
                let mut current = Some(line);
                while let Some(l) = current {
                    match l.kind {
                        DiffLineKind::Removed => removed.push(l),
                        DiffLineKind::Added => added.push(l),
                        DiffLineKind::Context => break,
                    }
                    current = lines.next_if(|next| next.kind != DiffLineKind::Context);
                }
                let height = removed.len().max(added.len());
                for i in 0..height {
                    rows.push((removed.get(i).copied(), added.get(i).copied()));
                }
            }
        }
    }
    rows
}

/// A hunk, split: old on the left, new on the right.
fn split_hunk_ui(ui: &mut egui::Ui, hunk: &DiffHunk, palette: &Palette) {
    ui.label(
        RichText::new(&hunk.header)
            .monospace()
            .color(palette.diff_line_number),
    );
    let rows = split_rows(hunk);
    ui.columns(2, |columns| {
        for (index, (title, side)) in [(OLD_COLUMN, 0usize), (NEW_COLUMN, 1)]
            .into_iter()
            .enumerate()
        {
            let column = &mut columns[index];
            let inner = column.vertical(|ui| {
                for row in &rows {
                    let line = if side == 0 { row.0 } else { row.1 };
                    match line {
                        Some(line) => line_ui(ui, line, palette, (side == 0, side == 1)),
                        None => {
                            ui.label(RichText::new(" ").monospace());
                        }
                    }
                }
            });
            inner
                .response
                .widget_info(|| WidgetInfo::labeled(Role::Group, true, title));
        }
    });
}

/// Draw `diff` inline in the transcript.
///
/// # Returns
///
/// Whether [`OPEN_DIFF`] was clicked.
pub fn inline_diff_ui(ui: &mut egui::Ui, diff: &FileDiff, palette: &Palette) -> bool {
    let mut open = false;
    egui::Frame::NONE
        .fill(palette.raised1)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(radius::SM)
        .inner_margin(space::SM)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&diff.path).monospace().color(palette.text));
                counts(ui, diff, palette);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    open = ui.add(button(OPEN_DIFF)).clicked();
                });
            });
            if let Some(hunk) = diff.hunks.first() {
                egui::ScrollArea::horizontal()
                    .id_salt(("inline_diff", &diff.path))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        unified_hunk_ui(ui, hunk, palette, INLINE_MAX_LINES)
                    });
            }
            let more = diff.hunks.len().saturating_sub(1);
            if more > 0 {
                let text = if more == 1 {
                    "1 more hunk".to_string()
                } else {
                    format!("{more} more hunks")
                };
                ui.label(RichText::new(text).color(palette.subtle));
            }
        });
    open
}

/// Draw all of `diff`, every hunk unified, with its path and counts: for a
/// place that cannot open the Diff pane (the permission dialog, a modal).
pub fn full_diff_ui(ui: &mut egui::Ui, diff: &FileDiff, palette: &Palette) {
    egui::Frame::NONE
        .fill(palette.raised1)
        .stroke(egui::Stroke::new(1.0, palette.border))
        .corner_radius(radius::SM)
        .inner_margin(space::SM)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&diff.path).monospace().color(palette.text));
                counts(ui, diff, palette);
            });
            for hunk in &diff.hunks {
                unified_hunk_ui(ui, hunk, palette, usize::MAX);
            }
            if diff.truncated {
                ui.label(RichText::new(TRUNCATED_NOTE).color(palette.warning));
            }
        });
}

/// What a diff cut short at the line limit says.
pub const TRUNCATED_NOTE: &str = "The diff is cut short; it is too long to show whole.";

/// What a Diff pane key does to the pane state.
type PaneAction = fn(&mut codemason_app_core::diff_view::DiffPaneState);

/// The action of a Diff pane key, if `key` is one.
fn pane_key_action(key: egui::Key) -> Option<PaneAction> {
    use codemason_app_core::diff_view::DiffPaneState;
    Some(match key {
        egui::Key::CloseBracket => DiffPaneState::next_hunk,
        egui::Key::OpenBracket => DiffPaneState::prev_hunk,
        egui::Key::N => DiffPaneState::next_file,
        egui::Key::P => DiffPaneState::prev_file,
        egui::Key::V => DiffPaneState::toggle_layout,
        egui::Key::M => DiffPaneState::toggle_reviewed,
        // Every other key is not the pane's.
        _ => return None,
    })
}

/// Apply the Diff pane's keys while it holds the keyboard focus: they are
/// taken out of this frame's input, so they neither type nor reach the
/// App. Call before the keys are routed.
pub fn take_pane_keys(ctx: &egui::Context, app: &mut App) {
    if !app.diff_pane.shown || !ctx.memory(|m| m.has_focus(pane_focus_id())) {
        return;
    }
    let mut actions = Vec::new();
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let action = match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.is_none() => pane_key_action(*key),
                egui::Event::Text(text) => {
                    return !matches!(text.as_str(), "]" | "[" | "n" | "p" | "v" | "m");
                }
                _ => None,
            };
            if let Some(action) = action {
                actions.push(action);
                return false;
            }
            true
        });
    });
    for action in actions {
        action(&mut app.diff_pane);
    }
}

/// Draw the Diff pane.
pub fn diff_pane_ui(ui: &mut egui::Ui, app: &mut App, palette: &Palette) {
    // A press anywhere in the pane, on its text or buttons too, gives it the
    // keyboard.
    let area = ui.available_rect_before_wrap();
    let focus = ui.interact(area, pane_focus_id(), Sense::click());
    let pressed_inside = ui.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| area.contains(p))
    });
    if pressed_inside {
        focus.request_focus();
    }
    focus.widget_info(|| WidgetInfo::labeled(Role::Pane, true, DIFF_PANE_LABEL));
    let pane = &mut app.diff_pane;
    let Some(file) = pane.files.get(pane.selected).cloned() else {
        ui.weak("No diff open. Click Open diff under an edit in the transcript.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&file.path).monospace().strong());
        counts(ui, &file, palette);
        ui.label(
            RichText::new(format!(
                "file {} of {}",
                pane.selected + 1,
                pane.files.len()
            ))
            .color(palette.muted),
        );
    });
    ui.horizontal_wrapped(|ui| {
        if ui.add(button(PREV_FILE)).clicked() {
            pane.prev_file();
        }
        if ui.add(button(NEXT_FILE)).clicked() {
            pane.next_file();
        }
        if ui.add(button(PREV_HUNK)).clicked() {
            pane.prev_hunk();
        }
        if ui.add(button(NEXT_HUNK)).clicked() {
            pane.next_hunk();
        }
        let split = pane.layout == DiffLayout::Split;
        if ui.add(button(SPLIT).selected(split)).clicked() {
            pane.toggle_layout();
        }
        let reviewed = pane.is_reviewed(&file);
        if ui.add(button(MARK_REVIEWED).selected(reviewed)).clicked() {
            pane.toggle_reviewed();
        }
    });
    ui.weak("] [ hunk * n p file * v layout * m reviewed (click the pane first)");
    ui.add_space(space::XS);
    if pane.is_reviewed(&file) {
        ui.label(RichText::new(REVIEWED_NOTE).color(palette.muted));
        return;
    }
    if file.truncated {
        ui.label(
            RichText::new("The diff is cut short; it is too long to show whole.")
                .color(palette.warning),
        );
    }
    let hunk_index = pane.hunk.min(file.hunks.len().saturating_sub(1));
    let layout = pane.layout;
    // Scroll to the hunk only when it changes, so the reader can scroll.
    let at = (pane.selected, hunk_index);
    let last_id = Id::unique("codemason_diff_pane_shown_hunk");
    let moved = ui.data(|d| d.get_temp::<(usize, usize)>(last_id)) != Some(at);
    ui.data_mut(|d| d.insert_temp(last_id, at));
    egui::ScrollArea::both()
        .id_salt("diff_pane_scroll")
        .auto_shrink(false)
        .show(ui, |ui| {
            for (index, hunk) in file.hunks.iter().enumerate() {
                let response = ui
                    .scope(|ui| match layout {
                        DiffLayout::Unified => unified_hunk_ui(ui, hunk, palette, usize::MAX),
                        DiffLayout::Split => split_hunk_ui(ui, hunk, palette),
                    })
                    .response;
                if moved && index == hunk_index {
                    // Keep the current hunk in view.
                    response.scroll_to_me_animation(
                        Some(egui::Align::TOP),
                        egui::style::ScrollAnimation::none(),
                    );
                }
                ui.add_space(space::SM);
            }
        });
}
