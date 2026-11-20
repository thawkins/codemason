//! The right-click context menu, over `App::context_menu`: Cut / Copy /
//! Paste on the prompt, Copy / Copy as Markdown / Quote in reply on the
//! transcript.
//!
//! A right-click on the prompt or the transcript opens it through app-core
//! ([`open`] calls `App::open_context_menu`), which decides, as in the TUI,
//! which items are enabled: Cut with a selection in the input, Copy with a
//! selection in the pane, Paste in the input with text on the clipboard. On
//! the transcript ([`open_transcript`]), Copy is enabled with a text
//! selection in it, and Copy as Markdown and Quote in reply when the menu
//! opened over a message; app-core runs those two itself.
//!
//! Copy on the transcript copies egui's label selection. egui drops that
//! selection at the end of a frame in which the pointer is pressed off the
//! text, so the item asks egui to copy in the same frame it is chosen. egui
//! also places an empty selection wherever a press lands on text and does
//! not say whether a selection is empty, so a right-click on text enables
//! Copy, which then copies nothing.
//!
//! While it is open app-core owns the keyboard (the menu is an input lock):
//! Up/Down move the highlight past disabled items and Esc closes, through
//! the router, as in the TUI. A left click outside the menu closes it, as
//! in the TUI.
//!
//! Running an item is the one GUI step: the selection the items act on is
//! the egui prompt editor's, which app-core does not see (its own
//! `execute_context_action` reads the terminal's cell selection). So Enter
//! ([`prepare`] takes it before the router) or a click closes the menu
//! through app-core (Esc) and queues the item; the next frame, before the
//! prompt reads input, [`apply_pending`] puts back the prompt's selection
//! from when the menu opened (egui collapses it when the prompt loses the
//! keyboard), gives the prompt the keyboard and hands it the matching egui
//! event (`Copy`, `Cut`, or `Paste` with the
//! clipboard text), so the prompt editor cuts, copies or pastes its own
//! selection exactly as for Ctrl+X/C/V. A disabled item only closes the
//! menu, as app-core's Enter does.
//!
//! Dependencies: `codemason-app-core` (the menu state, the clipboard) and
//! `egui`.

use codemason_app_core::app::{App, ContextAction, SelectionPane};
use codemason_app_core::input_event::Key;
use egui::Id;

use super::{plain, send_key};
use crate::panels::prompt::prompt_id;

/// Accessible label of the menu.
pub const CONTEXT_MENU_LABEL: &str = "Context menu";

/// The item labels, as the TUI shows them.
#[must_use]
pub fn item_label(action: ContextAction) -> &'static str {
    match action {
        ContextAction::Cut => "Cut",
        ContextAction::Copy => "Copy",
        ContextAction::Paste => "Paste",
        ContextAction::CopyMarkdown => "Copy as Markdown",
        ContextAction::QuoteReply => "Quote in reply",
    }
}

/// The egui memory slot of where the menu opens.
fn anchor_id() -> Id {
    Id::unique("codemason_context_menu_anchor")
}

/// The egui memory slot of the prompt editor's state when the menu opened.
fn saved_state_id() -> Id {
    Id::unique("codemason_context_menu_prompt_state")
}

/// The egui memory slot of the item to run next frame.
fn pending_id() -> Id {
    Id::unique("codemason_context_menu_pending")
}

/// Open the context menu over `pane` at the pointer.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `open_context_menu` builds the items.
/// * `pane` - the pane right-clicked (`Input` for the prompt).
/// * `has_selection` - whether that pane's text field holds a selection.
pub fn open(ctx: &egui::Context, app: &mut App, pane: SelectionPane, has_selection: bool) {
    let at = ctx
        .input(|i| i.pointer.interact_pos())
        .unwrap_or_else(|| ctx.content_rect().center());
    ctx.data_mut(|d| d.insert_temp(anchor_id(), at));
    // The prompt loses the keyboard while the menu is open, and egui then
    // collapses its selection; keep it for the item to act on.
    let saved = matches!(pane, SelectionPane::Input)
        .then(|| egui::TextEdit::load_state(ctx, prompt_id()))
        .flatten();
    ctx.data_mut(|d| match saved {
        Some(state) => {
            d.insert_temp(saved_state_id(), state);
        }
        None => d.remove::<egui::text_edit::TextEditState>(saved_state_id()),
    });
    app.open_context_menu(0, 0, pane, has_selection);
}

/// Open the context menu over the transcript at the pointer.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `open_transcript_context_menu` builds the items.
/// * `message` - the text of the message under the pointer, if any.
pub fn open_transcript(ctx: &egui::Context, app: &mut App, message: Option<String>) {
    let at = ctx
        .input(|i| i.pointer.interact_pos())
        .unwrap_or_else(|| ctx.content_rect().center());
    ctx.data_mut(|d| {
        d.insert_temp(anchor_id(), at);
        d.remove::<egui::text_edit::TextEditState>(saved_state_id());
    });
    let has_selection = ctx
        .with_plugin::<egui::text_selection::LabelSelectionState, _>(|s| s.has_selection())
        .unwrap_or(false);
    app.open_transcript_context_menu(has_selection, message);
}

