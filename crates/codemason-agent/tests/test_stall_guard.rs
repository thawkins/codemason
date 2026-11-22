//! Tests for `codemason_agent::session::stall_guard`: deciding when a primary
//! agent turn that ended without a tool call stalled mid-task.

use codemason_agent::session::stall_guard::is_stalled_turn;
use codemason_types::event::FinishReason;

#[test]
fn test_stall_guard_announced_step_after_tool_work_is_stall() {
    // Captured from a real `rust-hygiene` skill run that stopped here.
    let text = "Checks 1-3 PASS. Now the full test suite (captured to file per AGENTS guidance).";
    assert!(is_stalled_turn(Some(&FinishReason::Stop), true, text));
}

#[test]
fn test_stall_guard_let_me_and_trailing_colon_are_stalls() {
    let stop = Some(&FinishReason::Stop);
    assert!(is_stalled_turn(
        stop,
        true,
        "Build passes. Let me run clippy next."
    ));
    assert!(is_stalled_turn(
        stop,
        true,
        "I'll check the remaining files."
    ));
    assert!(is_stalled_turn(stop, true, "Running the formatter:"));
    assert!(is_stalled_turn(
        stop,
        true,
        "- Now updating the changelog..."
    ));
}

#[test]
fn test_stall_guard_first_step_is_never_stall() {
    // No tool work yet this turn: a short text reply is a plain answer.
    assert!(!is_stalled_turn(
        Some(&FinishReason::Stop),
        false,
        "Let me know."
    ));
    assert!(!is_stalled_turn(
        Some(&FinishReason::Truncation),
        false,
        "Partial"
    ));
}

#[test]
fn test_stall_guard_final_answers_are_not_stalls() {
    let stop = Some(&FinishReason::Stop);
    assert!(!is_stalled_turn(
        stop,
        true,
        "All five checks pass. Nothing left to fix."
    ));
    assert!(!is_stalled_turn(
        stop,
        true,
        "Done. Want me to push these changes?"
    ));
    assert!(!is_stalled_turn(
        stop,
        true,
        "Fixed. Next: run cargo test and paste the output."
    ));
    assert!(!is_stalled_turn(stop, true, ""));
}

#[test]
fn test_stall_guard_long_report_is_not_stall() {
    // A long final report whose last line happens to read like intent is a
    // deliverable, not narration.
    let mut text = "Summary of changes.\n".repeat(60);
    text.push_str("Now the code builds cleanly.");
    assert!(!is_stalled_turn(Some(&FinishReason::Stop), true, &text));
}

#[test]
fn test_stall_guard_truncated_reply_after_tool_work_is_stall() {
    let text = "The suite is green and the remaining work is to";
    assert!(is_stalled_turn(Some(&FinishReason::Truncation), true, text));
    assert!(is_stalled_turn(Some(&FinishReason::Length), true, text));
    assert!(!is_stalled_turn(
        Some(&FinishReason::Truncation),
        true,
        "   "
    ));
}
