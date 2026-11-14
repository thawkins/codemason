//! The message transcript view (the Messages pane).
//!
//! [`MessagesView`] draws `App::session.messages` as the core renders them:
//! each message becomes styled lines through
//! [`codemason_app_core::message_lines::message_to_lines`] (role prefixes,
//! tool-call headers and summaries, reasoning), laid out with
//! [`crate::styled_text::styled_to_layout_job`]. The GUI adds no message
//! formatting of its own.
//!
//! # Virtualisation
//!
//! Messages have different heights, so the view cannot use
//! `ScrollArea::show_rows` (uniform rows). It keeps one cached height per
//! message and the cumulative offsets (`offsets[i]` is the top of message
//! `i`), and draws through `ScrollArea::show_viewport`: a binary search finds
//! the first message intersecting the viewport, and messages are laid out
//! from there until the viewport's bottom. A message never laid out has an
//! estimated height (its line count times the row height). Every message
//! that is laid out records its real height at the current width, so after
//! a resize the visible messages are corrected on the next frame and the
//! rest as they scroll into view. Only visible messages are ever laid out,
//! so a frame's layout cost does not grow with the transcript.
//!
//! Dependencies: `egui`, `codemason-app-core` (messages, line rendering,
//! tool-call status labels), [`crate::palette`] and [`crate::styled_text`].

use codemason_agent::message::Message;
use codemason_app_core::app::App;
use codemason_app_core::message_lines::{message_to_lines, tool_call_status_labels};
use codemason_app_core::styled::StyledLine;
use egui::{FontId, Rect, Role, ScrollArea, Sense, WidgetInfo, pos2, vec2};

use crate::palette::Palette;
use crate::styled_text::{FONT_SIZE, styled_to_layout_job};

/// Accessible label of the transcript.
pub const MESSAGES_LABEL: &str = "Messages";

/// Inner margin, in points, between the transcript's background and text.
const MARGIN: f32 = 4.0;

/// Smallest height change, in points, treated as a real change.
const HEIGHT_EPSILON: f32 = 0.5;

/// One message's cached rendering.
#[derive(Debug)]
struct Entry {
    /// The message's id, to tell a replaced message from an edited one.
    id: String,
    /// The message's `edit_seq` when `lines` were rendered.
    edit_seq: u64,
    /// The core's rendering of the message.
    lines: Vec<StyledLine>,
    /// `"<Tool>: <status>"` for each tool call, from the core.
    status_labels: Vec<String>,
    /// Height in points: the last laid-out height, or an estimate for a
    /// message never laid out.
    height: f32,
}

impl Entry {
    /// Render `msg` through the core and estimate its height.
    fn render(msg: &Message, app: &App, row_height: f32) -> Self {
        let lines = message_to_lines(msg, &app.tool_step_map, &app.sid_to_display_name, &app.cwd);
        let height = lines.len().max(1) as f32 * row_height;
        Self {
            id: msg.id.clone(),
            edit_seq: msg.edit_seq,
            status_labels: tool_call_status_labels(msg),
            lines,
            height,
        }
    }
}

/// The virtualised message transcript.
#[derive(Debug, Default)]
pub struct MessagesView {
    /// One entry per message, in transcript order.
    entries: Vec<Entry>,
    /// `offsets[i]` is the top of message `i`; the last element is the
    /// total height. Rebuilt when `offsets_dirty`.
    offsets: Vec<f32>,
    /// Whether a height changed since `offsets` was built.
    offsets_dirty: bool,
    /// Messages laid out in the last frame.
    laid_out: usize,
}

