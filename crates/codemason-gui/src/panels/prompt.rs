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
//!   stripped from a paste, as the TUI does). It takes the focus only when
//!   no other text field has it and no modal is open: another focused text
//!   field keeps its typing ([`Focus::Widget`]);
//! - while an app-core modal locks the prompt
//!   ([`crate::dialogs::locks_prompt`]), the router gets [`Focus::Modal`]
//!   and the App every key; a composition the IME had not committed is
//!   removed from the buffer, since the editor keeps marked text in it while
//!   composing. A dialog's own text field keeps its typing, and a second
//!   composer removes the keys its input method owns;
//! - while the sessions sidebar has the keyboard (`App::sidebar.focused`,
//!   given by `Alt+S` or a click on it), the prompt gives up the focus and
//!   the router gets [`Focus::Modal`], so the sidebar's keys (arrows,
//!   Enter, `m`, the filter's letters) reach the App and none is typed, as
//!   in the TUI;
//! - `?` on an empty prompt goes to the App, which opens the shortcuts
//!   panel, as in the TUI; after text it types;
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
//! (`pending_attachments`) are listed above the prompt. A right-click opens
//! the Cut/Copy/Paste context menu ([`crate::dialogs::context_menu`]).
//!
//! # The collapse button
//!
//! Left of the field, first in the row, a dense Material icon button
//! ([`COLLAPSE_ALL_LABEL`]) closes every open block in the transcript
//! ([`crate::panels::messages::collapse_all`]). It is drawn whether or not
//! anything is open, so the field never moves, and it sits before the queue
//! counter so neither shifts the other.
//!
//! Dependencies: `codemason-app-core` (the App state), `craft-ui` (the IME
//! composer), this crate's Material components, and `egui`.

use codemason_app_core::app::{App, KeyWindow, SelectionPane};
use craft_ui::ime::Composer;
use egui::text::{CCursor, CCursorRange};
use egui::{Event, Id};

use codemason_app_core::input_event::{Key, KeyInput};

use crate::icons::Icon;
use crate::keys::{Focus, is_key_text, is_prompt_app_key, is_prompt_edit_key, to_key_input};
use crate::material::icon_button::{IconButtonKind, icon_button_inline};
use crate::panels::messages::COLLAPSE_ALL_LABEL;

/// Accessible label of the prompt input.
pub const PROMPT_LABEL: &str = "Prompt";

/// The most text rows the prompt editor grows to before it scrolls.
const MAX_ROWS: f32 = 8.0;

/// Where the height of the prompt's laid-out text is recorded each frame.
fn text_height_id() -> Id {
    prompt_id().with("text_height")
}

/// The prompt's text editor.
fn editor(
    ui: &mut egui::Ui,
    app: &mut App,
    id: Id,
    editable: bool,
) -> egui::text_edit::TextEditOutput {
    egui::TextEdit::multiline(&mut app.session.input)
        .id(id)
        .desired_rows(1)
        .desired_width(f32::INFINITY)
        .return_key(None)
        .interactive(editable)
        .hint_text(HINT)
        .show(ui)
}

/// Where the height of the editor's visible area is recorded each frame.
fn visible_height_id() -> Id {
    prompt_id().with("visible_height")
}

/// The height of the prompt editor's visible area on the last frame: the
/// part of the editor that is on screen, which stops growing at the row cap
/// while the editor's own rectangle keeps the full height of its text.
#[doc(hidden)]
#[must_use]
pub fn prompt_visible_height(ctx: &egui::Context) -> Option<f32> {
    ctx.data(|data| data.get_temp(visible_height_id()))
}

/// Accessible label of the `@` mention completion list.
pub const MENTION_LABEL: &str = "Mention completions";

/// Accessible label of the `/` command completion list.
pub const SLASH_LABEL: &str = "Slash commands";

/// Accessible label of the pending-attachments row.
pub const ATTACHMENTS_LABEL: &str = "Attachments";

/// Placeholder shown in an empty prompt.
const HINT: &str = "Ask codemason...";

