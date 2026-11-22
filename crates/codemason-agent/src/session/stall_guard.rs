//! Detection of a primary agent turn that stalled mid-task.
//!
//! The agent loop ends a turn on the first model response without a tool
//! call. Some models (most often while working through a multi-step skill)
//! end a step with a short progress note that announces the next action
//! ("Checks 1-3 PASS. Now the full test suite.") and then stop without making
//! the call. Others are cut off by the provider mid-reply. In both cases the
//! turn ends as if it were complete, and the user has to type "continue" to
//! get the work going again; `/resume` finds nothing because the run ended
//! normally.
//!
//! [`is_stalled_turn`] recognises those endings so the loop can feed the model
//! [`STALL_NUDGE`] and keep going, at most [`MAX_STALL_NUDGES`] times per
//! turn. The text check is deliberately narrow: only short replies whose last
//! sentence announces the model's own next step, or that end on a colon or an
//! ellipsis, qualify. A question to the user or an instruction addressed to the
//! user ("Next: run ...") never does.
//!
//! Depends only on [`FinishReason`] from `codemason-types`.

use codemason_types::event::FinishReason;

/// Upper bound on stall nudges in one turn, so a model that keeps narrating
/// without acting cannot loop forever.
pub const MAX_STALL_NUDGES: u32 = 2;

/// The user-side message fed back to the model when a turn stalled. It plays
/// the role of the "continue" the user would otherwise have to type.
pub const STALL_NUDGE: &str = "Your last message ended without a tool call, \
    but the task is not finished: you announced a next step or were cut off. \
    Continue now by making the tool call for that step. If the task is in fact \
    complete, reply with your final answer instead.";

/// Replies longer than this are treated as a deliverable (a report or
/// summary), never as a progress note, whatever their last sentence says.
const MAX_NARRATION_BYTES: usize = 600;

/// Sentence openings that announce the model's own next action.
const INTENT_PREFIXES: &[&str] = &[
    "now ",
    "next, ",
    "let me ",
    "let's ",
    "i'll ",
    "i will ",
    "i'm going to ",
    "i am going to ",
    "starting ",
    "moving on",
    "proceeding",
];

/// Decide whether a turn that ended without a tool call stalled mid-task.
///
/// # Arguments
///
/// * `finish` - the finish reason recorded for the final step.
/// * `after_tool_work` - `true` when an earlier step of this turn ran tools;
///   a first-step text reply is an ordinary answer and never a stall.
/// * `text` - the visible text of the final step.
///
/// # Returns
///
/// `true` when the reply was truncated by the provider (`Length` or a silent
/// `Truncation`) and is non-empty, or when it is a short progress note whose
/// last sentence announces a next step.
#[must_use]
pub fn is_stalled_turn(finish: Option<&FinishReason>, after_tool_work: bool, text: &str) -> bool {
    if !after_tool_work {
        return false;
    }
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    if matches!(
        finish,
        Some(FinishReason::Length | FinishReason::Truncation)
    ) {
        return true;
    }
    text.len() <= MAX_NARRATION_BYTES && announces_next_step(text)
}

/// `true` when the last sentence of `text` announces the speaker's next
/// action or trails off on a colon or an ellipsis.
fn announces_next_step(text: &str) -> bool {
    let last_line = text.lines().rev().map(str::trim).find(|l| !l.is_empty());
    let Some(line) = last_line else {
        return false;
    };
    if line.ends_with('?') {
        return false;
    }
    if line.ends_with(':') || line.ends_with("...") {
        return true;
    }
    let sentence = [". ", "! ", "? "]
        .iter()
        .filter_map(|sep| line.rfind(sep).map(|i| i + sep.len()))
        .max()
        .map_or(line, |start| &line[start..]);
    let sentence = sentence
        .trim_start_matches(['-', '*', '#', '>', ' '])
        .to_ascii_lowercase();
    INTENT_PREFIXES.iter().any(|p| sentence.starts_with(p))
}
