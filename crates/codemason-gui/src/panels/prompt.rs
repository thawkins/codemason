//! The prompt editor: an egui `TextEdit` over the App's own input buffer.
//!
//! # One buffer
//!
//! The App's input state (`App::session.input` and `input_cursor`) is the
//! only copy of the prompt. [`prompt_ui`] binds the `TextEdit` to that
//! `String` directly, so whatever the App does to it (a forwarded key, a
//! submit that clears it, an accepted `@` mention, Alt+Enter's newline)
//! shows in the next frame, and whatever the editor does (typing, an IME
//! commit, a paste, a mouse edit) is the App's text at once. Only the cursor
//! is copied, both ways, each frame:
//!
//! - before drawing, a cursor the App moved is pushed into the editor's
//!   state;
//! - after drawing, the editor's cursor is written back, and an edit is
//!   reported with `App::input_edited`, which refreshes the `/` and `@`
//!   completion menus the way typing through `App::handle_key` does.
//!
//! Submission is never done here: Enter is forwarded to `App::handle_key`
//! by the key router ([`crate::keys::is_prompt_app_key`]), which runs the
//! TUI's submit path.
//!
//! # Before routing
//!
//! [`PromptInput::prepare`] runs each frame before the key router:
//!
//! - typing reaches the prompt even when it is not focused: text, an IME
//!   commit, a paste or an editing key (`keys::is_prompt_edit_key`) makes
//!   the prompt take the focus, so the editor applies it once and by
//!   grapheme, never the App's char-based editing (carriage returns are
//!   stripped from a paste, as the TUI does);
//! - Up and Down go to the App on the first and last logical line (input
//!   history, as the TUI's `HistoryUp`/`HistoryDown`), and between lines
//!   stay with the editor;
//! - a key the App handles for the prompt that follows an edit in the same
//!   frame (typed text, then Enter) waits for the next frame, so the App
//!   sees the text before the key, in the order the user typed;
//! - a [`craft_ui::ime::Composer`] follows the IME composition and removes
//!   the keys the input method owns (the Enter that confirms a word, the
//!   Backspace that edits marked text) before the router could forward
//!   them to the App. The IME events themselves stay for the `TextEdit`,
//!   which inserts each commit exactly once and moves the cursor by
//!   grapheme.
//!
//! The `@` mention completions (`App::file_menu`) and the `/` command
//! completions (`App::slash_menu`) are drawn as a popup at the prompt; their
//! keys are forwarded to the App. The images staged with Alt+V
//! (`pending_attachments`) are listed above the prompt.
//!
//! Dependencies: `codemason-app-core` (the App state), `craft-ui` (the IME
//! composer) and `egui`.

use codemason_app_core::app::App;
use craft_ui::ime::Composer;
use egui::text::{CCursor, CCursorRange};
use egui::{Event, Id};

use codemason_app_core::input_event::KeyInput;

use crate::keys::{Focus, is_key_text, is_prompt_app_key, is_prompt_edit_key, to_key_input};

/// Accessible label of the prompt input.
pub const PROMPT_LABEL: &str = "Prompt";

/// Accessible label of the `@` mention completion list.
pub const MENTION_LABEL: &str = "Mention completions";

/// Accessible label of the `/` command completion list.
pub const SLASH_LABEL: &str = "Slash commands";

/// Accessible label of the pending-attachments row.
pub const ATTACHMENTS_LABEL: &str = "Attachments";

/// Placeholder shown in an empty prompt.
const HINT: &str = "Ask codemason...";

/// A focus under which every key [`is_prompt_app_key`] can match does, for
/// the conservative frame split in [`app_key_after_edit`].
const ANY_APP_KEY: Focus = Focus::Prompt {
    has_selection: false,
    completion_open: true,
    up_to_app: true,
    down_to_app: true,
};

/// Rows of a completion list shown at once (the TUI's
/// `FILE_MENU_VISIBLE_ROWS`; the App keeps the file menu's `scroll_offset`
/// in step).
const COMPLETION_ROWS: usize = 8;

