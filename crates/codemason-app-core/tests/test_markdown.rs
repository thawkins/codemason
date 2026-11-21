//! Tests for `codemason_app_core::markdown`, the frontend-neutral markdown
//! renderer shared by the terminal and GUI frontends.

use std::path::Path;

use codemason_app_core::markdown::{autolink_bare_urls, markdown_to_styled};
use codemason_app_core::styled::{TextStyle, ThemeColor};

#[test]
fn test_markdown_code_block_is_monospace() {
    let lines = markdown_to_styled("intro\n\n```rust\nlet x = 1;\n```\n", Path::new("/"), 80);
    let code = lines
        .iter()
        .find(|l| l.text().contains("let x = 1;"))
        .expect("code line rendered");
    assert_eq!(code.text(), "  let x = 1;");
    assert!(code.spans.iter().all(|s| s.style.monospace));
    assert_eq!(code.spans[0].style.fg, Some(ThemeColor::Code));
    let intro = lines
        .iter()
        .find(|l| l.text() == "intro")
        .expect("paragraph rendered");
    assert!(intro.spans.iter().all(|s| !s.style.monospace));
}

#[test]
fn test_markdown_inline_code_is_monospace() {
    let lines = markdown_to_styled("run `cargo` now", Path::new("/"), 80);
    let span = lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .find(|s| s.text == " `cargo` ")
        .expect("inline code span");
    assert!(span.style.monospace);
    assert_eq!(span.style.fg, Some(ThemeColor::Code));
    assert_eq!(span.style.bg, Some(ThemeColor::Raised));
    assert!(span.style.bold);
}

#[test]
fn test_markdown_heading_levels_keep_tui_styles() {
    let lines = markdown_to_styled("# One\n\n## Two\n\n### Three\n", Path::new("/"), 80);
    let style_of = |text: &str| -> TextStyle {
        lines
            .iter()
            .find(|l| l.text() == text)
            .expect("heading rendered")
            .spans[0]
            .style
    };
    assert_eq!(
        style_of("One"),
        TextStyle::new().fg(ThemeColor::Heading).bold().underline()
    );
    assert_eq!(
        style_of("Two"),
        TextStyle::new().fg(ThemeColor::Heading).bold()
    );
    assert_eq!(
        style_of("Three"),
        TextStyle::new().fg(ThemeColor::Text).bold()
    );
}

#[test]
fn test_markdown_ends_with_viewer_footer() {
    let lines = markdown_to_styled("body", Path::new("/"), 80);
    let last = lines.last().expect("footer line");
    assert!(last.text().starts_with("[Esc to close"));
    assert_eq!(
        lines[lines.len() - 2].spans,
        [] as [codemason_app_core::styled::StyledSpan; 0]
    );
}

#[test]
fn test_has_table_finds_only_real_tables() {
    use codemason_app_core::markdown::has_table;

    assert!(has_table("| a | b |\n|---|---|\n| 1 | 2 |\n"));
    assert!(has_table("Intro\n\n| a |\n|---|\n| 1 |\n\nOutro"));
    // A pipe in prose, or a pipe row with no delimiter row, is no table.
    assert!(!has_table("Use `a | b` to pipe."));
    assert!(!has_table("| a | b |\n| 1 | 2 |\n"));
    // A table inside a fenced code block is code.
    assert!(!has_table("```\n| a |\n|---|\n| 1 |\n```\n"));
    assert!(!has_table("no pipes here"));
}

#[test]
fn test_markdown_autolink_bare_url() {
    assert_eq!(
        autolink_bare_urls("PR #77 is open: https://github.com/o/r/pull/77"),
        "PR #77 is open: <https://github.com/o/r/pull/77>"
    );
    assert_eq!(
        autolink_bare_urls("See https://example.com/a. Then (http://x.io/b)."),
        "See <https://example.com/a>. Then (<http://x.io/b>)."
    );
    assert_eq!(
        autolink_bare_urls("- https://en.wikipedia.org/wiki/Rust_(language)"),
        "- <https://en.wikipedia.org/wiki/Rust_(language)>"
    );
}

#[test]
fn test_markdown_autolink_leaves_links_and_code_alone() {
    for text in [
        "no urls here",
        "[PR](https://github.com/o/r/pull/1)",
        "<https://github.com/o/r/pull/1>",
        "`https://github.com/o/r/pull/1`",
        "```\nhttps://github.com/o/r/pull/1\n```",
        "![img](https://example.com/a.png)",
        "https:// alone",
    ] {
        assert_eq!(autolink_bare_urls(text), text, "{text}");
    }
}
