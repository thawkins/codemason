//! Tests for the `/research open` markdown viewer rendering.

mod support;

use std::path::PathBuf;

// markdown_to_lines is private in layout.rs, so we test via the public
// render path using a small wrapper that exposes it for tests.
use codemason_tui::layout::markdown_to_lines_testable;

#[test]
fn test_markdown_to_lines_renders_headings() {
    let base = PathBuf::from("/");
    let lines = markdown_to_lines_testable("# Title\n\n## Subtitle\nbody", &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(rendered.iter().any(|l| l.contains("Title")));
    assert!(rendered.iter().any(|l| l.contains("Subtitle")));
    assert!(rendered.iter().any(|l| l.contains("body")));
}

#[test]
fn test_markdown_to_lines_renders_code_block() {
    let base = PathBuf::from("/");
    let md = "```rust\nlet x = 1;\n```";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(rendered.iter().any(|l| l.contains("let x = 1;")));
}

#[test]
fn test_markdown_to_lines_mermaid_block_shows_label() {
    let base = PathBuf::from("/");
    let md = "```mermaid\ngraph TD;\nA-->B;\n```";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("Mermaid diagram")),
        "expected mermaid label, got: {rendered:?}"
    );
    assert!(rendered.iter().any(|l| l.contains("A-->B")));
}

#[test]
fn test_markdown_to_lines_image_renders_placeholder() {
    let base = PathBuf::from("target/temp");
    let md = "![diagram](assets/diagram.png)";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join(" ");
    assert!(
        joined.contains("[Image: diagram"),
        "expected image placeholder, got: {joined}"
    );
}

#[test]
fn test_markdown_to_lines_link_renders_text_and_url() {
    let base = PathBuf::from("/");
    let md = "See [docs](https://example.com/docs).";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join(" ");
    assert!(
        joined.contains("[docs]"),
        "expected link text, got: {joined}"
    );
    assert!(
        joined.contains("(https://example.com/docs)"),
        "expected link url, got: {joined}"
    );
}

#[test]
fn test_markdown_to_lists_renders_bullet() {
    let base = PathBuf::from("/");
    let md = "- first\n- second";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("* first")),
        "expected bullet list, got: {rendered:?}"
    );
}

#[test]
fn test_markdown_to_lines_footer_note_present() {
    let base = PathBuf::from("/");
    let lines = markdown_to_lines_testable("hello", &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("Esc to close")),
        "expected footer note, got: {rendered:?}"
    );
}

/// Characterization: pins the exact ratatui output of the markdown viewer
/// (styles, indents, code blocks, rule, footer) so the move of the renderer
/// into `codemason-app-core` keeps every rule.
#[test]
fn test_markdown_to_lines_exact_styles_are_stable() {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};

    let base = PathBuf::from("/");
    let md = "# H1\n\n## H2\n\n### H3\n\n- item `x`\n\n```\ncode\n```\n\n---\n\n\
              [t](u)\n\n```mermaid\nA-->B\n```\n";
    let lines = markdown_to_lines_testable(md, &base, 10);
    let expected: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            "H1",
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "H2",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "H3",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::raw("* item "),
            Span::styled(
                " `x` ",
                Style::default()
                    .fg(Color::Green)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled("  code", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(
            "\u{2500}".repeat(10),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("[t]", Style::default().fg(Color::White)),
            Span::styled(
                "(u)",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
        Line::from(Span::styled(
            "[Mermaid diagram - rendered as text below]",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("  A-->B", Style::default().fg(Color::Cyan))),
        Line::from(""),
        Line::from(Span::styled(
            "[Esc to close * images shown as placeholders * links are plain text]",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    assert_eq!(lines, expected);
}