/// Widget id of the prompt input.
///
/// # Returns
///
/// The id the prompt `TextEdit` uses, to ask whether it has the keyboard.
#[must_use]
pub fn prompt_id() -> Id {
    Id::unique("codemason_prompt")
}

/// The prompt's input handling that must run before the key router.
#[derive(Debug, Default)]
pub struct PromptInput {
    composer: Composer,
    deferred: Vec<Event>,
}

impl PromptInput {
    /// Prepare this frame's input for the prompt and tell the router where
    /// the keyboard is (see the module docs, "Before routing").
    ///
    /// # Arguments
    ///
    /// * `ctx` - the frame's context, before any widget reads input.
    /// * `app` - the App, for whether the prompt is editable and whether a
    ///   completion menu is open.
    ///
    /// # Returns
    ///
    /// The focus to pass to [`crate::keys::KeyRouter::route`]:
    /// [`Focus::Prompt`] while the prompt is focused and editable.
    pub fn prepare(&mut self, ctx: &egui::Context, app: &App) -> Focus {
        if !self.deferred.is_empty() {
            let deferred = std::mem::take(&mut self.deferred);
            ctx.input_mut(|input| {
                let later = std::mem::replace(&mut input.events, deferred);
                input.events.extend(later);
            });
        }
        let id = prompt_id();
        // A locked prompt (a modal owns the keys) takes no typing; the App
        // gets every key.
        let mut focused = !app.is_input_locked() && ctx.memory(|m| m.has_focus(id));
        if !focused && !app.is_input_locked() && ctx.input(|i| types_into_prompt(&i.events)) {
            ctx.memory_mut(|m| m.request_focus(id));
            focused = true;
        }
        if !focused {
            if self.composer.is_composing() {
                self.composer = Composer::new();
            }
            return Focus::Elsewhere;
        }
        let composer = &mut self.composer;
        let deferred = &mut self.deferred;
        ctx.input_mut(|input| {
            strip_carriage_returns(&mut input.events);
            if let Some(at) = app_key_after_edit(&input.events) {
                *deferred = input.events.split_off(at);
            }
            let processed = composer.process(&input.events, input.time);
            input.events = keep_with_ime_events(&input.events, &processed.events);
        });
        if !self.deferred.is_empty() {
            ctx.request_repaint();
        }
        let has_selection = egui::TextEdit::load_state(ctx, id)
            .and_then(|state| state.cursor.char_range())
            .is_some_and(|range| !range.is_empty());
        let (up_to_app, down_to_app) = history_keys_to_app(app);
        Focus::Prompt {
            has_selection,
            completion_open: completion_open(app),
            up_to_app,
            down_to_app,
        }
    }
}

/// Whether the frame holds input the prompt editor must apply: text, an IME
/// event, a paste, or an editing key ([`is_prompt_edit_key`]). A `Text`
/// that a Ctrl, Alt or Cmd chord typed belongs to the chord and does not
/// count (the router forwards or drops it).
///
/// When the prompt is unfocused, such input focuses it, so the editor types
/// it once and edits by grapheme; nothing reaches the App's char-based
/// editing, and a character with no key (a dead key, a layout character, an
/// IME commit) is not lost.
fn types_into_prompt(events: &[Event]) -> bool {
    let mut previous: Option<KeyInput> = None;
    for event in events {
        if let Some(key) = to_key_input(event) {
            if is_prompt_edit_key(key) {
                return true;
            }
            previous = Some(key);
            continue;
        }
        match event {
            Event::Paste(_) | Event::Ime(_) => return true,
            Event::Text(text) => {
                let chord_text = previous.is_some_and(|key| {
                    (key.mods.ctrl || key.mods.alt || key.mods.cmd) && is_key_text(key, text)
                });
                if !chord_text {
                    return true;
                }
            }
            _ => {}
        }
        previous = None;
    }
    false
}