/// Copy the transcript's selected text now: hand egui a Copy event this
/// frame, after the prompt has read its input, and close the menu.
fn copy_transcript_selection(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.surrender_focus(prompt_id()));
    ctx.input_mut(|input| input.events.push(egui::Event::Copy));
    send_key(ctx, plain(Key::Esc));
}

/// Close the menu through app-core (Esc) and queue `action` for the next
/// frame when it is enabled.
fn choose(ctx: &egui::Context, action: ContextAction, enabled: bool) {
    send_key(ctx, plain(Key::Esc));
    if enabled {
        ctx.data_mut(|d| d.insert_temp(pending_id(), action));
    }
}

/// Take this frame's Enter before the router: it runs the highlighted
/// item (see the module docs).
///
/// # Arguments
///
/// * `ctx` - the frame's context, before the router reads input.
/// * `app` - the App whose menu is open.
pub fn prepare(ctx: &egui::Context, app: &App) {
    let Some(menu) = app.context_menu.as_ref() else {
        return;
    };
    let Some(&(action, enabled)) = menu.items.get(menu.selected) else {
        return;
    };
    // app-core runs Copy as Markdown and Quote in reply on its own Enter.
    if matches!(
        action,
        ContextAction::CopyMarkdown | ContextAction::QuoteReply
    ) {
        return;
    }
    let enter = ctx.input_mut(|input| {
        let before = input.events.len();
        input.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    ..
                }
            )
        });
        input.events.len() != before
    });
    if !enter {
        return;
    }
    if enabled && action == ContextAction::Copy && menu.pane == SelectionPane::Messages {
        copy_transcript_selection(ctx);
    } else {
        choose(ctx, action, enabled);
    }
}

/// Run the item chosen last frame on the prompt editor, before the prompt
/// reads this frame's input: focus the prompt and add the egui event that
/// does the same as the item.
///
/// # Arguments
///
/// * `ctx` - the frame's context, before any widget reads input.
pub fn apply_pending(ctx: &egui::Context) {
    let Some(action) = ctx.data_mut(|d| {
        let action = d.get_temp::<ContextAction>(pending_id());
        d.remove::<ContextAction>(pending_id());
        action
    }) else {
        return;
    };
    let event = match action {
        ContextAction::Cut => Some(egui::Event::Cut),
        ContextAction::Copy => Some(egui::Event::Copy),
        // Pasted through the window's own clipboard: the `Event::Paste`
        // arrives next frame, into the prompt focused below. An `arboard`
        // read here froze the window on Wayland, where the window serves its
        // own selection from this thread.
        ContextAction::Paste => None,
        // Never queued: app-core runs these at once.
        ContextAction::CopyMarkdown | ContextAction::QuoteReply => return,
    };
    let saved = ctx.data_mut(|d| {
        let state = d.get_temp::<egui::text_edit::TextEditState>(saved_state_id());
        d.remove::<egui::text_edit::TextEditState>(saved_state_id());
        state
    });
    if let Some(state) = saved {
        state.store(ctx, prompt_id());
    }
    ctx.memory_mut(|m| m.request_focus(prompt_id()));
    match event {
        Some(event) => ctx.input_mut(|input| input.events.push(event)),
        None => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
    }
}

/// Draw the context menu while the App shows it.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose `context_menu` is drawn.
///
/// # Returns
///
/// `true` while the menu is open.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    let Some(menu) = app.context_menu.as_ref() else {
        return false;
    };
    let at = ctx
        .data(|d| d.get_temp::<egui::Pos2>(anchor_id()))
        .unwrap_or_else(|| ctx.content_rect().center());
    let mut chosen = None;
    let area = egui::Area::new(Id::unique("codemason_context_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                let inner = ui.vertical(|ui| {
                    for (i, &(action, enabled)) in menu.items.iter().enumerate() {
                        let item = egui::Button::selectable(i == menu.selected, item_label(action));
                        if ui.add_enabled(enabled, item).clicked() {
                            chosen = Some((action, enabled));
                        }
                    }
                });
                inner.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::MenuListPopup, true, CONTEXT_MENU_LABEL)
                });
            });
        });
    let outside_click = ctx.input(|i| {
        i.pointer.primary_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|pos| !area.response.rect.contains(pos))
    });
    let pane = menu.pane;
    if let Some((action, enabled)) = chosen {
        match action {
            ContextAction::CopyMarkdown | ContextAction::QuoteReply if enabled => {
                app.execute_context_action(action);
                if action == ContextAction::QuoteReply {
                    ctx.memory_mut(|m| m.request_focus(prompt_id()));
                }
            }
            ContextAction::Copy if enabled && pane == SelectionPane::Messages => {
                copy_transcript_selection(ctx);
            }
            _ => choose(ctx, action, enabled),
        }
    } else if outside_click {
        send_key(ctx, plain(Key::Esc));
    }
    true
}
