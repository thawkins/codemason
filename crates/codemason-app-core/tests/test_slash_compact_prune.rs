//! `/compact prune [--all] [--dry-run]` slash-command tests.
//!
//! The pre-send compaction path appends a new `Role::Compaction` summary every
//! time the context window fills and never removes the one it superseded, so a
//! long-lived session accumulates several summary blocks. Each compaction
//! anchors on the previous one, so only the newest is meaningful. `/compact
//! prune` deletes the superseded copies; `--all` deletes every summary and has
//! to be confirmed; `--dry-run` reports the count and deletes nothing.

use std::sync::Arc;

use support::make_app;

mod support;

use codemason_types::message::{Message, MessagePart, Role};

/// Seed `texts` as compaction summaries an hour apart, oldest first.
fn seed_summaries(storage: &codemason_storage::Storage, session_id: &str, texts: &[&str]) {
    storage
        .create_session(session_id, "/tmp/compact-prune-test")
        .expect("create_session must succeed");
    let base = chrono::Utc::now() - chrono::Duration::hours(i64::try_from(texts.len()).unwrap());
    for (index, text) in texts.iter().enumerate() {
        let stamp = base + chrono::Duration::minutes(i64::try_from(index).unwrap());
        let msg = Message {
            id: format!("{session_id}-summary-{index}"),
            session_id: session_id.to_string(),
            role: Role::Compaction,
            parts: vec![MessagePart::Text {
                text: (*text).to_string(),
            }],
            created_at: stamp,
            updated_at: stamp,
            edit_seq: 0,
        };
        storage
            .create_message(&msg)
            .expect("create_message must succeed");
    }
}

/// The summaries currently stored for `session_id`, in stored order.
fn summaries(storage: &codemason_storage::Storage, session_id: &str) -> Vec<String> {
    storage
        .get_messages(session_id)
        .expect("get_messages must succeed")
        .into_iter()
        .filter(|m| m.role == Role::Compaction)
        .map(|m| m.text_content())
        .collect()
}

#[test]
fn test_compact_registered_in_slash_commands() {
    let def = codemason_app_core::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "compact")
        .expect("compact must be registered in SLASH_COMMANDS");
    assert!(
        def.description.contains("prune"),
        "description must advertise the prune subcommand: {}",
        def.description
    );
    assert!(
        def.description.contains("--all") && def.description.contains("--dry-run"),
        "description must advertise both flags: {}",
        def.description
    );
}

#[tokio::test]
async fn test_compact_prune_keeps_newest_summary() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["oldest", "middle", "newest"]);

    app.execute_slash_command("/compact prune").await;

    assert_eq!(
        summaries(&storage, "test-session"),
        vec!["newest".to_string()],
        "the default form keeps only the newest summary"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("prune should report a message")
        .text_content();
    assert!(
        text.contains("Deleted **2**"),
        "the report must name the count, got: {text}"
    );
}

#[tokio::test]
async fn test_compact_prune_relabels_when_nothing_to_do() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["only"]);

    app.execute_slash_command("/compact prune").await;

    assert_eq!(
        summaries(&storage, "test-session"),
        vec!["only".to_string()],
        "a lone summary is already the newest"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("prune should report a message")
        .text_content();
    assert!(
        text.contains("Deleted **0**"),
        "the report must still be produced, got: {text}"
    );
}

#[tokio::test]
async fn test_compact_prune_dry_run_deletes_nothing() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b", "c"]);

    app.execute_slash_command("/compact prune --dry-run").await;

    assert_eq!(
        summaries(&storage, "test-session"),
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        "--dry-run must delete nothing"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("dry run should create a message")
        .text_content();
    assert!(
        text.contains("Would delete **2**"),
        "dry run must report the count, got: {text}"
    );
    assert!(
        !app.compact_prune_confirm_open,
        "--dry-run must not arm the confirmation dialog"
    );
}

