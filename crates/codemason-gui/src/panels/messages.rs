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
//! # Cache and streaming
//!
//! Entries are matched to messages by id: when the core evicts the oldest
//! messages, their entries are dropped and every survivor keeps its lines
//! and height; while scrolled up, the scroll offset shrinks by the evicted
//! height so the same message stays in view. While the app is processing,
//! an edited message re-renders at most once per
//! `MESSAGE_STREAM_MIN_INTERVAL` (PERF-042, shared with the terminal), with
//! a repaint scheduled for the end of the window; once processing ends it
//! re-renders at once.
//!
//! Dependencies: `egui`, `codemason-app-core` (messages, line rendering,
//! tool-call status labels), [`crate::palette`] and [`crate::styled_text`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

use codemason_agent::message::Message;
use codemason_app_core::app::App;
use codemason_app_core::message_lines::{
    MESSAGE_STREAM_MIN_INTERVAL, message_to_lines, tool_call_status_labels,
};
use codemason_app_core::styled::StyledLine;
use egui::{FontId, Rect, Role, ScrollArea, Sense, WidgetInfo, pos2, vec2};

use crate::palette::Palette;
use crate::styled_text::{Bold, FONT_SIZE, styled_to_layout_job};

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
#[derive(Debug)]
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
    /// Messages rendered through the core since the view was created.
    rendered: usize,
    /// Index of the first message drawn in the last frame.
    first_visible: Option<usize>,
    /// A fixed time to use instead of the system clock (tests).
    clock: Option<Instant>,
    /// When an edited message was last re-rendered (PERF-042 throttle).
    last_stream_render: Option<Instant>,
    /// Whether the last frame ended scrolled to the bottom.
    at_bottom: bool,
    /// The scroll offset at the end of the last frame.
    scroll_offset: f32,
}

/// The messages a [`MessagesView`] shows.
#[derive(Debug, Clone, Copy)]
pub struct MessageSource<'a> {
    /// The messages, oldest first.
    pub messages: &'a [Message],
    /// Whether they are being streamed (the PERF-042 throttle applies).
    pub processing: bool,
}

/// Where [`MessagesView::show_messages`] moves the view this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollTo {
    /// To the end of the content, which then stays pinned there as content
    /// grows.
    Bottom,
    /// This many points above the end of the content.
    FromBottom(f32),
}

/// Where a [`MessagesView`] drew and how far it is scrolled.
#[derive(Debug, Clone, Copy)]
pub struct ViewScroll {
    /// The rectangle the view filled.
    pub rect: Rect,
    /// The scroll offset from the top, in points.
    pub offset: f32,
    /// The largest scroll offset, in points (zero when everything fits).
    pub max_offset: f32,
    /// The height of one text row, in points.
    pub row_height: f32,
}

/// What [`MessagesView::sync`] changed that drawing must react to.
#[derive(Debug)]
struct SyncOutcome {
    /// Height of the messages evicted from the front of the transcript.
    evicted_height: f32,
    /// When an edit was held back by the streaming throttle, how long until
    /// it may render.
    retry_in: Option<Duration>,
}

