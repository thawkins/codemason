//! Tests for `Storage::delete_messages_by_id` (the `/undo` storage step).

use codemason_storage::storage::Storage;
use codemason_types::message::Message;

#[test]
fn test_delete_messages_by_id_removes_only_the_named_messages() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/p").unwrap();
    let keep = Message::user_text("s1", "keep me findable");
    let drop_a = Message::user_text("s1", "drop alpha");
    let drop_b = Message::user_text("s1", "drop beta");
    for m in [&keep, &drop_a, &drop_b] {
        storage.create_message(m).unwrap();
    }

    let deleted = storage
        .delete_messages_by_id("s1", &[drop_a.id.clone(), drop_b.id.clone()])
        .unwrap();

    assert_eq!(deleted, 2);
    let left = storage.get_messages("s1").unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, keep.id);
    // A deleted message can be created again (redo), under the same id.
    storage.create_message(&drop_a).unwrap();
    assert_eq!(storage.get_messages("s1").unwrap().len(), 2);
}

#[test]
fn test_delete_messages_by_id_is_scoped_to_the_session() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/p").unwrap();
    storage.create_session("s2", "/tmp/p").unwrap();
    let other = Message::user_text("s2", "other session");
    storage.create_message(&other).unwrap();

    let deleted = storage
        .delete_messages_by_id("s1", &[other.id.clone(), "no-such-id".to_string()])
        .unwrap();

    assert_eq!(deleted, 0);
    assert_eq!(storage.get_messages("s2").unwrap().len(), 1);
}
