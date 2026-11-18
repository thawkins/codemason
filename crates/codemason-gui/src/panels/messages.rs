//! The message transcript view (the Messages pane).
//!
//! [`MessagesView`] draws `App::session.messages` as blocks: each message is
//! split by [`codemason_app_core::message_blocks::message_to_blocks`], and
//! each block is drawn with its own widget. The assistant's Markdown goes
//! through `egui_commonmark` (headings, lists, tables, links that open in the
//! browser, code blocks with a Copy button); the user's text is plain; tool
//! calls, notices and reasoning are the core's styled lines, laid out with
//! [`crate::styled_text::styled_to_layout_job`]. Every block's text is
//! selectable.
//!
//! # Framing
//!
//! A user turn is a raised card with a `You` chip; an assistant turn has a
//! thin accent rule down its left edge and no frame; a compaction summary is
//! a card with a `Compaction` chip. Turns are separated by the spacing
//! scale's medium gap.
//!
//! # Virtualisation
//!
//! Messages have different heights, so the view cannot use
//! `ScrollArea::show_rows` (uniform rows). It keeps one cached height per
//! message and the cumulative offsets (`offsets[i]` is the top of message
//! `i`), and draws through `ScrollArea::show_viewport`: a binary search finds
//! the first message intersecting the viewport, and messages are laid out
//! from there until the viewport's bottom. A message never laid out has an
//! estimated height (its text's line count times the row height). Every message
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
//! Dependencies: `egui`, `egui_commonmark`, `codemason-app-core` (messages,
//! blocks), [`crate::palette`], [`crate::metrics`] and
//! [`crate::styled_text`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

use codemason_agent::message::{Message, Role as MessageRole};
use codemason_app_core::activity::{ActivityGroup, group_activity};
use codemason_app_core::app::App;
use codemason_app_core::message_blocks::{ToolState, TranscriptBlock, message_to_blocks};
use codemason_app_core::message_lines::MESSAGE_STREAM_MIN_INTERVAL;
use codemason_app_core::message_text::canonical_tool_name;
use codemason_app_core::styled::StyledLine;
use codemason_app_core::transcript_prefs::{ThinkingDisplay, TranscriptPrefs, Verbosity};
use codemason_types::sanitize_terminal::sanitize_terminal;
use egui::{FontId, Rect, Role, ScrollArea, Sense, WidgetInfo, pos2, vec2};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::icons::{Icon, icon_label};
use crate::metrics::{radius, space, text_size};
use crate::palette::Palette;
use crate::panels::code_block::{CodeAction, code_block_ui};
use crate::styled_text::{Bold, FONT_SIZE, styled_to_layout_job};

/// The chip on a user turn.
pub const USER_CHIP: &str = "You";

/// The chip on a compaction summary.
pub const COMPACTION_CHIP: &str = "Compaction";

/// Width of the accent rule down an assistant turn's left edge, in points.
const RULE_WIDTH: f32 = 2.0;

/// Width of a card's outline, in points.
const CARD_STROKE: f32 = 1.0;

/// A card's inner margin, top and bottom together.
const CARD_PADDING: f32 = space::SM + space::SM;

/// The largest side of an image thumbnail, in points.
pub const THUMBNAIL_SIZE: f32 = 160.0;

/// Width of the caret after a reply that is still streaming, in points.
const CARET_WIDTH: f32 = 7.0;

/// What a block asked the caller to do this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockAction {
    /// Put this code into the prompt.
    Insert(String),
    /// Show this image at full size.
    Enlarge(std::path::PathBuf),
    /// Open the Diff pane on these files, with this one selected.
    OpenDiff(Vec<codemason_app_core::diff_view::FileDiff>, usize),
}

/// The space between two blocks of one message: the added gap plus the
/// item spacing.
const BLOCK_GAP: f32 = 2.0 * space::XS;

/// Accessible label of the transcript.
pub const MESSAGES_LABEL: &str = "Messages";