/// A focus under which every key [`is_prompt_app_key`] can match does (with
/// the shown popup `window`'s own keys), for the conservative frame split in
/// [`app_key_after_edit`].
const fn any_app_key(window: Option<KeyWindow>) -> Focus {
    Focus::Prompt {
        has_selection: false,
        completion_open: true,
        up_to_app: true,
        down_to_app: true,
        delete_to_app: true,
        window,
        question_opens_shortcuts: true,
    }
}

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
    /// Follows the IME composition of a dialog's text field (the question's
    /// answer, the model filter) while a modal has the keyboard.
    field_composer: Composer,
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
    ///   completion menu is open; a composition the IME had not committed when
    ///   a modal locked the prompt is removed from its input buffer.
    ///
    /// # Returns
    ///
    /// The focus to pass to [`crate::keys::KeyRouter::route`]:
    /// [`Focus::Prompt`] while the prompt is focused and editable,
    /// [`Focus::Modal`] while an app-core modal locks it or the sessions
    /// sidebar has the keyboard, [`Focus::Widget`]
    /// while another text field has the keyboard, else [`Focus::Elsewhere`].
    pub fn prepare(&mut self, ctx: &egui::Context, app: &mut App) -> Focus {
        if !self.deferred.is_empty() {
            let deferred = std::mem::take(&mut self.deferred);
            ctx.input_mut(|input| {
                let later = std::mem::replace(&mut input.events, deferred);
                input.events.extend(later);
            });
        }
        let id = prompt_id();
        let focused_id = ctx.memory(|m| m.focused());
        // A locked prompt (a modal owns the keys) takes no typing; the App
        // gets every key, and a dialog's own text field keeps its typing.
        if crate::dialogs::locks_prompt(app) {
            if self.composer.is_composing() {
                self.drop_preedit(ctx, app);
            }
            let field_focused = ctx.text_edit_focused() && focused_id != Some(id);
            self.compose_for_field(ctx, field_focused);
            return Focus::Modal {
                field_focused,
                field_selection: field_focused && focused_has_selection(ctx),
            };
        }
        // The focused sessions sidebar owns the keyboard, as in the TUI,
        // where it takes every key before the input does: the prompt gives
        // up the focus, and the keys reach the App as under a modal, so none
        // is typed and Enter or Space presses no button.
        if app.sidebar.focused && app.sidebar.visible {
            if self.composer.is_composing() {
                self.drop_preedit(ctx, app);
            }
            if focused_id == Some(id) {
                ctx.memory_mut(|m| m.surrender_focus(id));
            }
            return Focus::Modal {
                field_focused: false,
                field_selection: false,
            };
        }
        let mut focused = focused_id == Some(id);
        // Only another text field keeps the typing; a button focused with
        // Tab does not, so typing still reaches the prompt.
        let other_focused = ctx.text_edit_focused() && focused_id != Some(id);
        let modal_open = ctx.memory(|m| m.top_modal_layer().is_some());
        // Typing takes the focus only from nobody: never from another widget
        // or from under a modal.
        // A focused button or slider keeps its own navigation keys (a slider
        // steps with the arrows); typing still moves to the prompt.
        let navigation = focused_id.is_none();
        if !focused
            && !other_focused
            && !modal_open
            && ctx.input(|i| types_into_prompt(&i.events, navigation))
        {
            ctx.memory_mut(|m| m.request_focus(id));
            focused = true;
        }
        if !focused {
            if self.composer.is_composing() {
                self.composer = Composer::new();
            }
            return if other_focused {
                Focus::Widget {
                    has_selection: focused_has_selection(ctx),
                }
            } else {
                Focus::Elsewhere
            };
        }
        let composer = &mut self.composer;
        let deferred = &mut self.deferred;
        let window = app.key_window();
        ctx.input_mut(|input| {
            strip_carriage_returns(&mut input.events);
            if let Some(at) = app_key_after_edit(&input.events, window) {
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
            delete_to_app: app.show_memory && app.session.input.is_empty(),
            window: app.key_window(),
            question_opens_shortcuts: app.shortcuts_panel_enabled && app.session.input.is_empty(),
        }
    }

    /// While a dialog's text field has the keyboard, remove the keys its
    /// input method owns (the Enter that confirms a word, the Backspace that
    /// edits marked text) before the router forwards them to the App; the
    /// IME events stay for the field's `TextEdit`. Otherwise forget any
    /// composition.
    fn compose_for_field(&mut self, ctx: &egui::Context, field_focused: bool) {
        if !field_focused {
            if self.field_composer.is_composing() {
                self.field_composer = Composer::new();
            }
            return;
        }
        let composer = &mut self.field_composer;
        ctx.input_mut(|input| {
            let processed = composer.process(&input.events, input.time);
            input.events = keep_with_ime_events(&input.events, &processed.events);
        });
    }

    /// Remove the IME's uncommitted (marked) text from the App's input
    /// buffer and end the composition.
    ///
    /// egui's `TextEdit` keeps the marked text inside the buffer, selected,
    /// while the IME composes. When a modal locks the prompt mid-composition
    /// the editor stops taking IME events, so that text would stay in
    /// `App::session.input` as if typed. It is removed when the editor's
    /// selection still holds exactly the composer's marked text, and the
    /// editor state is reset so it no longer treats the range as a
    /// composition. The platform IME is turned off with the prompt (it no
    /// longer asks for it), which drops its copy.
    fn drop_preedit(&mut self, ctx: &egui::Context, app: &mut App) {
        let id = prompt_id();
        let preedit = self.composer.preedit().map(|p| p.text.to_owned());
        let range = egui::TextEdit::load_state(ctx, id).and_then(|state| state.cursor.char_range());
        self.composer = Composer::new();
        let (Some(preedit), Some(range)) = (preedit, range) else {
            return;
        };
        let (a, b) = (range.primary.index.0, range.secondary.index.0);
        let (start, end) = (a.min(b), a.max(b));
        let marked: String = app
            .session
            .input
            .chars()
            .skip(start)
            .take(end - start)
            .collect();
        if marked != preedit {
            return;
        }
        app.session.input = app
            .session
            .input
            .chars()
            .take(start)
            .chain(app.session.input.chars().skip(end))
            .collect();
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(start))));
        state.store(ctx, id);
        app.input_edited(start);
    }
}

