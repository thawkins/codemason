//! Tests for `readable_names`: team-member names are
//! `<agent-type>-<adjective>-<surname>` and never collide.

use codemason_agent::readable_names::readable_member_name;
use std::collections::HashSet;

#[test]
fn test_readable_names_member_name_is_prefixed_by_agent_type() {
    let name = readable_member_name("explore", |_| false);
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 3, "{name}");
    assert_eq!(parts[0], "explore");
    assert!(
        parts[1..]
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase())),
        "{name}"
    );
}

#[test]
fn test_readable_names_member_name_slugs_the_agent_type() {
    let name = readable_member_name("Code Reviewer!", |_| false);
    assert!(name.starts_with("code-reviewer-"), "{name}");
    let blank = readable_member_name("  ", |_| false);
    assert!(blank.starts_with("agent-"), "{blank}");
}

#[test]
fn test_readable_names_member_name_avoids_taken_names() {
    let mut taken: HashSet<String> = HashSet::new();
    for _ in 0..200 {
        let name = readable_member_name("general", |n| taken.contains(n));
        assert!(taken.insert(name.clone()), "duplicate {name}");
    }
}

#[test]
fn test_readable_names_member_name_falls_back_to_a_number_when_all_draws_are_taken() {
    // Every unnumbered draw is taken: the fallback appends `-<n>`.
    let name = readable_member_name("general", |n| n.split('-').count() == 3);
    assert_eq!(name.split('-').count(), 4, "{name}");
    assert!(name.ends_with("-2"), "{name}");
}