/// Inner margin, in points, between the transcript's background and text.
const MARGIN: f32 = 4.0;

/// Smallest height change, in points, treated as a real change.
const HEIGHT_EPSILON: f32 = 0.5;

/// One message's cached rendering.
struct Entry {
    /// The message's id, to tell a replaced message from an edited one.
    id: String,
    /// The message's `edit_seq` when `blocks` were made.
    edit_seq: u64,
    /// Who wrote the message.
    role: MessageRole,
    /// The core's blocks of the message.
    blocks: Vec<TranscriptBlock>,
    /// The message's text as written (Markdown for the assistant), for Copy
    /// as Markdown and Quote in reply; terminal escapes removed.
    source: String,
    /// Height in points: the last laid-out height, or an estimate for a
    /// message never laid out.
    height: f32,
}

impl Entry {
    /// Split `msg` into blocks through the core and estimate its height.
    fn render(msg: &Message, app: &App, rows: RowHeights) -> Self {
        let blocks = group_activity(
            message_to_blocks(msg, &app.tool_step_map, &app.cwd),
            app.transcript_prefs.verbosity,
        );
        let mut entry = Self {
            id: msg.id.clone(),
            edit_seq: msg.edit_seq,
            role: msg.role.clone(),
            blocks,
            source: sanitize_terminal(&msg.text_content()).into_owned(),
            height: 0.0,
        };
        entry.height = entry.estimate(rows);
        entry
    }

    /// The height this entry is drawn at before it is measured: each block's
    /// text rows, the gaps between blocks, a card's chip and margins, and the
    /// gap after the message. Exact for plain one-paragraph blocks, so a
    /// transcript of short messages settles in one frame.
    fn estimate(&self, rows: RowHeights) -> f32 {
        let text: f32 = self
            .blocks
            .iter()
            .map(|block| {
                let lines = block.plain_text().lines().count().max(1) as f32;
                match block {
                    TranscriptBlock::Markdown(_)
                    | TranscriptBlock::Plain(_)
                    | TranscriptBlock::Image { .. } => lines * rows.label,
                    TranscriptBlock::Code { .. } => lines.mul_add(
                        rows.code,
                        rows.header + space::XS + CARD_PADDING + CARD_STROKE,
                    ),
                    TranscriptBlock::Notice(_)
                    | TranscriptBlock::Thinking(_)
                    | TranscriptBlock::Tool { .. } => lines * rows.styled,
                    TranscriptBlock::Activity(_) => rows.label,
                    TranscriptBlock::Diff(diff) => {
                        let shown = diff.hunks.first().map_or(0, |h| h.lines.len().min(40)) + 1;
                        (shown as f32).mul_add(rows.code, rows.header + CARD_PADDING + CARD_STROKE)
                    }
                }
            })
            .sum();
        let gaps = self.blocks.len().saturating_sub(1) as f32 * BLOCK_GAP;
        let card = match self.role {
            // egui grows a stroked frame by one stroke width in all.
            MessageRole::User | MessageRole::Compaction => {
                rows.chip + space::XS + CARD_PADDING + CARD_STROKE
            }
            MessageRole::Assistant => 0.0,
        };
        text.max(rows.label) + gaps + card + space::MD
    }
}

/// The row heights a message estimate is built from.
#[derive(Debug, Clone, Copy)]
struct RowHeights {
    /// A row of label text (Markdown, plain text).
    label: f32,
    /// A row of the core's styled lines.
    styled: f32,
    /// The role chip's row.
    chip: f32,
    /// A row of code.
    code: f32,
    /// A code block's header row (its buttons).
    header: f32,
}

