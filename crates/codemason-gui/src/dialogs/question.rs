//! The agent's question dialog: an agent waits for the user's answer.
//!
//! Draws the front of `App::session.question_queue` the way the TUI's
//! `render_question_dialog` does: the question, then either its options
//! (with the highlighted one, `question_selected_index`, and the default
//! marked) or a free-text answer field, plus the countdown of a timed
//! question.
//!
//! - **Options.** Up/Down and j/k move the highlight and Enter answers, as
//!   in the TUI; the keys reach the App through the router. Clicking an
//!   option moves the highlight (the state the arrows move); a double click
//!   or Select sends Enter.
//! - **Free text.** The answer field is a Material outlined field bound to
//!   the App's own `pending_question_input`, so typing, a paste and an IME
//!   composition are applied by the editor (the router leaves a dialog field
//!   its typing, and the prompt's composer for dialog fields removes the keys
//!   the input method owns). Enter (the key or Submit) runs the TUI's submit
//!   arm, which answers with the trimmed text. A secret question masks the
//!   field.
//! - Esc (or Dismiss) answers with the TUI's dismissal marker.
//!
//! The dialog is a Material dialog: the question is its body text, the
//! options are list items (the highlighted one selected, the default marked
//! by its trailing text), and Select and Dismiss are its action row.
//!
//! app-core answers a question only after it has been the front for
//! `QUESTION_ANSWER_DELAY` and removes it at once
//! (`App::question_answerable`), so a double Enter or a double click cannot
//! answer the next question unseen. The buttons are disabled meanwhile. The
//! TUI's timeout (a timed question answers its default) runs from the
//! shell's timer poll every frame (`App::poll_timers`), drawn or not.
//!
//! Dependencies: `codemason-app-core`, `codemason-types` (countdown text)
//! and `egui`.

use std::time::Duration;

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use egui::Id;

use super::{material_actions, material_hint, material_modal, plain, send_key};
use crate::material::dialog::dialog_body;
use crate::material::list::list_item;
use crate::material::text_field::{field_id, outlined_secret_field, outlined_text_field};
use crate::metrics::space;
use crate::palette::palette_for;
use crate::panels::prompt::prompt_id;

/// Title and accessible label of the dialog.
pub const QUESTION_TITLE: &str = "Agent Question";

/// Accessible label of the option list.
pub const OPTIONS_LABEL: &str = "Options";

/// Accessible label of the free-text answer field.
pub const ANSWER_LABEL: &str = "Answer";

/// The trailing text marking the option a timed question answers by default.
pub const DEFAULT_MARK: &str = "(default)";

/// The button that answers with the highlighted option (Enter).
pub const SELECT_OPTION: &str = "Select";

/// The button that answers with the typed text (Enter).
pub const SUBMIT: &str = "Submit";

/// The button that dismisses the question (Esc).
pub const DISMISS: &str = "Dismiss";

/// Width of the dialog's content, in points.
const WIDTH: f32 = 520.0;

/// The keyboard hint under an option list.
const OPTIONS_HINT: &str = "Up/Down or j/k to navigate  Enter to select  Esc to dismiss";

/// The keyboard hint under the free-text field.
const ANSWER_HINT: &str = "Enter to submit  Esc to dismiss";

/// Widget id of the free-text answer field.
///
/// # Returns
///
/// The id of the answer field's editor, which the router and
/// [`super::show`]'s focus check both name.
#[must_use]
pub fn answer_field_id() -> Id {
    field_id(ANSWER_LABEL)
}

/// The TUI's countdown suffix of a timed question: `(m:ss to default)`.
fn countdown(expire_at_ms: Option<u64>) -> Option<String> {
    let deadline = expire_at_ms?;
    let now = codemason_app_core::text_utils::now_unix_ms();
    let remaining = deadline.saturating_sub(now) / 1000;
    Some(if remaining == 0 {
        "(auto-answering default...)".to_string()
    } else {
        format!(
            "({} to default)",
            codemason_types::strutil::format_mmss(remaining)
        )
    })
}

