//! Tests for the Context panel's frontend-neutral lines
//! (`codemason_app_core::context_panel`), the rows the removed terminal
//! UI's `render_context_panel` drew and the GUI Context pane now draws.

use codemason_app_core::app::ContextPartitionSnapshot;
use codemason_app_core::context_panel::{context_panel_lines, format_tokens, progress_bar};

/// A snapshot with the given partitions and window.
fn snapshot(window: Option<usize>) -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: 12_000,
        tool_catalog_tokens: 5_000,
        tool_metadata_tokens: 600,
        history_tokens: 2_400,
        history_message_count: 3,
        skills_tokens: 100,
        memory_tokens: 50,
        agents_md_tokens: 0,
        context_window_tokens: window,
        last_input_tokens: 0,
    }
}

/// The plain text of every line.
fn texts(snapshot: &ContextPartitionSnapshot, out: u64, inn: u64) -> Vec<String> {
    context_panel_lines(snapshot, out, inn, 10)
        .iter()
        .map(codemason_app_core::styled::StyledLine::text)
        .collect()
}

#[test]
fn test_format_tokens_compacts_like_the_tui() {
    assert_eq!(format_tokens(123), "123");
    assert_eq!(format_tokens(12_300), "12.3k");
    assert_eq!(format_tokens(1_200_000), "1.2M");
}

#[test]
fn test_progress_bar_is_ascii_and_bounded() {
    assert_eq!(progress_bar(0.5, 10), "[#####.....]");
    assert_eq!(progress_bar(2.0, 4), "[####]");
    assert_eq!(progress_bar(-1.0, 4), "[....]");
}

#[test]
fn test_lines_list_every_partition_with_percentages() {
    let lines = texts(&snapshot(Some(128_000)), 0, 0);
    let all = lines.join("\n");
    for expected in [
        "Context window",
        "128.0k",
        "Sent to model",
        "System prompt",
        "skills",
        "memory",
        "agents.md",
        "Tool catalog",
        "Tool metadata",
        "History",
        "3 messages",
        "Total",
        "Free",
        "0.0 KB up  0.0 KB down",
    ] {
        assert!(all.contains(expected), "missing {expected:?} in:\n{all}");
    }
    assert!(all.contains('%'), "percentages with a known window:\n{all}");
    assert!(!all.contains("unknown"), "no unknown ratios:\n{all}");
}

#[test]
fn test_lines_read_unknown_without_a_window() {
    let all = texts(&snapshot(None), 2048, 1024).join("\n");
    assert!(all.contains("Context window"), "{all}");
    assert!(all.contains("unknown"), "FR-011 unknown ratios: {all}");
    assert!(
        all.lines()
            .any(|l| l.starts_with("Free") && l.trim_end().ends_with("unknown")),
        "{all}"
    );
    assert!(
        all.lines()
            .any(|l| l.starts_with("System prompt") && l.chars().any(|c| c.is_ascii_digit())),
        "absolute counts remain: {all}"
    );
    assert!(all.contains("2.0 KB up  1.0 KB down"), "{all}");
}