impl MessagesView {
    /// Draw the transcript of `app` into `ui`, filling the available space.
    ///
    /// The view starts at, and sticks to, the bottom of the transcript
    /// while the user has not scrolled away from it.
    ///
    /// # Arguments
    ///
    /// * `ui` - where to draw.
    /// * `app` - the application whose `session.messages` are shown.
    /// * `palette` - the colours of the current theme mode.
    pub fn show(&mut self, ui: &mut egui::Ui, app: &App, palette: &Palette) {
        self.laid_out = 0;
        let row_height = ui
            .ctx()
            .fonts_mut(|fonts| fonts.row_height(&FontId::proportional(FONT_SIZE)));
        self.sync(app, row_height);

        let frame = egui::Frame::NONE
            .fill(palette.background)
            .inner_margin(MARGIN)
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("messages_scroll")
                    .auto_shrink(false)
                    .stick_to_bottom(true)
                    .show_viewport(ui, |ui, viewport| self.show_visible(ui, viewport, palette));
            });
        let response = ui.interact(
            frame.response.rect,
            ui.scope_id().with("messages_transcript"),
            Sense::hover(),
        );
        response.widget_info(|| WidgetInfo::labeled(Role::Log, true, MESSAGES_LABEL));
    }

    /// How many messages the last frame laid out.
    ///
    /// Counts messages (one layout job each), not wrapped rows; only
    /// messages intersecting the viewport are laid out.
    #[doc(hidden)]
    #[must_use]
    pub fn layout_count(&self) -> usize {
        self.laid_out
    }

    /// Bring the cached entries in line with `app.session.messages`.
    ///
    /// A message whose id and `edit_seq` match its entry keeps it; any
    /// other is re-rendered through the core. An edited message keeps its
    /// last height until it is measured again, so it does not jump.
    fn sync(&mut self, app: &App, row_height: f32) {
        let messages = &app.session.messages;
        if self.entries.len() > messages.len() {
            self.entries.truncate(messages.len());
            self.offsets_dirty = true;
        }
        for (i, msg) in messages.iter().enumerate() {
            match self.entries.get_mut(i) {
                Some(entry) if entry.id == msg.id && entry.edit_seq == msg.edit_seq => {}
                Some(entry) => {
                    let mut fresh = Entry::render(msg, app, row_height);
                    if entry.id == msg.id {
                        fresh.height = entry.height;
                    }
                    *entry = fresh;
                    self.offsets_dirty = true;
                }
                None => {
                    self.entries.push(Entry::render(msg, app, row_height));
                    self.offsets_dirty = true;
                }
            }
        }
        if self.offsets.len() != self.entries.len() + 1 {
            self.offsets_dirty = true;
        }
    }

    /// Recompute the cumulative offsets from the entry heights.
    fn rebuild_offsets(&mut self) {
        self.offsets.clear();
        self.offsets.reserve(self.entries.len() + 1);
        let mut top = 0.0;
        self.offsets.push(top);
        for entry in &self.entries {
            top += entry.height;
            self.offsets.push(top);
        }
        self.offsets_dirty = false;
    }

    /// Lay out and paint the messages intersecting `viewport`.
    ///
    /// `viewport` is relative to the top of the scrolled content.
    fn show_visible(&mut self, ui: &mut egui::Ui, viewport: Rect, palette: &Palette) {
        let width = ui.available_width().max(1.0);
        if self.offsets_dirty {
            self.rebuild_offsets();
        }
        let total = self.offsets.last().copied().unwrap_or(0.0);
        ui.set_height(total);

        let origin = ui.max_rect().left_top();
        let first = self
            .offsets
            .partition_point(|&top| top <= viewport.min.y)
            .saturating_sub(1);
        let mut y = self.offsets.get(first).copied().unwrap_or(0.0);
        let mut height_changed = false;
        for entry in self.entries.iter_mut().skip(first) {
            if y >= viewport.max.y {
                break;
            }
            let job = styled_to_layout_job(&entry.lines, palette, width);
            let galley = ui.painter().layout_job(job);
            self.laid_out += 1;

            let height = galley.size().y;
            if (entry.height - height).abs() > HEIGHT_EPSILON {
                height_changed = true;
            }
            entry.height = height;

            let rect = Rect::from_min_size(pos2(origin.x, origin.y + y), vec2(width, height));
            let response = ui.interact(rect, ui.scope_id().with(&entry.id), Sense::hover());
            let mut label = galley.text().to_owned();
            for status in &entry.status_labels {
                label.push('\n');
                label.push_str(status);
            }
            ui.painter().galley(rect.min, galley, palette.text);
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &label));
            y += height;
        }
        if height_changed {
            // Positions below the changed message move; draw again with
            // the new offsets.
            self.offsets_dirty = true;
            ui.ctx().request_repaint();
        }
    }
}
