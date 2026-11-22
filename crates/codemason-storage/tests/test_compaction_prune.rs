//! Tests for pruning superseded compaction summaries.
//!
//! Each compaction is built with the previous summary as its anchor, so a later
//! summary already merges in everything still true from the earlier ones. The
//! pre-send compaction path nevertheless only *appends*, which leaves every
//! superseded summary in the transcript for the loader to replay as a separate
//! assistant turn. These tests pin the delete that removes them, and the count
//! that backs its `--dry-run`.

use codemason_storage::storage::Storage;
use codemason_types::message::{Message, MessagePart, Role};

/// Seed `texts` as compaction summaries an hour apart, oldest first, and return
/// their ids in the same order.
fn seed_summaries(storage: &Storage, session_id: &str, texts: &[&str]) -> Vec<String> {
    let base = chrono::Utc::now() - chrono::Duration::hours(i64::try_from(texts.len()).unwrap());
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let mut msg = Message {
                id: format!("{session_id}-summary-{index}"),
                session_id: session_id.to_string(),
                role: Role::Compaction,
                parts: vec![MessagePart::Text {
                    text: (*text).to_string(),
                }],
                created_at: base + chrono::Duration::minutes(i64::try_from(index).unwrap()),
                updated_at: base + chrono::Duration::minutes(i64::try_from(index).unwrap()),
                edit_seq: 0,
            };
            msg.touch();
            storage.create_message(&msg).expect("seed summary");
            msg.id
        })
        .collect()
}

#[test]
fn test_delete_keeps_newest_summary_only() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/project").unwrap();
    seed_summaries(&storage, "s1", &["oldest", "middle", "newest"]);

    assert_eq!(storage.delete_compaction_messages("s1", 1).unwrap(), 2);

    let remaining = storage.get_messages("s1").unwrap();
    assert_eq!(remaining.len(), 1, "only the newest summary survives");
    assert_eq!(remaining[0].text_content(), "newest");
}

#[test]
fn test_delete_all_summaries_leaves_other_messages() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s2", "/tmp/project").unwrap();
    storage
        .create_message(&Message::user_text("s2", "the question"))
        .unwrap();
    seed_summaries(&storage, "s2", &["first", "second"]);
    storage
        .create_message(&Message::assistant_text("s2", "the answer"))
        .unwrap();

    // keep = 0 drops every summary and nothing else.
    assert_eq!(storage.delete_compaction_messages("s2", 0).unwrap(), 2);

    let text: Vec<String> = storage
        .get_messages("s2")
        .unwrap()
        .iter()
        .map(Message::text_content)
        .collect();
    assert_eq!(text, vec!["the question", "the answer"]);
}

#[test]
fn test_count_matches_delete_and_leaves_rows() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s3", "/tmp/project").unwrap();
    seed_summaries(&storage, "s3", &["a", "b", "c", "d"]);

    for keep in 0..=4 {
        assert_eq!(
            storage.count_compaction_messages("s3", keep).unwrap(),
            4 - keep,
            "keep={keep} counts the summaries past the retained ones"
        );
    }

    // A dry-run never deletes.
    assert_eq!(storage.get_messages("s3").unwrap().len(), 4);
    assert_eq!(storage.delete_compaction_messages("s3", 1).unwrap(), 3);
    assert_eq!(storage.get_messages("s3").unwrap().len(), 1);
}

#[test]
fn test_prune_is_scoped_to_one_session() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("mine", "/tmp/project").unwrap();
    storage.create_session("theirs", "/tmp/project").unwrap();
    seed_summaries(&storage, "mine", &["mine-1", "mine-2"]);
    seed_summaries(&storage, "theirs", &["theirs-1", "theirs-2"]);

    assert_eq!(storage.delete_compaction_messages("mine", 1).unwrap(), 1);
    assert_eq!(storage.get_messages("mine").unwrap().len(), 1);
    assert_eq!(
        storage.get_messages("theirs").unwrap().len(),
        2,
        "another session's summaries are untouched"
    );
}

#[test]
fn test_prune_fewer_summaries_than_kept_is_a_no_op() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s4", "/tmp/project").unwrap();
    seed_summaries(&storage, "s4", &["only"]);

    assert_eq!(storage.count_compaction_messages("s4", 1).unwrap(), 0);
    assert_eq!(storage.delete_compaction_messages("s4", 1).unwrap(), 0);
    assert_eq!(storage.get_messages("s4").unwrap().len(), 1);

    // A session with no summaries at all is also a clean no-op.
    storage.create_session("s5", "/tmp/project").unwrap();
    assert_eq!(storage.delete_compaction_messages("s5", 1).unwrap(), 0);
}

#[test]
fn test_prune_removes_deleted_summaries_from_search() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s6", "/tmp/project").unwrap();
    seed_summaries(
        &storage,
        "s6",
        &["obsolete quokka phrase", "current wallaby phrase"],
    );

    assert_eq!(storage.delete_compaction_messages("s6", 1).unwrap(), 1);

    let hits = storage
        .search_conversation("s6", "quokka", 10)
        .expect("search after prune");
    assert!(
        hits.is_empty(),
        "a pruned summary must not linger in full-text search: {hits:?}"
    );
    let kept = storage
        .search_conversation("s6", "wallaby", 10)
        .expect("search kept summary");
    assert_eq!(kept.len(), 1, "the retained summary stays searchable");
}
