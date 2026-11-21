//! Diffs: the inline diff under an edit in the transcript, and the Diff
//! pane.
//!
//! [`inline_diff_ui`] draws a [`FileDiff`] in the transcript: the path with
//! its added and removed counts, then its first [`INLINE_MAX_LINES`] lines.
//! A longer diff ends in a banner saying how many lines are left and an
//! [`OPEN_DIFF`] button, which shows the whole diff in an overlay
//! ([`crate::dialogs::diff_view`]) drawn with the Diff pane's own view
//! ([`diff_view_ui`]).
//!
//! The Diff pane shows one file at a time, unified or split (old on the
//! left, new on the right). Its header is the file's path, its added and
//! removed counts, and a [`CLOSE_DIFF`] button that hides the pane
//! (`App::diff_pane.shown`, as the dock's own Close does); one row of
//! 32-point icon buttons under it steps through the files and the hunks and
//! switches the layout and the reviewed mark, with a readout of where the
//! reader is at its right end. A file marked reviewed is drawn collapsed to
//! its header. While the pane holds the keyboard focus (a click in it), `]`
//! and `[` move between hunks, `n` and `p` between files, `v` switches the
//! layout and `m` toggles the reviewed mark ([`take_pane_keys`]); everywhere
//! else those keys type as usual.
//!
//! The pane is narrow (the default layout gives it the right column, about
//! 270 points), so it is laid out to stay narrow: the header is one line
//! whatever the path's length, the controls are icon buttons that fit that
//! width in one row, and nothing is drawn beside the header that the path
//! would then be squeezed out of. The keys are named in the buttons' hover
//! text rather than in a legend line of their own, which cost height on
//! every frame to say something the reader needs once.
//!
//! Lines are drawn in the theme's `diff.*` colours: added and removed rows
//! on their backgrounds, line numbers and hunk headers in
//! `diff.line_number`.
//!
//! Dependencies: `egui`, `codemason-app-core` (the diff model and pane
//! state), [`crate::palette`] and [`crate::metrics`].

use codemason_app_core::app::App;
use codemason_app_core::diff_view::{
    DiffHunk, DiffLayout, DiffLine, DiffLineKind, DiffPaneState, FileDiff,
};
use egui::{Id, RichText, Role, Sense, Stroke, Vec2, WidgetInfo};

use crate::icons::Icon;
use crate::material::icon_button::{DENSE_SIZE, IconButtonKind, icon_button_dense};
use crate::metrics::{radius, space, text_size};
use crate::palette::Palette;

/// The button that opens a transcript diff in the diff overlay.
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

/// The Diff pane's close button: it hides the pane.
pub const CLOSE_DIFF: &str = "Close diff";

/// The Diff pane's header when it has no file to show.
pub const NO_DIFF_OPEN: &str = "No diff open";

/// What an empty Diff pane says to do about it.
pub const NO_DIFF_HINT: &str = "Click Open diff under an edit in the transcript.";

/// What a file marked reviewed shows instead of its hunks.
pub const REVIEWED_NOTE: &str = "Marked reviewed; press m or Reviewed to show it again";

/// Accessible labels of the split layout's two columns.
pub const OLD_COLUMN: &str = "Old";
/// See [`OLD_COLUMN`].
pub const NEW_COLUMN: &str = "New";

/// Most lines of the first hunk an inline diff shows.
pub const INLINE_MAX_LINES: usize = 10;

/// The banner under an inline diff cut short at [`INLINE_MAX_LINES`].
#[must_use]
pub fn more_lines_label(count: usize) -> String {
    format!("{count} more lines...")
}

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

/// One of the pane's four step buttons: its icon, whether there is anything
/// that way, and its name and key in the hover text.
///
/// A button with nothing that way is drawn disabled rather than live: a
/// press at the first hunk must not look like a press that did nothing. Its
/// hover text still shows, which [`egui::Response::on_disabled_hover_text`]
/// is for, so the reader learns why the button is out.
///
/// # Returns
///
/// Whether the button was pressed.
fn nav(ui: &mut egui::Ui, palette: &Palette, active: bool, icon: Icon, label: &str) -> bool {
    let key = match label {
        PREV_FILE => "p",
        NEXT_FILE => "n",
        PREV_HUNK => "[",
        NEXT_HUNK => "]",
        _ => "",
    };
    if active {
        icon_button_dense(ui, palette, IconButtonKind::Standard, icon, label, None)
            .on_hover_text(format!("{label} ({key})"))
            .clicked()
    } else {
        ui.add_enabled_ui(false, |ui| {
            icon_button_dense(ui, palette, IconButtonKind::Standard, icon, label, None)
        })
        .inner
        .on_disabled_hover_text(format!("{label} ({key}): none that way"))
        .clicked()
    }
}

/// A vertical rule between two groups of a toolbar row, one text line high.
fn divider(ui: &mut egui::Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, text_size::BODY), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0, palette.border),
    );
}