impl RowHeights {
    /// The row heights of `ui`'s current style, rounded to whole points as
    /// egui places rows.
    fn of(ui: &egui::Ui) -> Self {
        let body = egui::TextStyle::Body.resolve(ui.style());
        let header = ui.spacing().interact_size.y.round();
        ui.ctx().fonts_mut(|fonts| Self {
            label: fonts.row_height(&body).round(),
            styled: fonts.row_height(&FontId::proportional(FONT_SIZE)).round(),
            chip: fonts
                .row_height(&FontId::proportional(text_size::SMALL))
                .round(),
            code: fonts.row_height(&FontId::monospace(FONT_SIZE)).round(),
            header,
        })
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
    /// `egui_commonmark`'s cache (parsed images, link hooks).
    markdown: CommonMarkCache,
    /// Code a code block's Insert button asked to put in the prompt, for
    /// the caller to take with [`MessagesView::take_insert`].
    pending_insert: Option<String>,
    /// The index of the message under the pointer in the last frame.
    under_pointer: Option<usize>,
    /// An image a thumbnail click asked to show at full size, for the caller
    /// to take with [`MessagesView::take_enlarge`].
    pending_enlarge: Option<std::path::PathBuf>,
    /// The verbosity the entries' blocks were grouped for; a change
    /// regroups every message.
    verbosity: Option<Verbosity>,
    /// The diffs an Open diff button asked the Diff pane to show.
    pending_diff: Option<(Vec<codemason_app_core::diff_view::FileDiff>, usize)>,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("edit_seq", &self.edit_seq)
            .field("blocks", &self.blocks.len())
            .field("source", &self.source.len())
            .field("height", &self.height)
            .finish()
    }
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
            markdown: CommonMarkCache::default(),
            pending_insert: None,
            under_pointer: None,
            pending_enlarge: None,
            verbosity: None,
            pending_diff: None,
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
        // The transcript's node is made before its contents, so egui can
        // place every message's widgets under it.
        //
        // Hover only (a click sense would make the transcript a Tab stop);
        // the shell reads a right-click over it from the pointer to open
        // the context menu, as it holds the App mutably.
        let id = ui.scope_id().with("messages_transcript");
        let response = ui.interact(ui.available_rect_before_wrap(), id, Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Log, true, MESSAGES_LABEL));
        self.show_messages(ui, app, source, palette, None, id);
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
    /// * `node` - the id of the caller's accessibility node for the
    ///   transcript (the `Log` it labels), made before this call; every
    ///   message's widgets are placed under it.
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
        node: egui::Id,
    ) -> ViewScroll {
        self.laid_out = 0;
        let rows = RowHeights::of(ui);
        let now = self.clock.unwrap_or_else(Instant::now);
        let outcome = self.sync(app, source, rows, now);
        if let Some(delay) = outcome.retry_in {
            // A streamed edit is waiting out the throttle; draw it then.
            ui.ctx().request_repaint_after(delay);
        }

        let row_height = ui
            .ctx()
            .fonts_mut(|fonts| fonts.row_height(&FontId::proportional(FONT_SIZE)));
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
                    self.show_visible(
                        ui,
                        viewport,
                        palette,
                        node,
                        source.processing,
                        app.transcript_prefs,
                    );
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

    /// The code a code block's Insert button asked for since the last call,
    /// if any. The caller puts it in the prompt (the view only reads the App).
    pub fn take_insert(&mut self) -> Option<String> {
        self.pending_insert.take()
    }

    /// Carry out what the view's blocks asked for this frame: Insert puts
    /// the code at the end of the prompt, on a line of its own, and focuses
    /// it; a thumbnail click opens the image view; Open diff shows the Diff
    /// pane on the message's diffs. Every caller of
    /// [`Self::show_messages`] calls this after it, with the App it may now
    /// change.
    pub fn apply_actions(&mut self, ctx: &egui::Context, app: &mut App) {
        if let Some(code) = self.take_insert() {
            let input = &mut app.session.input;
            if !input.is_empty() && !input.ends_with('\n') {
                input.push('\n');
            }
            input.push_str(&code);
            let end = app.session.input.chars().count();
            // Moves the cursor to the end and refreshes the completion menus.
            app.input_edited(end);
            ctx.memory_mut(|m| m.request_focus(crate::panels::prompt::prompt_id()));
        }
        if let Some(path) = self.take_enlarge() {
            crate::dialogs::image_view::open(ctx, path);
        }
        if let Some((files, selected)) = self.pending_diff.take() {
            app.diff_pane.open(files, selected);
        }
    }

    /// The image a thumbnail click asked to show at full size since the last
    /// call, if any.
    pub fn take_enlarge(&mut self) -> Option<std::path::PathBuf> {
        self.pending_enlarge.take()
    }

    /// The text of the message under the pointer in the last frame, if it
    /// has any: its Markdown for an assistant message, as typed for a user's.
    #[must_use]
    pub fn message_at_pointer(&self) -> Option<&str> {
        self.under_pointer
            .and_then(|i| self.entries.get(i))
            .map(|e| e.source.as_str())
            .filter(|source| !source.trim().is_empty())
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
        rows: RowHeights,
        now: Instant,
    ) -> SyncOutcome {
        let messages = source.messages;
        let verbosity = app.transcript_prefs.verbosity;
        if self.verbosity.is_some_and(|v| v != verbosity) {
            // The blocks are grouped by verbosity: group every message again.
            self.entries.clear();
            self.offsets_dirty = true;
        }
        self.verbosity = Some(verbosity);
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
                    *entry = Entry::render(msg, app, rows);
                    entry.height = height;
                    self.rendered += 1;
                    rendered_edit = true;
                    self.offsets_dirty = true;
                }
                None => {
                    self.entries.push(Entry::render(msg, app, rows));
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

    /// Lay out and draw the messages intersecting `viewport`.
    ///
    /// `viewport` is relative to the top of the scrolled content; each
    /// message's widgets are placed under the accessibility node `node`.
    /// While `processing`, the last message, if it is the assistant's, ends
    /// with a caret.
    fn show_visible(
        &mut self,
        ui: &mut egui::Ui,
        viewport: Rect,
        palette: &Palette,
        node: egui::Id,
        processing: bool,
        prefs: TranscriptPrefs,
    ) {
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
        let pointer = ui.input(|i| i.pointer.hover_pos());
        self.under_pointer = None;
        let last = self.entries.len().saturating_sub(1);
        for (index, entry) in self.entries.iter_mut().enumerate().skip(first) {
            if y >= viewport.max.y {
                break;
            }
            let top = pos2(origin.x, origin.y + y);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("message", &entry.id))
                    .accessibility_parent(node)
                    .max_rect(Rect::from_min_size(top, vec2(width, f32::INFINITY)))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            let mut actions = Vec::new();
            let style = BlockStyle {
                palette,
                bold,
                streaming: processing && index == last,
                thinking: prefs.thinking,
            };
            draw_message(&mut child, entry, style, &mut self.markdown, &mut actions);
            for action in actions {
                match action {
                    BlockAction::Insert(code) => self.pending_insert = Some(code),
                    BlockAction::Enlarge(path) => self.pending_enlarge = Some(path),
                    BlockAction::OpenDiff(files, selected) => {
                        self.pending_diff = Some((files, selected));
                    }
                }
            }
            self.laid_out += 1;

            if pointer.is_some_and(|p| child.min_rect().contains(p) && ui.clip_rect().contains(p)) {
                self.under_pointer = Some(index);
            }
            let height = child.min_rect().height() + space::MD;
            if (entry.height - height).abs() > HEIGHT_EPSILON {
                height_changed = true;
            }
            entry.height = height;
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

/// How a message's blocks are drawn.
#[derive(Clone, Copy)]
struct BlockStyle<'a> {
    /// The active palette.
    palette: &'a Palette,
    /// How bold text is drawn.
    bold: Bold,
    /// Whether the message is the reply still streaming (it ends with a
    /// caret, and its thinking blocks start open).
    streaming: bool,
    /// How thinking blocks are shown.
    thinking: ThinkingDisplay,
}

/// Draw one message: its frame for the role, then its blocks.
fn draw_message(
    ui: &mut egui::Ui,
    entry: &Entry,
    style: BlockStyle<'_>,
    markdown: &mut CommonMarkCache,
    actions: &mut Vec<BlockAction>,
) {
    let palette = style.palette;
    match entry.role {
        MessageRole::User | MessageRole::Compaction => {
            let chip = if entry.role == MessageRole::User {
                USER_CHIP
            } else {
                COMPACTION_CHIP
            };
            egui::Frame::NONE
                .fill(palette.raised1)
                .stroke(egui::Stroke::new(CARD_STROKE, palette.border))
                .corner_radius(radius::MD)
                .inner_margin(space::SM)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(chip)
                            .size(text_size::SMALL)
                            .color(palette.muted)
                            .strong(),
                    );
                    draw_blocks(ui, entry, style, markdown, actions);
                });
        }
        MessageRole::Assistant => {
            let response = egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: (space::MD) as i8,
                    ..egui::Margin::ZERO
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    draw_blocks(ui, entry, style, markdown, actions);
                })
                .response;
            let rect = response.rect;
            ui.painter().rect_filled(
                Rect::from_min_size(rect.left_top(), vec2(RULE_WIDTH, rect.height())),
                0.0,
                palette.accent,
            );
        }
    }
}