#[tokio::test]
async fn test_compact_prune_all_arms_confirmation_and_deletes_only_on_yes() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b", "c"]);

    app.execute_slash_command("/compact prune --all").await;

    assert!(
        app.compact_prune_confirm_open,
        "`--all` must arm the confirmation dialog"
    );
    assert!(
        !app.compact_prune_confirm_is_yes(),
        "the dialog must open on `No` so Enter cannot delete"
    );
    assert_eq!(
        summaries(&storage, "test-session").len(),
        3,
        "nothing may be deleted before the user confirms"
    );
}

#[tokio::test]
async fn test_compact_prune_all_dry_run_counts_every_summary() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b", "c"]);

    app.execute_slash_command("/compact prune --all --dry-run")
        .await;

    assert!(
        !app.compact_prune_confirm_open,
        "--dry-run must not arm the confirmation dialog"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("dry run should create a message")
        .text_content();
    assert!(
        text.contains("Would delete **3**"),
        "--all counts every summary, got: {text}"
    );
}

#[tokio::test]
async fn test_compact_prune_all_deletes_every_summary_when_confirmed() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b", "c"]);
    storage
        .create_message(&Message::user_text("test-session", "keeping this turn"))
        .expect("create_message must succeed");

    app.open_compact_prune_confirm();
    app.compact_prune_confirm_toggle();
    assert!(app.compact_prune_confirm_is_yes());
    app.run_compaction_prune(0, true);

    assert!(
        summaries(&storage, "test-session").is_empty(),
        "a confirmed `--all` deletes every summary"
    );
    let remaining: Vec<String> = storage
        .get_messages("test-session")
        .expect("get_messages must succeed")
        .into_iter()
        .map(|m| m.text_content())
        .collect();
    assert_eq!(
        remaining,
        vec!["keeping this turn".to_string()],
        "ordinary messages survive a summary prune"
    );
}

#[tokio::test]
async fn test_compact_prune_refreshes_the_visible_transcript() {
    // The in-memory list must match storage after the delete, or the transcript
    // the user sees disagrees with the one the next request replays.
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["oldest", "newest"]);
    app.session.messages = storage.get_messages("test-session").expect("load");

    app.execute_slash_command("/compact prune").await;

    let in_memory: Vec<String> = app
        .session
        .messages
        .iter()
        .filter(|m| m.role == Role::Compaction)
        .map(|m| m.text_content())
        .collect();
    assert_eq!(
        in_memory,
        vec!["newest".to_string()],
        "the visible transcript must show the pruned result"
    );
}

#[tokio::test]
async fn test_compact_prune_rejects_unknown_flag() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b"]);

    app.execute_slash_command("/compact prune --bogus").await;

    assert_eq!(
        summaries(&storage, "test-session").len(),
        2,
        "an unknown flag must delete nothing"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("usage should be reported")
        .text_content();
    assert!(
        text.contains("--bogus") && text.contains("Usage"),
        "the usage error must name the bad flag, got: {text}"
    );
}

#[tokio::test]
async fn test_compact_unknown_subcommand_is_rejected() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["a", "b"]);

    app.execute_slash_command("/compact bogus").await;

    assert_eq!(
        summaries(&storage, "test-session").len(),
        2,
        "an unknown subcommand must delete nothing"
    );
    let text = app
        .session
        .messages
        .last()
        .expect("usage should be reported")
        .text_content();
    assert!(
        text.contains("Usage") && text.contains("prune"),
        "the usage error must list the prune form, got: {text}"
    );
}

#[tokio::test]
async fn test_compact_prune_is_scoped_to_the_active_session() {
    let mut app = make_app();
    app.session.session_id = Some("test-session".to_string());
    let storage = Arc::clone(&app.storage);
    seed_summaries(&storage, "test-session", &["mine-1", "mine-2"]);
    seed_summaries(&storage, "other-session", &["theirs-1", "theirs-2"]);

    app.execute_slash_command("/compact prune").await;

    assert_eq!(
        summaries(&storage, "test-session"),
        vec!["mine-2".to_string()],
        "the active session keeps its newest summary"
    );
    assert_eq!(
        summaries(&storage, "other-session").len(),
        2,
        "another session's summaries are untouched"
    );
}