/// Where the reader is: the hunk of the file, and the file of the set.
///
/// The pane used to say only `file 2 of 5`, so a file with four hunks gave
/// no sign that a press of `]` had moved to hunks two, three or four. Each
/// part is left out when there is nothing to count against: one file of one
/// file says nothing, and the path gets its width back instead. The counts
/// are written `1/3` rather than `1 of 3` because the pane is narrow and the
/// reader already knows what the number is counting.
fn position(ui: &mut egui::Ui, pane: &DiffPaneState, hunks: usize, palette: &Palette) {
    let mut parts = Vec::new();
    if hunks > 1 {
        parts.push(format!("hunk {}/{}", pane.hunk.min(hunks - 1) + 1, hunks));
    }
    if pane.files.len() > 1 {
        parts.push(format!("file {}/{}", pane.selected + 1, pane.files.len()));
    }
    if parts.is_empty() {
        return;
    }
    ui.label(
        RichText::new(parts.join("  "))
            .size(text_size::SMALL)
            .color(palette.muted),
    );
}

/// `+a -r`, or `+a` and [`WHOLE_FILE_NOTE`]'s short form, as small text: the
/// header's own line, beside its path rather than a line of its own.
///
/// The two counts are one layout job, not two labels, so they keep their
/// order wherever the header lays them out (a right-to-left row would read
/// `-r +a` if they were separate items) and the pair can never be split
/// across the header's width.
fn pane_counts(ui: &mut egui::Ui, diff: &FileDiff, palette: &Palette) {
    let font = egui::FontId::proportional(text_size::SMALL);
    let mut job = egui::text::LayoutJob::default();
    let mut push = |text: &str, colour: egui::Color32| {
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), colour));
    };
    push(&format!("+{}", diff.added), palette.diff_added);
    if diff.whole_file {
        push(" whole file", palette.muted);
    } else {
        push(&format!(" -{}", diff.removed), palette.diff_removed);
    }
    let response = ui.add(egui::Label::new(job));
    if diff.whole_file {
        response.on_hover_text(WHOLE_FILE_NOTE);
    }
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
fn unified_hunk_ui(ui: &mut egui::Ui, hunk: &DiffHunk, palette: &Palette) {
    ui.label(
        RichText::new(&hunk.header)
            .monospace()
            .color(palette.diff_line_number),
    );
    for line in &hunk.lines {
        line_ui(ui, line, palette, (true, true));
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
    let total: usize = diff.hunks.iter().map(|hunk| hunk.lines.len()).sum();
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
            // A solid bar takes its own row under the lines; egui's default
            // floating bar is drawn over the last line of a wide diff.
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::horizontal()
                .id_salt(("inline_diff", &diff.path))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    // The hunks in order, until the line budget is spent.
                    let mut left = INLINE_MAX_LINES;
                    for hunk in &diff.hunks {
                        if left == 0 {
                            break;
                        }
                        ui.label(
                            RichText::new(&hunk.header)
                                .monospace()
                                .color(palette.diff_line_number),
                        );
                        for line in hunk.lines.iter().take(left) {
                            line_ui(ui, line, palette, (true, true));
                        }
                        left = left.saturating_sub(hunk.lines.len());
                    }
                });
            let more = total.saturating_sub(INLINE_MAX_LINES);
            if more > 0 {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(more_lines_label(more)).color(palette.subtle));
                    open = ui.add(button(OPEN_DIFF)).clicked();
                });
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
                unified_hunk_ui(ui, hunk, palette);
            }
            if diff.truncated {
                ui.label(RichText::new(TRUNCATED_NOTE).color(palette.warning));
            }
        });
}

/// What a diff cut short at the line limit says.
pub const TRUNCATED_NOTE: &str = "The diff is cut short; it is too long to show whole.";

/// What a Diff pane key does to the pane state.
type PaneAction = fn(&mut DiffPaneState);

