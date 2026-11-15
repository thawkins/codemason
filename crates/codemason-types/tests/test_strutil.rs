//! Integration tests for `codemason-types` string truncation utilities.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/strutil.rs`
//! (T-010 of the testconsolidate spec). All tested functions are public.

use codemason_types::strutil::{truncate_bytes, truncate_chars};

#[test]
fn test_truncate_chars_noop() {
    assert_eq!(truncate_chars("hello", 10), "hello");
}

#[test]
fn test_truncate_chars_ascii() {
    assert_eq!(truncate_chars("hello world", 5), "he...");
}

#[test]
fn test_truncate_chars_multibyte() {
    // Budget now includes the 3-char ellipsis: 5 chars -> 2 head + "..."
    assert_eq!(truncate_chars("café résumé", 5), "ca...");
}

#[test]
fn test_truncate_chars_en_dash() {
    assert_eq!(truncate_chars("A – B C", 4), "A...");
}

#[test]
fn test_truncate_bytes_noop() {
    assert_eq!(truncate_bytes("hello", 10), "hello");
}

#[test]
fn test_truncate_bytes_ascii() {
    assert_eq!(truncate_bytes("hello world", 3), "hel...");
}

#[test]
fn test_truncate_bytes_boundary_adjustment() {
    assert_eq!(truncate_bytes("café", 3), "caf...");
}

#[test]
fn test_truncate_bytes_en_dash() {
    let result = truncate_bytes("A – B", 2);
    assert!(result.ends_with("..."));
    assert!(result.starts_with('A'));
}

#[test]
fn test_truncate_bytes_em_dash_at_400_boundary() {
    // Regression test for a panic where a tool-result preview sliced at a
    // fixed 400-byte index that fell inside a 3-byte em dash (bytes 398..401).
    // `truncate_bytes` must step back to the previous character boundary.
    let prefix = "a".repeat(398);
    let input = format!("{prefix}\u{2014}more text after");
    let result = truncate_bytes(&input, 400);
    // The 3-byte em dash at bytes 398..401 is cut; the suffix is the ASCII
    // ellipsis (M1 switched the marker away from U+2014).
    assert_eq!(result, format!("{prefix}..."));
}

#[test]
fn test_strutil_lowercase_range_to_original_maps_length_changes_back() {
    use codemason_types::strutil::lowercase_range_to_original;
    // U+0130 lowercases to 3 bytes; the match sits after three of them.
    let text = "\u{130}\u{130}\u{130} needle";
    let lowered = text.to_lowercase();
    let start = lowered.find("needle").expect("match");
    let (s, e) = lowercase_range_to_original(text, start, start + "needle".len());
    assert_eq!(&text[s..e], "needle");
    // ASCII: identical offsets; a word-final sigma keeps its byte length.
    assert_eq!(lowercase_range_to_original("abc", 1, 2), (1, 2));
    let greek = "x \u{39F}\u{3A3} y";
    let lowered = greek.to_lowercase();
    let start = lowered.find("\u{3BF}\u{3C2}").expect("final sigma form");
    let (s, e) = lowercase_range_to_original(greek, start, start + "\u{3BF}\u{3C2}".len());
    assert_eq!(&greek[s..e], "\u{39F}\u{3A3}");
    // A range at the very end maps to the end.
    assert_eq!(lowercase_range_to_original("ab", 2, 2), (2, 2));
}
