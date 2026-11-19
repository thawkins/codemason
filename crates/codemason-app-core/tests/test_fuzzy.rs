//! Tests for `codemason_app_core::fuzzy`.

use codemason_app_core::fuzzy::{fuzzy_rank, fuzzy_score};

#[test]
fn test_fuzzy_score_prefers_word_starts_and_runs() {
    let ns_new = fuzzy_score("ns", "new session").expect("match");
    let ns_trans = fuzzy_score("ns", "transcript").expect("match");
    assert!(ns_new > ns_trans, "{ns_new} <= {ns_trans}");
    assert_eq!(fuzzy_score("xyz", "new session"), None);
    assert_eq!(fuzzy_score("", "anything"), Some(0));
    // Case is ignored.
    assert!(fuzzy_score("NS", "new session").is_some());
    // Order matters.
    assert_eq!(fuzzy_score("sn", "new s"), None);
    assert!(fuzzy_score("theme nord", "Theme: nord").is_some());
}

#[test]
fn test_fuzzy_rank_is_best_first_and_stable() {
    let items = ["transcript", "new session", "next step", "unrelated"];
    let ranked = fuzzy_rank("ns", &items, |s| s);
    assert_eq!(ranked.first().copied(), Some(&"new session"));
    assert!(!ranked.contains(&&"unrelated"));
    // Equal scores keep input order.
    let same = ["aa", "ab", "ac"];
    assert_eq!(fuzzy_rank("a", &same, |s| s), [&"aa", &"ab", &"ac"]);
    assert_eq!(fuzzy_rank("", &same, |s| s).len(), 3);
}
