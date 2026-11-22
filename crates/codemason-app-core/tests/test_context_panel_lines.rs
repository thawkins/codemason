//! Tests for the Context panel's frontend-neutral view model
//! (`codemason_app_core::context_panel`), which the GUI Context pane draws.

use codemason_app_core::app::{ContextItem, ContextItems, ContextPartitionSnapshot};
use codemason_app_core::context_panel::{
    ContextCategory, context_view, format_percent, format_tokens,
};

/// A snapshot with the given partitions and window.
fn snapshot(window: Option<usize>) -> ContextPartitionSnapshot {
    ContextPartitionSnapshot {
        system_prompt_tokens: 12_000,
        tool_catalog_tokens: 5_000,
        tool_metadata_tokens: 600,
        history_tokens: 2_400,
        history_message_count: 3,
        memory_message_count: 3,
        skills_tokens: 100,
        memory_tokens: 50,
        agents_md_tokens: 0,
        context_window_tokens: window,
        last_input_tokens: 0,
        items: ContextItems {
            mcp_tools: vec![ContextItem {
                name: "github: create_issue".to_string(),
                tokens: 1_000,
            }],
            ..ContextItems::default()
        },
    }
}

/// Every line of the view's text, joined.
fn text(snapshot: &ContextPartitionSnapshot, out: u64, inn: u64) -> String {
    context_view(snapshot, out, inn).text_lines().join("\n")
}

#[test]
fn test_format_tokens_compacts_like_the_tui() {
    assert_eq!(format_tokens(123), "123");
    assert_eq!(format_tokens(12_300), "12.3k");
    assert_eq!(format_tokens(1_200_000), "1.2M");
}

#[test]
fn test_format_percent_reads_unknown_without_a_window() {
    assert_eq!(format_percent(Some(6.14)), "6.1%");
    assert_eq!(format_percent(None), "unknown");
}

#[test]
fn test_view_rows_do_not_overlap_and_sum_to_the_total() {
    let snap = snapshot(Some(128_000));
    let view = context_view(&snap, 0, 0);
    let sum: u64 = view.rows.iter().map(|row| row.tokens).sum();
    assert_eq!(sum, snap.total_tokens());
    assert_eq!(view.used_tokens, snap.total_tokens());
    let tokens = |category| {
        view.rows
            .iter()
            .find(|row| row.category == category)
            .map(|row| row.tokens)
    };
    assert_eq!(tokens(ContextCategory::SystemPrompt), Some(12_000 - 150));
    assert_eq!(tokens(ContextCategory::McpTools), Some(1_000));
    assert_eq!(tokens(ContextCategory::BuiltInTools), Some(4_000));
    assert_eq!(view.rows.len(), ContextCategory::ALL.len());
}

#[test]
fn test_view_reports_usage_and_free_space_of_a_known_window() {
    let view = context_view(&snapshot(Some(100_000)), 0, 0);
    assert_eq!(view.window_tokens, Some(100_000));
    assert_eq!(view.free_tokens, Some(100_000 - 20_000));
    assert_eq!(view.usage_text(), "20% used  20.0k / 100.0k");
    let free = view.free_percent.unwrap_or_default();
    assert!((free - 80.0).abs() < 1e-9, "{free}");
}

#[test]
fn test_text_lists_every_partition_with_percentages() {
    let all = text(&snapshot(Some(128_000)), 0, 0);
    for expected in [
        "Context window",
        "128.0k",
        "Sent to model",
        "System prompt",
        "Skills",
        "Memory",
        "Instruction files",
        "Built-in tools",
        "MCP tools",
        "  github: create_issue  1.0k",
        "Tool metadata",
        "History",
        "3 messages",
        "Free space",
        "0.0 KB up  0.0 KB down",
    ] {
        assert!(all.contains(expected), "missing {expected:?} in:\n{all}");
    }
    assert!(all.contains('%'), "percentages with a known window:\n{all}");
    assert!(!all.contains("unknown"), "no unknown ratios:\n{all}");
}

#[test]
fn test_text_reads_unknown_without_a_window() {
    let all = text(&snapshot(None), 2048, 1024);
    assert!(all.contains("window unknown"), "{all}");
    assert!(
        all.lines()
            .any(|l| l.starts_with("Free space") && l.trim_end().ends_with("unknown")),
        "{all}"
    );
    assert!(
        all.lines()
            .any(|l| l.starts_with("System prompt") && l.chars().any(|c| c.is_ascii_digit())),
        "absolute counts remain: {all}"
    );
    assert!(all.contains("2.0 KB up  1.0 KB down"), "{all}");
}