/// Whether the focused text field has a non-empty selection.
fn focused_has_selection(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.focused())
        .and_then(|id| egui::TextEdit::load_state(ctx, id))
        .and_then(|state| state.cursor.char_range())
        .is_some_and(|range| !range.is_empty())
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
///
/// `navigation` says whether the cursor keys (arrows, Home, End) count:
/// not while another widget has the keyboard focus, which keeps them.
fn types_into_prompt(events: &[Event], navigation: bool) -> bool {
    let mut previous: Option<KeyInput> = None;
    for event in events {
        if let Some(key) = to_key_input(event) {
            if is_prompt_edit_key(key) && (navigation || !is_navigation_key(key)) {
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

/// Whether `key` only moves the cursor: an arrow, Home or End.
const fn is_navigation_key(key: KeyInput) -> bool {
    matches!(
        key.key,
        Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
    )
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
    (
        app.cursor_on_first_logical_line(),
        app.cursor_on_last_logical_line(),
    )
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
fn app_key_after_edit(events: &[Event], window: Option<KeyWindow>) -> Option<usize> {
    let mut edited = false;
    for (i, event) in events.iter().enumerate() {
        if let Some(key) = to_key_input(event) {
            if edited && is_prompt_app_key(key, any_app_key(window)) {
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

/// The editor state to put back while a right-click on the prompt is in
/// progress, from its press to its release, so the click leaves the
/// selection as it was.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `before_press` - the editor's state from before this frame's right
///   press, if the right button was pressed this frame.
/// * `over_prompt` - whether the pointer is over the prompt.
///
/// # Returns
///
/// The state to restore this frame, if any.
fn keep_selection_through_right_click(
    ctx: &egui::Context,
    before_press: Option<egui::text_edit::TextEditState>,
    over_prompt: bool,
) -> Option<egui::text_edit::TextEditState> {
    let slot = Id::unique("codemason_prompt_right_press");
    if let Some(state) = before_press
        && over_prompt
    {
        ctx.data_mut(|d| d.insert_temp(slot, state));
    }
    let saved = ctx.data(|d| d.get_temp::<egui::text_edit::TextEditState>(slot))?;
    if !ctx.input(|i| i.pointer.button_down(egui::PointerButton::Secondary)) {
        // Released (this frame or earlier): restore once more, then forget.
        ctx.data_mut(|d| d.remove::<egui::text_edit::TextEditState>(slot));
    }
    Some(saved)
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
    let editable = !crate::dialogs::locks_prompt(app);
    let queued = app.session.input_queue.len();
    // egui's editor moves its cursor on any button press and drag, so a
    // right-click would drop the selection the context menu's Cut and Copy
    // act on: keep the editor's state from before the right press.
    let before_right_press = ui
        .input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary))
        .then(|| egui::TextEdit::load_state(ui.ctx(), id))
        .flatten();
    attachments_row(ui, app);
    // The editor starts one row tall and grows with its text. Once the text
    // is taller than `MAX_ROWS` (or than the pane has room for) the editor
    // stops growing and scrolls. The text height is last frame's, so the
    // switch happens one frame after the text crosses the limit.
    let row_height = ui.text_style_height(&egui::TextStyle::Body);
    let max_height = (row_height * MAX_ROWS)
        .min(ui.available_height())
        .max(row_height);
    let text_height: f32 = ui
        .ctx()
        .data(|data| data.get_temp(text_height_id()))
        .unwrap_or_default();
    let scrolls = text_height > max_height;
    let palette = crate::palette::palette_for(app, ui.ctx());
    let collapse = |ui: &mut egui::Ui| {
        // Left of the field and first in the row, so it never moves when the
        // queue counter appears, and its row stays one text row tall.
        let clicked = icon_button_inline(
            ui,
            &palette,
            IconButtonKind::Standard,
            Icon::UnfoldLess,
            COLLAPSE_ALL_LABEL,
            None,
        )
        .clicked();
        if clicked {
            crate::panels::messages::collapse_all(ui.ctx());
        }
    };
    let counter = |ui: &mut egui::Ui| {
        if queued > 0 {
            // The TUI's queue counter before the prompt, e.g. "03>".
            ui.label(format!("{queued:02}>"));
        }
    };
    // `visible` is the part of the editor on screen: its own rectangle, or
    // the scroll area's viewport once it scrolls (the editor's rectangle then
    // extends above and below it).
    let (output, visible) = if scrolls {
        // A plain `horizontal` row is one text row tall; this row is given
        // the whole height budget for the scroll area to fill.
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), max_height),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
                collapse(ui);
                counter(ui);
                let scrolled = egui::ScrollArea::vertical()
                    .id_salt("prompt_scroll")
                    .max_height(max_height)
                    .auto_shrink(false)
                    .show(ui, |ui| editor(ui, app, id, editable));
                (scrolled.inner, scrolled.inner_rect)
            },
        )
        .inner
    } else {
        let output = ui
            .horizontal(|ui| {
                collapse(ui);
                counter(ui);
                editor(ui, app, id, editable)
            })
            .inner;
        let rect = output.response.response.rect;
        (output, rect)
    };
    let visible_height = visible.height();
    ui.ctx().data_mut(|data| {
        data.insert_temp(text_height_id(), output.galley.size().y);
        data.insert_temp(visible_height_id(), visible_height);
    });
    // No visible label: the hint text names the field on screen, and the
    // accessible name is set on the editor itself.
    ui.ctx()
        .accesskit_node_builder(output.response.response.id, |node| {
            node.set_label(PROMPT_LABEL);
        });
    let mut cursor = output.cursor_range.map(|range| range.primary.index.0);
    if let Some(state) = keep_selection_through_right_click(
        ui.ctx(),
        before_right_press,
        output.response.response.contains_pointer(),
    ) {
        cursor = state.cursor.char_range().map(|range| range.primary.index.0);
        state.store(ui.ctx(), id);
    }
    if output.response.response.changed() {
        app.input_edited(cursor.unwrap_or(app.session.input_cursor));
    } else if let Some(cursor) = cursor {
        app.session.input_cursor = cursor;
    }
    if editable && output.response.response.secondary_clicked() {
        // A right-click opens app-core's Cut/Copy/Paste menu over the
        // input; its items act on this editor's selection.
        let has_selection = egui::TextEdit::load_state(ui.ctx(), id)
            .and_then(|state| state.cursor.char_range())
            .is_some_and(|range| !range.is_empty());
        crate::dialogs::context_menu::open(ui.ctx(), app, SelectionPane::Input, has_selection);
    }
    if editable {
        completion_popup(ui, app, visible);
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