/// Draw `entry`'s blocks, top to bottom, separated by the small gap, and
/// collect what their code blocks asked for into `actions`.
fn draw_blocks(
    ui: &mut egui::Ui,
    entry: &Entry,
    style: BlockStyle<'_>,
    markdown: &mut CommonMarkCache,
    actions: &mut Vec<BlockAction>,
) {
    let BlockStyle {
        palette,
        bold,
        streaming,
        thinking,
    } = style;
    ui.spacing_mut().item_spacing.y = space::XS;
    let mut drawn = 0;
    for (index, block) in entry.blocks.iter().enumerate() {
        let hidden_thinking = match block {
            TranscriptBlock::Thinking(_) => true,
            TranscriptBlock::Activity(group) => thinking_only(group).is_some(),
            TranscriptBlock::Markdown(_)
            | TranscriptBlock::Code { .. }
            | TranscriptBlock::Plain(_)
            | TranscriptBlock::Notice(_)
            | TranscriptBlock::Tool { .. }
            | TranscriptBlock::Image { .. }
            | TranscriptBlock::Diff(_) => false,
        };
        if thinking == ThinkingDisplay::Hidden && hidden_thinking {
            continue;
        }
        if drawn > 0 {
            ui.add_space(space::XS);
        }
        drawn += 1;
        let id = block_id(&entry.id, index);
        match block {
            TranscriptBlock::Markdown(text) => {
                ui.push_id(index, |ui| {
                    CommonMarkViewer::new().show(ui, markdown, text);
                });
            }
            TranscriptBlock::Code { lang, text } => {
                let id = ui.scope_id().with(("code", index));
                if let Some(CodeAction::Insert(code)) =
                    code_block_ui(ui, id, lang.as_deref(), text, palette)
                {
                    actions.push(BlockAction::Insert(code));
                }
            }
            TranscriptBlock::Plain(text) => {
                ui.add(egui::Label::new(egui::RichText::new(text).color(palette.text)).wrap());
            }
            TranscriptBlock::Notice(lines) => {
                let job = styled_to_layout_job(lines, palette, ui.available_width(), bold);
                ui.add(egui::Label::new(job).wrap());
            }
            TranscriptBlock::Thinking(lines) => thinking_ui(ui, id, lines, style),
            TranscriptBlock::Diff(diff) => diff_ui(ui, entry, diff, palette, actions),
            TranscriptBlock::Activity(group) => activity_ui(ui, id, entry, group, style, actions),
            TranscriptBlock::Tool {
                lines,
                status_label,
                state,
                name,
                ..
            } => tool_ui(ui, id, name, lines, status_label, *state, style),
            TranscriptBlock::Image { name, path } => {
                let uri = format!("file://{}", path.display());
                let alt = format!("Image: {name}");
                // A file that cannot be loaded (gone, or a format the
                // window cannot decode) is named instead of drawn.
                let loadable = ui
                    .ctx()
                    .try_load_image(&uri, egui::load::SizeHint::default())
                    .is_ok();
                let response = if loadable {
                    let image = egui::Image::new(uri)
                        .max_size(vec2(THUMBNAIL_SIZE, THUMBNAIL_SIZE))
                        .corner_radius(radius::SM)
                        .alt_text(alt)
                        .sense(Sense::CLICK);
                    ui.add(image).on_hover_text(name.as_str())
                } else {
                    ui.label(egui::RichText::new(block.plain_text()).color(palette.info))
                };
                if loadable && response.clicked() {
                    actions.push(BlockAction::Enlarge(path.clone()));
                }
            }
        }
    }
    if streaming && entry.role == MessageRole::Assistant {
        // A caret where the reply will continue.
        let height = ui.text_style_height(&egui::TextStyle::Body);
        let (rect, _) = ui.allocate_exact_size(vec2(CARET_WIDTH, height), Sense::hover());
        ui.painter().rect_filled(rect, 1.0, palette.accent);
    }
}

