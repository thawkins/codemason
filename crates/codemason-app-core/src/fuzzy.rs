//! Fuzzy matching for pickers and the command palette.
//!
//! [`fuzzy_score`] scores a candidate against a query by matching the
//! query's characters in order (a subsequence, ignoring case): each matched
//! character scores a point, more at the start of a word and more again
//! right after the previous match, so "ns" ranks "new session" above
//! "transcript". [`fuzzy_rank`] keeps the matching items, best first, in
//! their original order among equal scores.
//!
//! No dependencies.

/// Points for each matched character.
const MATCH: u32 = 1;

/// Extra points for a match at the start of a word.
const WORD_START: u32 = 5;

/// Extra points for a match right after the previous one.
const CONSECUTIVE: u32 = 3;

/// How well `candidate` matches `query`.
///
/// # Returns
///
/// `Some(score)` when the query's characters appear in `candidate` in order
/// (ignoring case), higher for a better match; `Some(0)` for an empty
/// query; `None` when they do not all appear in order.
#[must_use]
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32> {
    let wanted: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return Some(0);
    }
    let mut score = 0;
    let mut next = 0usize;
    let mut previous: Option<usize> = None;
    let mut prior: Option<char> = None;
    for (index, c) in candidate.chars().enumerate() {
        let lower = c.to_lowercase().next().unwrap_or(c);
        if next < wanted.len() && lower == wanted[next] {
            score += MATCH;
            if prior.is_none_or(|p| !p.is_alphanumeric()) {
                score += WORD_START;
            }
            if previous.is_some_and(|p| p + 1 == index) {
                score += CONSECUTIVE;
            }
            previous = Some(index);
            next += 1;
        }
        prior = Some(c);
    }
    (next == wanted.len()).then_some(score)
}

/// The items of `items` whose `key` matches `query`, best first.
///
/// Items with equal scores keep their order in `items`, so a caller's own
/// order (recent first, catalog order) breaks ties. An empty query keeps
/// every item in order.
#[must_use]
pub fn fuzzy_rank<'a, T>(query: &str, items: &'a [T], key: impl Fn(&T) -> &str) -> Vec<&'a T> {
    let mut scored: Vec<(u32, &T)> = items
        .iter()
        .filter_map(|item| fuzzy_score(query, key(item)).map(|score| (score, item)))
        .collect();
    // `sort_by` is stable: equal scores keep their input order.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, item)| item).collect()
}