/// The memory slot holding the option highlighted last frame, so a newly
/// highlighted option is scrolled into view once.
fn last_selected_id() -> Id {
    Id::unique("codemason_question_last_selected")
}

/// Draw the question dialog for the front question, if any.
///
/// # Arguments
///
/// * `ctx` - the frame's context.
/// * `app` - the App whose question queue is read; the free-text field
///   edits its `pending_question_input`.
///
/// # Returns
///
/// `true` while a question is waiting.
pub fn show(ctx: &egui::Context, app: &mut App) -> bool {
    // The question is shown from this frame on; it can be answered once
    // app-core's answer delay has passed.
    app.note_question_front();
    let answerable = app.question_answerable();
    let Some(request) = app.session.question_queue.front() else {
        return false;
    };
    let question = request.question.clone();
    let options = request.options.clone();
    let default = request.resolved_default_index();
    let secret = request.secret;
    let countdown = countdown(request.expire_at_ms);
    let selected = app.session.question_selected_index;
    let palette = palette_for(app, ctx);
    let answer = &mut app.session.pending_question_input;
    let mut clicked: Option<(usize, bool)> = None;
    let moved = ctx.data_mut(|d| {
        let last = d.get_temp::<usize>(last_selected_id());
        d.insert_temp(last_selected_id(), selected);
        last != Some(selected)
    });
    material_modal(
        ctx,
        &palette,
        "codemason_question_dialog",
        QUESTION_TITLE,
        WIDTH,
        |ui| {
            dialog_body(ui, &palette, &question);
            if options.is_empty() {
                let field = if secret {
                    outlined_secret_field(ui, &palette, ANSWER_LABEL, answer, None, false)
                } else {
                    outlined_text_field(ui, &palette, ANSWER_LABEL, answer, None, false)
                };
                // The answer field takes the typing while the question is
                // open.
                if ctx.memory(|m| m.focused().is_none_or(|id| id == prompt_id())) {
                    field.request_focus();
                }
                ui.add_space(space::LG);
                ui.add_enabled_ui(answerable, |ui| {
                    material_actions(
                        ui,
                        &palette,
                        &[(SUBMIT, plain(Key::Enter)), (DISMISS, plain(Key::Esc))],
                        true,
                    );
                });
                ui.add_space(space::LG);
                material_hint(ui, &palette, ANSWER_HINT);
            } else {
                // Many options scroll, so Select and Dismiss stay in view.
                let list = egui::ScrollArea::vertical()
                    .id_salt("question_options")
                    .max_height(super::body_height(ctx, 0.5, super::DIALOG_CHROME))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (i, option) in options.iter().enumerate() {
                                let mark = (i == default).then_some(DEFAULT_MARK);
                                let row =
                                    list_item(ui, &palette, option, None, mark, i == selected);
                                if i == selected && moved {
                                    row.scroll_to_me(None);
                                }
                                if row.double_clicked() {
                                    clicked = Some((i, true));
                                } else if row.clicked() {
                                    clicked = Some((i, false));
                                }
                            }
                        })
                        .response
                    });
                list.inner.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::List, true, OPTIONS_LABEL)
                });
                ui.add_space(space::LG);
                ui.add_enabled_ui(answerable, |ui| {
                    material_actions(
                        ui,
                        &palette,
                        &[
                            (SELECT_OPTION, plain(Key::Enter)),
                            (DISMISS, plain(Key::Esc)),
                        ],
                        true,
                    );
                });
                ui.add_space(space::LG);
                material_hint(ui, &palette, OPTIONS_HINT);
            }
            if let Some(countdown) = countdown.as_deref() {
                material_hint(ui, &palette, countdown);
            }
        },
    );
    if let Some((i, choose)) = clicked {
        app.session.question_selected_index = i;
        if choose && answerable {
            send_key(ctx, plain(Key::Enter));
        }
    }
    // The countdown changes every second; disabled buttons wake sooner.
    ctx.request_repaint_after(if answerable {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(100)
    });
    true
}