/// The id of a message's block, for state kept across frames (whether a
/// row is open): stable while the message streams and as the view scrolls.
#[must_use]
pub fn block_id(message_id: &str, block_index: usize) -> egui::Id {
    egui::Id::unique(("transcript_block", message_id, block_index))
}

/// The label of a running activity row's spinner.
pub const ACTIVITY_RUNNING_LABEL: &str = "Running";

/// The header of a collapsed thinking block.
pub const THINKING_LABEL: &str = "Thinking";

/// A tool call's lines, labelled with its status for assistive technology.
/// A `think` call's thought is drawn as thinking. A finished call's leading
/// marker (`* ` or `[x] `) is drawn as a check or
/// a cross icon beside the lines instead.
#[allow(clippy::too_many_arguments)]
fn tool_ui(
    ui: &mut egui::Ui,
    id: egui::Id,
    name: &str,
    lines: &[StyledLine],
    status_label: &str,
    state: ToolState,
    style: BlockStyle<'_>,
) {
    // A `think` call's reasoning follows the thinking display, like the
    // model's own reasoning: its header is the call, the lines under it the
    // thought.
    if canonical_tool_name(name) == "think" && lines.len() > 1 {
        tool_ui(ui, id, "", &lines[..1], status_label, state, style);
        thinking_ui(ui, id.with("thought"), &lines[1..], style);
        return;
    }
    let icon = match state {
        ToolState::Done => Some((Icon::Check, style.palette.success)),
        ToolState::Failed => Some((Icon::Cross, style.palette.error)),
        ToolState::Running => None,
    };
    let mut lines = lines.to_vec();
    let marked = lines
        .first()
        .and_then(|l| l.spans.first())
        .is_some_and(|marker| matches!(marker.text.as_str(), "* " | "[x] "));
    let Some((icon, color)) = icon.filter(|_| marked) else {
        draw_tool_lines(ui, &lines, status_label, style);
        return;
    };
    if let Some(first) = lines.first_mut() {
        first.spans.remove(0);
    }
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = space::XS;
        ui.label(icon_label(ui.ctx(), icon, FONT_SIZE, color));
        draw_tool_lines(ui, &lines, status_label, style);
    });
}