/// Whether Up and Down go to the App, as `(up, down)`: the TUI's
/// `HistoryUp` browses history from the first logical line and
/// `HistoryDown` from the last (otherwise they move between lines), both
/// browse while a history entry is shown, and the Memory panel takes them
/// whenever it is visible.
fn history_keys_to_app(app: &App) -> (bool, bool) {
    if app.session.history_index.is_some() || app.show_memory {
        return (true, true);
    }
    let input = &app.session.input;
    let byte = input
        .char_indices()
        .nth(app.session.input_cursor)
        .map_or(input.len(), |(byte, _)| byte);
    (!input[..byte].contains('\n'), !input[byte..].contains('\n'))
}

/// Whether the App has an `@` mention or `/` command menu open.
fn completion_open(app: &App) -> bool {
    app.file_menu.is_some() || app.slash_menu.is_some()
}

/// Remove `\r` from pasted text, as the TUI's paste does.
fn strip_carriage_returns(events: &mut [Event]) {
    for event in events {
        if let Event::Paste(text) = event
            && text.contains('\r')
        {
            text.retain(|c| c != '\r');
        }
    }
}

/// The index of the first key press the App handles for the prompt
/// ([`is_prompt_app_key`]) that follows an editing event in the same frame.
///
/// The router forwards that key before any widget runs, so the App would see
/// it before the text typed ahead of it; it waits for the next frame
/// instead. Every completion-menu key counts, since a menu can open from the
/// text typed ahead of it.
fn app_key_after_edit(events: &[Event]) -> Option<usize> {
    let mut edited = false;
    for (i, event) in events.iter().enumerate() {
        if let Some(key) = to_key_input(event) {
            if edited && is_prompt_app_key(key, ANY_APP_KEY) {
                return Some(i);
            }
            edited = true;
            continue;
        }
        if matches!(
            event,
            Event::Text(_) | Event::Paste(_) | Event::Copy | Event::Cut | Event::Ime(_)
        ) {
            edited = true;
        }
    }
    None
}

/// `events` without the ones the composer used, keeping the IME events.
///
/// `kept` is the composer's output: `events` in order, minus the IME events
/// and the keys, text and clipboard events the input method owns. The IME
/// events go back in for the `TextEdit`, which applies the composition.
fn keep_with_ime_events(events: &[Event], kept: &[Event]) -> Vec<Event> {
    let mut kept = kept.iter().peekable();
    events
        .iter()
        .filter(|event| {
            if kept.peek() == Some(event) {
                kept.next();
                true
            } else {
                matches!(event, Event::Ime(_))
            }
        })
        .cloned()
        .collect()
}

/// Draw the prompt editor over the App's input buffer, with the `@` mention
/// completions above it while the App has them open.
///
/// # Arguments
///
/// * `ui` - the ui to draw into.
/// * `app` - the App whose `session.input` the editor edits.
pub fn prompt_ui(ui: &mut egui::Ui, app: &mut App) {
    let id = prompt_id();
    push_app_cursor(ui.ctx(), id, app.session.input_cursor);
    let editable = !app.is_input_locked();
    let queued = app.session.input_queue.len();
    attachments_row(ui, app);
    let output = ui
        .horizontal(|ui| {
            let label = ui.label(PROMPT_LABEL);
            if queued > 0 {
                // The TUI's queue counter before the prompt, e.g. "03>".
                ui.label(format!("{queued:02}>"));
            }
            let output = egui::TextEdit::multiline(&mut app.session.input)
                .id(id)
                .desired_rows(1)
                .desired_width(f32::INFINITY)
                .return_key(None)
                .interactive(editable)
                .hint_text(HINT)
                .show(ui);
            output.response.response.clone().labelled_by(label.id);
            output
        })
        .inner;
    let cursor = output.cursor_range.map(|range| range.primary.index.0);
    if output.response.response.changed() {
        app.input_edited(cursor.unwrap_or(app.session.input_cursor));
    } else if let Some(cursor) = cursor {
        app.session.input_cursor = cursor;
    }
    if editable {
        completion_popup(ui, app, output.response.response.rect);
    }
}

