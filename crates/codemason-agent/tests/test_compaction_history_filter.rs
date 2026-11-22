//! Tests for dropping superseded compaction summaries from the request.
//!
//! The pre-send compaction path appends a new `Role::Compaction` summary every
//! time the context window fills and never removes the summary it superseded.
//! Each compaction anchors on the previous one, so only the newest is
//! meaningful, but the transcript builder used to replay them all as several
//! consecutive assistant turns. These tests pin the filter that collapses them.

use codemason_agent::message::{Message, MessagePart, Role};
use codemason_agent::session::history::drop_superseded_summaries;

/// Build a synthetic compaction summary, matching what the compaction runner
/// persists: a `Role::Compaction` message carrying one text part.
fn compaction(session: &str, text: &str) -> Message {
    Message::new(
        session,
        Role::Compaction,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

#[test]
fn test_no_summaries_is_a_no_op() {
    let mut history = vec![
        Message::user_text("s", "question"),
        Message::assistant_text("s", "answer"),
    ];

    assert_eq!(drop_superseded_summaries(&mut history), 0);
    assert_eq!(history.len(), 2);
}

#[test]
fn test_empty_history_is_a_no_op() {
    let mut history: Vec<Message> = Vec::new();
    assert_eq!(drop_superseded_summaries(&mut history), 0);
    assert!(history.is_empty());
}

#[test]
fn test_single_summary_is_kept() {
    let mut history = vec![
        Message::user_text("s", "question"),
        compaction("s", "only summary"),
    ];

    assert_eq!(drop_superseded_summaries(&mut history), 0);
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].text_content(), "only summary");
}

#[test]
fn test_only_the_newest_summary_survives() {
    let mut history = vec![
        Message::user_text("s", "first question"),
        compaction("s", "oldest summary"),
        Message::user_text("s", "second question"),
        compaction("s", "middle summary"),
        Message::user_text("s", "third question"),
        compaction("s", "newest summary"),
        Message::assistant_text("s", "final answer"),
    ];

    assert_eq!(drop_superseded_summaries(&mut history), 2);

    let roles: Vec<Role> = history.iter().map(|m| m.role.clone()).collect();
    assert_eq!(
        roles,
        vec![
            Role::User,
            Role::User,
            Role::User,
            Role::Compaction,
            Role::Assistant,
        ],
        "the superseded summaries go, everything else stays in order"
    );
    assert_eq!(history[3].text_content(), "newest summary");
}

#[test]
fn test_newest_summary_keeps_its_position() {
    let mut history = vec![
        compaction("s", "old"),
        Message::user_text("s", "in between"),
        compaction("s", "new"),
        Message::assistant_text("s", "after"),
    ];

    assert_eq!(drop_superseded_summaries(&mut history), 1);
    assert_eq!(history[1].role, Role::Compaction);
    assert_eq!(history[1].text_content(), "new");
    assert_eq!(history[2].text_content(), "after");
}

#[test]
fn test_adjacent_summaries_collapse_to_one() {
    // Consecutive summaries are exactly what the append-only pre-send path
    // produces when it fires twice in a row.
    let mut history = vec![
        compaction("s", "first"),
        compaction("s", "second"),
        compaction("s", "third"),
    ];

    assert_eq!(drop_superseded_summaries(&mut history), 2);
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text_content(), "third");
}