/// A tool call's lines as one selectable label.
fn draw_tool_lines(
    ui: &mut egui::Ui,
    lines: &[StyledLine],
    status_label: &str,
    style: BlockStyle<'_>,
) {
    let job = styled_to_layout_job(lines, style.palette, ui.available_width(), style.bold);
    let text = job.text.clone();
    let response = ui.add(egui::Label::new(job).wrap());
    response
        .widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{text}\n{status_label}")));
}

/// A row that opens and closes the content under it: a disclosure triangle
/// and `label`, as one click-only button that reports whether it is
/// expanded. `extra` draws after the label on the same row. Whether it is
/// open is kept under `id`; until it is first clicked it is `default_open`.
///
/// # Returns
///
/// Whether the content is open.
fn disclosure(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    icon: Option<Icon>,
    palette: &Palette,
    default_open: bool,
    extra: impl FnOnce(&mut egui::Ui),
) -> bool {
    let key = id.with("open");
    let mut open = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(default_open);
    ui.horizontal(|ui| {
        if let Some(icon) = icon {
            let size = egui::TextStyle::Body.resolve(ui.style()).size;
            ui.label(icon_label(ui.ctx(), icon, size, palette.muted));
        }
        let row = ui.text_style_height(&egui::TextStyle::Body);
        let galley = egui::WidgetText::from(egui::RichText::new(label).color(palette.muted))
            .into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Body,
            );
        let size = vec2(row + space::XS + galley.size().x, row.max(galley.size().y));
        let (rect, response) = ui.allocate_exact_size(size, Sense::CLICK);
        if response.clicked() {
            open = !open;
            ui.data_mut(|d| d.insert_temp(key, open));
        }
        if ui.is_rect_visible(rect) {
            // A triangle pointing right when closed, down when open.
            let icon = Rect::from_center_size(
                pos2(rect.left() + row / 2.0, rect.center().y),
                vec2(row * 0.5, row * 0.5),
            );
            let points = if open {
                vec![icon.left_top(), icon.right_top(), icon.center_bottom()]
            } else {
                vec![icon.left_top(), icon.right_center(), icon.left_bottom()]
            };
            ui.painter().add(egui::Shape::convex_polygon(
                points,
                palette.muted,
                egui::Stroke::NONE,
            ));
            let text_pos = pos2(
                rect.left() + row + space::XS,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter().galley(text_pos, galley, palette.muted);
        }
        response.widget_info(|| WidgetInfo::labeled(Role::Button, true, label));
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_expanded(open));
        extra(ui);
    });
    open
}

