//! Tests for `Storage::replace_messages` (compaction's history swap).

use codemason_storage::storage::Storage;
use codemason_types::message::Message;

#[test]
fn test_replace_messages_swaps_the_whole_history() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/p").unwrap();
    storage.create_session("s2", "/tmp/p").unwrap();
    for text in ["old one", "old two"] {
        storage
            .create_message(&Message::user_text("s1", text))
            .unwrap();
    }
    storage
        .create_message(&Message::user_text("s2", "other session"))
        .unwrap();
    let kept = Message::user_text("s1", "summary");
    let tail = Message::user_text("s1", "recent");

    storage
        .replace_messages("s1", &[kept.clone(), tail.clone()])
        .unwrap();

    let ids: Vec<String> = storage
        .get_messages("s1")
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, [kept.id, tail.id]);
    assert_eq!(storage.get_messages("s2").unwrap().len(), 1, "scoped");
}

#[test]
fn test_replace_messages_is_all_or_nothing() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/p").unwrap();
    let original = Message::user_text("s1", "original");
    storage.create_message(&original).unwrap();
    let fresh = Message::user_text("s1", "fresh");

    // The same id twice: the second insert fails, so the delete of the
    // original history must roll back with it.
    let result = storage.replace_messages("s1", &[fresh.clone(), fresh]);

    assert!(result.is_err());
    let left = storage.get_messages("s1").unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, original.id, "the history is unchanged");
}