impl Default for MessagesView {
    /// An empty view that starts scrolled to the bottom.
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            offsets: Vec::new(),
            offsets_dirty: false,
            laid_out: 0,
            rendered: 0,
            first_visible: None,
            clock: None,
            last_stream_render: None,
            at_bottom: true,
            scroll_offset: 0.0,
        }
    }
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
    /// * `palette` - the colours of the current theme.
    ///
    /// # Returns
    ///
    /// The transcript's response (hover only), labelled [`MESSAGES_LABEL`].
    pub fn show(&mut self, ui: &mut egui::Ui, app: &App, palette: &Palette) -> egui::Response {
        let source = MessageSource {
            messages: &app.session.messages,
            processing: app.session.is_processing,
        };
        let frame = self.show_messages(ui, app, source, palette, None);
        // Hover only (a click sense would make the transcript a Tab stop);
        // the shell reads a right-click over it from the pointer to open
        // the context menu, as it holds the App mutably.
        let response = ui.interact(
            frame.rect,
            ui.scope_id().with("messages_transcript"),
            Sense::hover(),
        );
        response.widget_info(|| WidgetInfo::labeled(Role::Log, true, MESSAGES_LABEL));
        response
    }

    /// Draw `source.messages` into `ui`, filling the available space, and
    /// report the scroll geometry.
    ///
    /// # Arguments
    ///
    /// * `ui` - where to draw.
    /// * `app` - the application, for the core's message rendering (tool
    ///   step numbers, display names, working directory).
    /// * `source` - the messages to show and whether they are streaming.
    /// * `palette` - the colours of the current theme.
    /// * `scroll_to` - where to move this frame (a keyboard scroll, or
    ///   keeping a rebuilt view in place); `None` keeps the user's scrolling.
    ///
    /// # Returns
    ///
    /// The drawn rectangle and the scroll offset, largest offset and row
    /// height after this frame.
    pub fn show_messages(
        &mut self,
        ui: &mut egui::Ui,
        app: &App,
        source: MessageSource<'_>,
        palette: &Palette,
        scroll_to: Option<ScrollTo>,
    ) -> ViewScroll {
        self.laid_out = 0;
        let row_height = ui
            .ctx()
            .fonts_mut(|fonts| fonts.row_height(&FontId::proportional(FONT_SIZE)));
        let now = self.clock.unwrap_or_else(Instant::now);
        let outcome = self.sync(app, source, row_height, now);
        if let Some(delay) = outcome.retry_in {
            // A streamed edit is waiting out the throttle; draw it then.
            ui.ctx().request_repaint_after(delay);
        }

        let mut max_offset = 0.0;
        let frame = egui::Frame::NONE
            .fill(palette.background)
            .inner_margin(MARGIN)
            .show(ui, |ui| {
                let mut scroll = ScrollArea::vertical()
                    .id_salt("messages_scroll")
                    .auto_shrink(false)
                    .stick_to_bottom(true);
                if let Some(ScrollTo::FromBottom(points)) = scroll_to {
                    // The view fills the space left (`auto_shrink(false)`),
                    // so its largest offset is the content height minus that.
                    if self.offsets_dirty {
                        self.rebuild_offsets();
                    }
                    let total = self.offsets.last().copied().unwrap_or(0.0);
                    let max = (total - ui.available_height()).max(0.0);
                    scroll = scroll.vertical_scroll_offset((max - points).max(0.0));
                } else if outcome.evicted_height > 0.0 && !self.at_bottom {
                    // Keep the same message at the top of the viewport while
                    // the user is scrolled up: the content above it shrank.
                    scroll = scroll.vertical_scroll_offset(
                        (self.scroll_offset - outcome.evicted_height).max(0.0),
                    );
                }
                let to_bottom = matches!(scroll_to, Some(ScrollTo::Bottom));
                let output = scroll.show_viewport(ui, |ui, viewport| {
                    self.show_visible(ui, viewport, palette);
                    if to_bottom {
                        // Scroll to the end of the content; once there, the
                        // area sticks to the bottom as more arrives.
                        let bottom = ui.max_rect().bottom();
                        ui.scroll_to_rect_animation(
                            Rect::from_min_max(
                                pos2(ui.max_rect().left(), bottom - 1.0),
                                pos2(ui.max_rect().left() + 1.0, bottom),
                            ),
                            Some(egui::Align::BOTTOM),
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                });
                max_offset = (output.content_size.y - output.inner_rect.height()).max(0.0);
                self.scroll_offset = output.state.clamped_offset().y;
                self.at_bottom = self.scroll_offset >= max_offset - HEIGHT_EPSILON;
            });
        ViewScroll {
            rect: frame.response.rect,
            offset: self.scroll_offset,
            max_offset,
            row_height,
        }
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

    /// How many messages have been rendered through the core since the
    /// view was created (cumulative).
    #[doc(hidden)]
    #[must_use]
    pub fn render_count(&self) -> usize {
        self.rendered
    }

    /// The cached height of each message, in transcript order.
    #[doc(hidden)]
    #[must_use]
    pub fn message_heights(&self) -> Vec<f32> {
        self.entries.iter().map(|e| e.height).collect()
    }

    /// The id of the first message drawn in the last frame.
    #[doc(hidden)]
    #[must_use]
    pub fn first_visible_id(&self) -> Option<&str> {
        self.first_visible
            .and_then(|i| self.entries.get(i))
            .map(|e| e.id.as_str())
    }

    /// Use `now` as the current time instead of the system clock, so the
    /// streaming throttle is deterministic in tests.
    #[doc(hidden)]
    pub fn set_clock(&mut self, now: Instant) {
        self.clock = Some(now);
    }

    /// Bring the cached entries in line with `source.messages`.
    ///
    /// Entries are matched to messages by id, so a message keeps its
    /// rendered lines and measured height when messages before it are
    /// evicted (the core caps a session and drops the oldest message as a
    /// new one arrives). Only new messages and messages whose `edit_seq`
    /// changed are re-rendered through the core; an edited message keeps
    /// its last height until it is measured again, so it does not jump.
    ///
    /// PERF-042: while the app is processing, an edited message that has
    /// already been rendered re-renders at most once per
    /// [`MESSAGE_STREAM_MIN_INTERVAL`]; once processing ends, edits render
    /// at once so the final text is always shown.
    ///
    /// # Returns
    ///
    /// The cumulative height of the messages evicted from the front, and,
    /// when an edit was held back by the throttle, how long until it may
    /// render.
    fn sync(
        &mut self,
        app: &App,
        source: MessageSource<'_>,
        row_height: f32,
        now: Instant,
    ) -> SyncOutcome {
        let messages = source.messages;
        let evicted_height = self.drop_evicted_front(messages);
        self.reorder_by_id(messages);

        let throttled = source.processing
            && self
                .last_stream_render
                .is_some_and(|t| now.saturating_duration_since(t) < MESSAGE_STREAM_MIN_INTERVAL);
        let mut held_back = false;
        let mut rendered_edit = false;
        for (i, msg) in messages.iter().enumerate() {
            match self.entries.get_mut(i) {
                Some(entry) if entry.edit_seq == msg.edit_seq => {}
                Some(_) if throttled => held_back = true,
                Some(entry) => {
                    let height = entry.height;
                    *entry = Entry::render(msg, app, row_height);
                    entry.height = height;
                    self.rendered += 1;
                    rendered_edit = true;
                    self.offsets_dirty = true;
                }
                None => {
                    self.entries.push(Entry::render(msg, app, row_height));
                    self.rendered += 1;
                    self.offsets_dirty = true;
                }
            }
        }
        if rendered_edit {
            self.last_stream_render = Some(now);
        }
        if self.offsets.len() != self.entries.len() + 1 {
            self.offsets_dirty = true;
        }
        let retry_in = held_back.then(|| {
            let since = self
                .last_stream_render
                .map_or(MESSAGE_STREAM_MIN_INTERVAL, |t| {
                    now.saturating_duration_since(t)
                });
            MESSAGE_STREAM_MIN_INTERVAL.saturating_sub(since)
        });
        SyncOutcome {
            evicted_height,
            retry_in,
        }
    }

    /// Drop the entries of messages evicted from the front of `messages`.
    ///
    /// # Returns
    ///
    /// The cumulative height of the dropped entries.
    fn drop_evicted_front(&mut self, messages: &[Message]) -> f32 {
        let Some(first) = messages.first() else {
            return 0.0;
        };
        if self.entries.first().is_none_or(|e| e.id == first.id) {
            return 0.0;
        }
        let Some(count) = self.entries.iter().position(|e| e.id == first.id) else {
            return 0.0;
        };
        self.offsets_dirty = true;
        self.entries.drain(..count).map(|e| e.height).sum()
    }

    /// Put the entries in the order of `messages`, by id.
    ///
    /// Entries already in place are left alone (the usual case, checked
    /// without allocating). Otherwise the entries of the leading messages
    /// that have one are moved into place, the rest are dropped, and
    /// [`Self::sync`] renders the messages after them.
    fn reorder_by_id(&mut self, messages: &[Message]) {
        let in_place = self.entries.len() <= messages.len()
            && self
                .entries
                .iter()
                .zip(messages)
                .all(|(entry, msg)| entry.id == msg.id);
        if in_place {
            return;
        }
        let mut by_id: HashMap<String, Entry> = self
            .entries
            .drain(..)
            .map(|entry| (entry.id.clone(), entry))
            .collect();
        // Entries must stay contiguous from the front; stop at the first
        // message with no entry and let `sync` render the rest.
        self.entries = messages
            .iter()
            .map_while(|msg| by_id.remove(&msg.id))
            .collect();
        self.offsets_dirty = true;
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
        self.first_visible = (first < self.entries.len()).then_some(first);
        let mut height_changed = false;
        let bold = Bold::of(ui.ctx());
        for entry in self.entries.iter_mut().skip(first) {
            if y >= viewport.max.y {
                break;
            }
            let job = styled_to_layout_job(&entry.lines, palette, width, bold);
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
