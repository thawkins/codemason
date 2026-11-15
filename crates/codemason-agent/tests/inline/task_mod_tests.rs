//! Relocated inline tests for `task/mod.rs` (ANTIPAT M2 test relocation).
//!
//! The body previously lived in an inline `#[cfg(test)] mod tests` in the
//! source file; it is now compiled from this file via a `#[path]` hook.

use super::*;

#[test]
fn test_task_status_serialization() {
    let status = TaskStatus::Running;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"running\"");

    let status: TaskStatus = serde_json::from_str("\"completed\"").unwrap();
    assert_eq!(status, TaskStatus::Completed);
}

#[test]
fn test_truncate_str_short() {
    assert_eq!(truncate_str("hello", 10).as_ref(), "hello");
}

#[test]
fn test_truncate_str_exact() {
    assert_eq!(truncate_str("hello", 5).as_ref(), "hello");
}

#[test]
fn test_truncate_str_long() {
    let result = truncate_str("hello world", 5);
    assert_eq!(result.as_ref(), "hello...");
}

#[test]
fn test_truncate_str_multibyte_boundary_safe() {
    let s = "caf\u{e9} na\u{ef}ve r\u{e9}sum\u{e9}";
    let result = truncate_str(s, 6);
    assert_eq!(result.as_ref(), "caf\u{e9} n...");
}

#[test]
fn test_truncate_str_multibyte_not_truncated_when_shorter() {
    let s = "na\u{ef}ve";
    let result = truncate_str(s, 10);
    assert_eq!(result.as_ref(), "na\u{ef}ve");
}

#[test]
fn test_task_entry_serialization() {
    let entry = TaskEntry {
        id: "task-1".to_string(),
        parent_session_id: "parent-1".to_string(),
        child_session_id: "child-1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "Find auth code".to_string(),
        background: true,
        detached: false,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::Complete,
        time_bound_secs: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"explore\""));
    assert!(json.contains("\"running\""));
}

// D4 fix: Tests for sanitize_for_id
#[test]
fn test_sanitize_for_id_basic() {
    assert_eq!(sanitize_for_id("explore"), "explore");
    assert_eq!(sanitize_for_id("code-review"), "code-review");
}

#[test]
fn test_sanitize_for_id_with_spaces() {
    assert_eq!(sanitize_for_id("Code Review"), "code-review");
    assert_eq!(sanitize_for_id("  spaced  "), "spaced");
}

#[test]
fn test_sanitize_for_id_with_special_chars() {
    assert_eq!(sanitize_for_id("test@agent"), "test-agent");
    assert_eq!(sanitize_for_id("agent.name"), "agent-name");
}

#[test]
fn test_sanitize_for_id_consecutive_specials() {
    assert_eq!(sanitize_for_id("a--b"), "a-b");
    assert_eq!(sanitize_for_id("a---b"), "a-b");
}

#[test]
fn test_sanitize_for_id_trims_leading_trailing() {
    assert_eq!(sanitize_for_id("-leading"), "leading");
    assert_eq!(sanitize_for_id("trailing-"), "trailing");
}

#[test]
fn test_sanitize_for_id_empty_fallback() {
    assert_eq!(sanitize_for_id(""), "task");
    assert_eq!(sanitize_for_id("---"), "task");
}

#[test]
fn test_sanitize_for_id_length_limit() {
    let long = "a".repeat(50);
    let result = sanitize_for_id(&long);
    assert!(result.len() <= 20, "Result should be limited to 20 chars");
}

/// AA-01: a non-ASCII agent name used to panic in `String::truncate(20)`
/// because byte 20 fell inside a multi-byte char.
#[test]
fn test_sanitize_for_id_non_ascii_cuts_at_char_boundary() {
    let name = "\u{65e5}\u{672c}\u{8a9e}\u{30a8}\u{30fc}\u{30b8}\u{30a7}\u{30f3}\u{30c8}\u{30c6}\u{30b9}\u{30c8}";
    let result = sanitize_for_id(name);
    assert!(result.len() <= 20, "result must stay within 20 bytes");
    assert!(
        name.starts_with(&result),
        "result must be a prefix of the name"
    );
    assert_eq!(result.chars().count(), 6, "3-byte chars: 6 fit in 20 bytes");
    let id = make_task_id(name);
    assert!(
        id.starts_with(&result),
        "task id uses the sanitized prefix: {id}"
    );
}

/// The `SubagentComplete` event `summary` remains truncated to 2000
/// chars for TUI display - this is separate from the full result.
#[test]
fn test_event_summary_is_short() {
    let long = "z".repeat(10_000);
    let summary = truncate_str(&long, 2000).into_owned();
    assert!(summary.len() < long.len());
    assert!(summary.ends_with("..."));
}
