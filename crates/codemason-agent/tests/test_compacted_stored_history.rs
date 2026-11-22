//! Tests for `compaction::runner::compacted_stored_history`, the history a
//! pre-send compaction stores in place of the one it summarised.

use codemason_agent::compaction::runner::compacted_stored_history;
use codemason_agent::message::{Message, Role};
use codemason_config::compaction::CompactionConfig;

const SESSION: &str = "s";

/// A user message of about `kb` kilobytes.
fn user(text: &str, kb: usize) -> Message {
    Message::user_text(SESSION, format!("{text} {}", "x".repeat(kb * 1024)))
}

/// A summary message.
fn summary(text: &str) -> Message {
    Message::new(
        SESSION,
        Role::Compaction,
        Message::user_text(SESSION, text).parts,
    )
}

/// A 10k-token window keeping 20%: a 2k-token budget.
fn config() -> (CompactionConfig, usize) {
    let mut config = CompactionConfig::default();
    config.keep.tokens = Some(0.2);
    (config, 10_000)
}

#[test]
fn test_compacted_stored_history_keeps_the_turn_rows_past_the_budget() {
    // The turn's placeholder is empty and an inbox message after it is far
    // over the keep budget: both must survive, or the turn's updates of its
    // assistant row match nothing.
    let (config, window) = config();
    let old: Vec<Message> = (0..6).map(|i| user(&format!("old {i}"), 4)).collect();
    let ask = user("this turn's question", 1);
    let placeholder = Message::new(SESSION, Role::Assistant, vec![]);
    let inbox = user("big inbox message", 40);
    let mut stored = old.clone();
    stored.extend([ask.clone(), placeholder.clone(), inbox.clone()]);

    let kept = compacted_stored_history(stored, &summary("sum"), &config, window, &placeholder.id);

    let ids: Vec<&str> = kept.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(kept[0].role, Role::Compaction, "the summary leads");
    assert!(ids.contains(&placeholder.id.as_str()), "placeholder kept");
    assert!(ids.contains(&inbox.id.as_str()), "inbox kept");
    assert!(
        ids.contains(&ask.id.as_str()),
        "the last earlier row is kept"
    );
    assert!(!ids.contains(&old[0].id.as_str()), "the head is dropped");
    assert!(
        kept.windows(2).all(|w| w[0].created_at <= w[1].created_at),
        "oldest first, so the summary loads first"
    );
}

#[test]
fn test_compacted_stored_history_drops_earlier_summaries() {
    let (config, window) = config();
    let placeholder = Message::new(SESSION, Role::Assistant, vec![]);
    let stored = vec![
        summary("earlier summary"),
        user("old", 4),
        user("question", 1),
        placeholder.clone(),
    ];

    let kept = compacted_stored_history(stored, &summary("new"), &config, window, &placeholder.id);

    let summaries: Vec<String> = kept
        .iter()
        .filter(|m| m.role == Role::Compaction)
        .map(Message::text_content)
        .collect();
    assert_eq!(summaries, ["new"]);
}

#[test]
fn test_compacted_stored_history_without_the_placeholder_keeps_a_tail() {
    let (config, window) = config();
    let stored: Vec<Message> = (0..6).map(|i| user(&format!("m{i}"), 4)).collect();
    let last = stored[5].id.clone();

    let kept = compacted_stored_history(stored, &summary("s"), &config, window, "missing");

    assert_eq!(kept[0].role, Role::Compaction);
    assert_eq!(kept.last().map(|m| m.id.clone()), Some(last));
    assert!(kept.len() < 7, "the head is dropped: {}", kept.len());
}
