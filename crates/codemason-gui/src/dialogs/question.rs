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
//! - **Free text.** The answer field is a `TextEdit` bound to the App's own
//!   `pending_question_input`, so typing, a paste and an IME composition are
//!   applied by the editor (the router leaves a dialog field its typing, and
//!   the prompt's composer for dialog fields removes the keys the input
//!   method owns). Enter (the key or Submit) runs the TUI's submit arm,
//!   which answers with the trimmed text. A secret question masks the field.
//! - Esc (or Dismiss) answers with the TUI's dismissal marker.
//!
//! app-core answers a question only after it has been the front for
//! `QUESTION_ANSWER_DELAY` and removes it at once
//! (`App::question_answerable`), so a double Enter or a double click cannot
//! answer the next question unseen. The buttons are disabled meanwhile. The
//! TUI's timeout (a timed question answers its default) is polled while the
//! dialog is drawn.
//!
//! Dependencies: `codemason-app-core`, `codemason-types` (countdown text)
//! and `egui`.

use std::time::Duration;

use codemason_app_core::app::App;
use codemason_app_core::input_event::Key;
use egui::Id;

use super::{key_buttons, modal, plain, send_key};
use crate::panels::prompt::prompt_id;

/// Title and accessible label of the dialog.
pub const QUESTION_TITLE: &str = "Agent Question";

/// Accessible label of the option list.
pub const OPTIONS_LABEL: &str = "Options";

/// Accessible label of the free-text answer field.
pub const ANSWER_LABEL: &str = "Answer";

/// The button that answers with the highlighted option (Enter).
pub const SELECT_OPTION: &str = "Select";

/// The button that answers with the typed text (Enter).
pub const SUBMIT: &str = "Submit";

/// The button that dismisses the question (Esc).
pub const DISMISS: &str = "Dismiss";

/// Widget id of the free-text answer field.
///
/// # Returns
///
/// The id of the answer `TextEdit`.
#[must_use]
pub fn answer_field_id() -> Id {
    Id::unique("codemason_question_answer")
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
    // A timed question past its deadline answers its default, as in the TUI.
    app.poll_question_expiry();
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
    let answer = &mut app.session.pending_question_input;
    let mut clicked: Option<(usize, bool)> = None;
    modal(ctx, "codemason_question_dialog", QUESTION_TITLE, |ui| {
        ui.label(question.as_str());
        ui.add_space(4.0);
        if options.is_empty() {
            let field = ui
                .horizontal(|ui| {
                    let label = ui.label(ANSWER_LABEL);
                    let field = ui.add(
                        egui::TextEdit::singleline(answer)
                            .id(answer_field_id())
                            .password(secret)
                            .desired_width(360.0),
                    );
                    field.clone().labelled_by(label.id);
                    field
                })
                .inner;
            // The answer field takes the typing while the question is open.
            if ctx.memory(|m| m.focused().is_none_or(|id| id == prompt_id())) {
                field.request_focus();
            }
            ui.add_space(6.0);
            ui.add_enabled_ui(answerable, |ui| {
                key_buttons(
                    ui,
                    &[(SUBMIT, plain(Key::Enter)), (DISMISS, plain(Key::Esc))],
                );
            });
            ui.weak("Enter to submit  Esc to dismiss");
        } else {
            let list = ui.vertical(|ui| {
                for (i, option) in options.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let row = ui.selectable_label(i == selected, option.as_str());
                        if row.double_clicked() {
                            clicked = Some((i, true));
                        } else if row.clicked() {
                            clicked = Some((i, false));
                        }
                        if i == default {
                            ui.weak("(default)");
                        }
                    });
                }
            });
            list.response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::List, true, OPTIONS_LABEL));
            ui.add_space(6.0);
            ui.add_enabled_ui(answerable, |ui| {
                key_buttons(
                    ui,
                    &[
                        (SELECT_OPTION, plain(Key::Enter)),
                        (DISMISS, plain(Key::Esc)),
                    ],
                );
            });
            ui.weak("Up/Down or j/k to navigate  Enter to select  Esc to dismiss");
        }
        if let Some(countdown) = countdown.as_deref() {
            ui.weak(countdown);
        }
    });
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