/// A thinking block, as [`ThinkingDisplay`] asks: hidden, in full, or under
/// a "Thinking" row that is open while the reply streams.
fn thinking_ui(ui: &mut egui::Ui, id: egui::Id, lines: &[StyledLine], style: BlockStyle<'_>) {
    let full = |ui: &mut egui::Ui| {
        let job = styled_to_layout_job(lines, style.palette, ui.available_width(), style.bold);
        ui.add(egui::Label::new(job).wrap());
    };
    match style.thinking {
        ThinkingDisplay::Hidden => {}
        ThinkingDisplay::Expanded => full(ui),
        ThinkingDisplay::Collapsed => {
            if disclosure(
                ui,
                id,
                THINKING_LABEL,
                None,
                style.palette,
                style.streaming,
                |_| {},
            ) {
                full(ui);
            }
        }
    }
}

/// An activity row: its summary, a spinner while a call runs and the
/// failure count in the error colour; opened, every folded block.
fn activity_ui(
    ui: &mut egui::Ui,
    id: egui::Id,
    entry: &Entry,
    group: &ActivityGroup,
    style: BlockStyle<'_>,
    actions: &mut Vec<BlockAction>,
) {
    // A group of reasoning only is drawn as its thinking blocks, so the
    // thinking display alone decides whether and how it shows.
    if let Some(thoughts) = thinking_only(group) {
        for (index, lines) in thoughts.into_iter().enumerate() {
            thinking_ui(ui, id.with(index), lines, style);
        }
        return;
    }
    let palette = style.palette;
    let label = group.summary.label();
    let errors = group.summary.errors;
    let summary = group.summary;
    let kind = if summary.edits > 0 {
        Icon::Edit
    } else if summary.commands > 0 {
        Icon::Command
    } else if summary.searches > 0 {
        Icon::Search
    } else if summary.reads > 0 {
        Icon::Read
    } else if summary.other > 0 {
        Icon::Agent
    } else {
        Icon::Info
    };
    let open = disclosure(ui, id, &label, Some(kind), palette, false, |ui| {
        if group.running {
            crate::panels::status_bar::busy_spinner(ui, palette, ACTIVITY_RUNNING_LABEL);
        }
        if errors > 0 {
            ui.label(egui::RichText::new(format!("{errors} failed")).color(palette.error));
        }
    });
    if !open {
        return;
    }
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: space::LG as i8,
            ..egui::Margin::ZERO
        })
        .show(ui, |ui| {
            for (index, item) in group.items.iter().enumerate() {
                match item {
                    TranscriptBlock::Tool {
                        lines,
                        status_label,
                        state,
                        name,
                        ..
                    } => tool_ui(ui, id.with(index), name, lines, status_label, *state, style),
                    TranscriptBlock::Thinking(lines) => {
                        thinking_ui(ui, id.with(index), lines, style);
                    }
                    TranscriptBlock::Markdown(_)
                    | TranscriptBlock::Code { .. }
                    | TranscriptBlock::Plain(_)
                    | TranscriptBlock::Notice(_)
                    | TranscriptBlock::Image { .. }
                    | TranscriptBlock::Activity(_) => {}
                    TranscriptBlock::Diff(diff) => diff_ui(ui, entry, diff, palette, actions),
                }
            }
        });
}