/// Move the editor's cursor to the App's when the App moved it.
fn push_app_cursor(ctx: &egui::Context, id: Id, cursor: usize) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    if state.cursor.char_range().map(|range| range.primary.index.0) != Some(cursor) {
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(cursor))));
        state.store(ctx, id);
    }
}

/// The App's open completion menu, `@` mentions or `/` commands, in a
/// popup above the prompt at `anchor`. A click accepts an entry as Tab does.
fn completion_popup(ui: &egui::Ui, app: &mut App, anchor: egui::Rect) {
    let clicked = if let Some(menu) = app.file_menu.as_ref() {
        let rows: Vec<String> = menu.matches.iter().map(|m| m.display.clone()).collect();
        let shown = CompletionList {
            label: MENTION_LABEL,
            rows: &rows,
            selected: menu.selected,
            first: menu.scroll_offset,
            empty: "No matching files",
        };
        shown.show(ui, anchor)
    } else if let Some(menu) = app.slash_menu.as_ref() {
        let rows: Vec<String> = menu
            .matches
            .iter()
            .map(|m| format!("/{}  {}", m.trigger, m.description))
            .collect();
        let shown = CompletionList {
            label: SLASH_LABEL,
            rows: &rows,
            selected: menu.selected,
            // The App keeps no scroll offset for this menu: keep the
            // highlighted row in view.
            first: menu.selected.saturating_sub(COMPLETION_ROWS - 1),
            empty: "No matching commands",
        };
        shown.show(ui, anchor)
    } else {
        None
    };
    let Some(i) = clicked else {
        return;
    };
    if let Some(menu) = app.file_menu.as_mut() {
        menu.selected = i;
        let _ = app.accept_file_menu_selection(); // INTENTIONAL: acceptance flag, not a fallible error
    } else if let Some(menu) = app.slash_menu.as_mut() {
        menu.selected = i;
        let _ = app.accept_slash_menu_selection(); // INTENTIONAL: acceptance flag, not a fallible error
    }
    ui.memory_mut(|m| m.request_focus(prompt_id()));
    ui.ctx().request_repaint();
}

/// One completion list as drawn: its rows, the highlighted one and the
/// first row in view.
struct CompletionList<'a> {
    label: &'static str,
    rows: &'a [String],
    selected: usize,
    first: usize,
    empty: &'static str,
}

impl CompletionList<'_> {
    /// Draw the list in a popup above `anchor`.
    ///
    /// # Returns
    ///
    /// The index of a clicked row, if any.
    fn show(&self, ui: &egui::Ui, anchor: egui::Rect) -> Option<usize> {
        let mut clicked = None;
        egui::Area::new(Id::unique("codemason_completion_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor.left_top())
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let list = ui.vertical(|ui| {
                        let rows = self.rows.iter().enumerate();
                        for (i, row) in rows.skip(self.first).take(COMPLETION_ROWS) {
                            if ui.selectable_label(i == self.selected, row).clicked() {
                                clicked = Some(i);
                            }
                        }
                        if self.rows.is_empty() {
                            ui.weak(self.empty);
                        }
                    });
                    list.response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::ListBox, true, self.label)
                    });
                });
            });
        clicked
    }
}

/// The images staged with Alt+V for the next message, above the prompt.
fn attachments_row(ui: &mut egui::Ui, app: &App) {
    let attachments = &app.session.pending_attachments;
    if attachments.is_empty() {
        return;
    }
    let row = ui.horizontal(|ui| {
        ui.weak(format!("{ATTACHMENTS_LABEL}:"));
        for path in attachments {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            ui.label(name);
        }
    });
    row.response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, ATTACHMENTS_LABEL));
}