/// The action of a Diff pane key, if `key` is one.
fn pane_key_action(key: egui::Key) -> Option<PaneAction> {
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

/// Apply the Diff pane's keys while it holds the keyboard focus, or while
/// the diff overlay is open: they are
/// taken out of this frame's input, so they neither type nor reach the
/// App. Call before the keys are routed.
pub fn take_pane_keys(ctx: &egui::Context, app: &mut App) {
    let pane_focused = app.diff_pane.shown && ctx.memory(|m| m.has_focus(pane_focus_id()));
    if !pane_focused && !crate::dialogs::diff_view::is_open(ctx) {
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
    // A press on the diff's selectable text clears the focus at the end of
    // that frame (egui's text selection does), so the pane claims it again
    // on the frame after the press as well.
    let claim = pane_focus_id().with("claim");
    let claimed_last_frame = ui
        .data_mut(|d| d.remove_temp::<bool>(claim))
        .unwrap_or(false);
    if pressed_inside || claimed_last_frame {
        focus.request_focus();
    }
    if pressed_inside {
        ui.data_mut(|d| d.insert_temp(claim, true));
    }
    focus.widget_info(|| WidgetInfo::labeled(Role::Pane, true, DIFF_PANE_LABEL));
    if diff_view_ui(ui, &mut app.diff_pane, palette) {
        app.diff_pane.shown = false;
    }
}

/// The diff view the Diff pane and the diff overlay share: the file's path
/// with its counts and a [`CLOSE_DIFF`] button at the top right, the row of
/// step and layout buttons, then the hunks.
///
/// # Arguments
///
/// * `ui` - where to draw.
/// * `pane` - the files shown and where the reader is in them.
/// * `palette` - the colours.
///
/// # Returns
///
/// Whether [`CLOSE_DIFF`] was clicked.
pub fn diff_view_ui(ui: &mut egui::Ui, pane: &mut DiffPaneState, palette: &Palette) -> bool {
    let hunks = pane.current().map_or(0, |file| file.hunks.len());
    // Laid out from the right first: the close button and the counts take
    // their width, and the path is what gives way. Hiding the pane is the
    // dock's own Close (its tab's context menu, or a middle click) done here,
    // where the reader can see it.
    let mut closed = false;
    let (_, ()) = egui::Sides::new().shrink_left().height(DENSE_SIZE).show(
        ui,
        |ui| {
            let Some(file) = pane.files.get(pane.selected) else {
                ui.label(
                    RichText::new(NO_DIFF_OPEN)
                        .size(text_size::SMALL)
                        .color(palette.muted),
                );
                return;
            };
            // Whatever the path does not fit in is truncated with an
            // ellipsis and its whole text in the hover, so the header is one
            // line for every file: a wrapped path would give the header a
            // different height per file and make the toolbar under it jump as
            // the reader stepped through a set.
            ui.add(
                egui::Label::new(RichText::new(&file.path).monospace().strong())
                    .truncate()
                    .sense(Sense::hover()),
            )
            .on_hover_text(&file.path);
        },
        |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                closed = icon_button_dense(
                    ui,
                    palette,
                    IconButtonKind::Standard,
                    Icon::Cross,
                    CLOSE_DIFF,
                    None,
                )
                .clicked();
                if let Some(file) = pane.files.get(pane.selected) {
                    pane_counts(ui, file, palette);
                }
            });
        },
    );
    if closed {
        return true;
    }
    let Some(file) = pane.files.get(pane.selected).cloned() else {
        // On a line of its own, where it wraps to the pane's width: beside
        // the header's close button it would have been laid out past the
        // pane's edge and covered the button.
        ui.label(
            RichText::new(NO_DIFF_HINT)
                .size(text_size::SMALL)
                .color(palette.muted),
        );
        return false;
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::XS;
        if nav(ui, palette, pane.selected > 0, Icon::ChevronLeft, PREV_FILE) {
            pane.prev_file();
        }
        if nav(
            ui,
            palette,
            pane.selected + 1 < pane.files.len(),
            Icon::ChevronRight,
            NEXT_FILE,
        ) {
            pane.next_file();
        }
        if nav(ui, palette, pane.hunk > 0, Icon::ArrowUpward, PREV_HUNK) {
            pane.prev_hunk();
        }
        if nav(
            ui,
            palette,
            pane.hunk + 1 < hunks,
            Icon::ArrowDownward,
            NEXT_HUNK,
        ) {
            pane.next_hunk();
        }
        // Stepping through the file, and changing how it is shown: two
        // groups, so a press meant for the next hunk is not a press on the
        // layout.
        divider(ui, palette);
        let split = pane.layout == DiffLayout::Split;
        let toggled = icon_button_dense(
            ui,
            palette,
            IconButtonKind::Standard,
            Icon::VerticalSplit,
            SPLIT,
            Some(split),
        );
        if toggled.clicked() {
            pane.toggle_layout();
        }
        let reviewed = pane.is_reviewed(&file);
        let mark = icon_button_dense(
            ui,
            palette,
            IconButtonKind::Standard,
            Icon::Check,
            MARK_REVIEWED,
            Some(reviewed),
        );
        if mark.clicked() {
            pane.toggle_reviewed();
        }
        // At the far right of the row: what the four step buttons are
        // stepping through, read after the press rather than before it, and
        // out of the header's narrower width.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            position(ui, pane, hunks, palette);
        });
    });
    if pane.is_reviewed(&file) {
        ui.label(RichText::new(REVIEWED_NOTE).color(palette.muted));
        return false;
    }
    if file.truncated {
        ui.label(RichText::new(TRUNCATED_NOTE).color(palette.warning));
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
                        DiffLayout::Unified => unified_hunk_ui(ui, hunk, palette),
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
    false
}