/// Every diff of `entry`, in order, folded ones included.
fn entry_diffs(entry: &Entry) -> Vec<&codemason_app_core::diff_view::FileDiff> {
    let mut diffs = Vec::new();
    for block in &entry.blocks {
        match block {
            TranscriptBlock::Diff(diff) => diffs.push(diff),
            TranscriptBlock::Activity(group) => {
                diffs.extend(group.items.iter().filter_map(|item| match item {
                    TranscriptBlock::Diff(diff) => Some(diff),
                    TranscriptBlock::Markdown(_)
                    | TranscriptBlock::Code { .. }
                    | TranscriptBlock::Plain(_)
                    | TranscriptBlock::Notice(_)
                    | TranscriptBlock::Tool { .. }
                    | TranscriptBlock::Thinking(_)
                    | TranscriptBlock::Image { .. }
                    | TranscriptBlock::Activity(_) => None,
                }));
            }
            TranscriptBlock::Markdown(_)
            | TranscriptBlock::Code { .. }
            | TranscriptBlock::Plain(_)
            | TranscriptBlock::Notice(_)
            | TranscriptBlock::Tool { .. }
            | TranscriptBlock::Thinking(_)
            | TranscriptBlock::Image { .. } => {}
        }
    }
    diffs
}

/// An inline diff; Open diff asks for the Diff pane on the message's diffs
/// with this one selected.
fn diff_ui(
    ui: &mut egui::Ui,
    entry: &Entry,
    diff: &codemason_app_core::diff_view::FileDiff,
    palette: &Palette,
    actions: &mut Vec<BlockAction>,
) {
    if crate::panels::diff::inline_diff_ui(ui, diff, palette) {
        let diffs = entry_diffs(entry);
        let selected = diffs
            .iter()
            .position(|d| std::ptr::eq(*d, diff))
            .unwrap_or(0);
        let files = diffs.into_iter().cloned().collect();
        actions.push(BlockAction::OpenDiff(files, selected));
    }
}

/// The thinking blocks of `group` when it holds nothing else.
fn thinking_only(group: &ActivityGroup) -> Option<Vec<&[StyledLine]>> {
    group
        .items
        .iter()
        .map(|item| match item {
            TranscriptBlock::Thinking(lines) => Some(lines.as_slice()),
            TranscriptBlock::Markdown(_)
            | TranscriptBlock::Code { .. }
            | TranscriptBlock::Plain(_)
            | TranscriptBlock::Notice(_)
            | TranscriptBlock::Tool { .. }
            | TranscriptBlock::Image { .. }
            | TranscriptBlock::Activity(_)
            | TranscriptBlock::Diff(_) => None,
        })
        .collect()
}
