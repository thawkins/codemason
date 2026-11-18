//! Tests for the `/research open` markdown viewer rendering.

mod support;

use std::path::PathBuf;

// markdown_to_lines is private in layout.rs, so we test via the public
// render path using a small wrapper that exposes it for tests.

mod interaction_tests {
    use crate::term_events::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };

    use codemason_app_core::app::ResearchViewState;

    use super::{PathBuf, support};
    use crate::term_events::CrosstermAppExt;
    use codemason_app_core::geometry::Area;

    fn make_research_view() -> ResearchViewState {
        ResearchViewState {
            name: "test".to_string(),
            path: PathBuf::from("target/temp/research/test/RESEARCH.md"),
            base_dir: PathBuf::from("target/temp/research/test"),
            markdown: "line\n".repeat(200),
            scroll_offset: 10,
            max_scroll: 100,
            line_cache: codemason_app_core::app::OutputViewLineCache {
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        }
    }

    fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[tokio::test]
    async fn test_research_view_esc_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await;
        assert!(
            app.research_view.is_none(),
            "Esc should close research view"
        );
    }

    #[tokio::test]
    async fn test_research_view_page_down_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 5);
    }

    #[tokio::test]
    async fn test_research_view_page_up_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 15);
    }

    #[tokio::test]
    async fn test_research_view_down_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 9);
    }

    #[tokio::test]
    async fn test_research_view_up_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 11);
    }

    #[tokio::test]
    async fn test_research_view_ctrl_page_end_jumps_to_end() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 0);
    }

    #[tokio::test]
    async fn test_research_view_ctrl_page_home_jumps_to_start() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL))
            .await;
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 100);
    }

    #[test]
    fn test_research_view_mouse_scroll_inside() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_scroll_down(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 7);

        app.handle_mouse_event(mouse_scroll_up(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 10);
    }

    #[test]
    fn test_research_view_click_outside_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(2, 2));
        assert!(
            app.research_view.is_none(),
            "click outside should close research view"
        );
    }

    #[test]
    fn test_research_view_click_inside_keeps_open() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Area::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(10, 10));
        assert!(
            app.research_view.is_some(),
            "click inside should keep research view open"
        );
    }

    #[tokio::test]
    async fn test_research_view_keys_do_not_type_into_input() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
            .await;
        assert!(app.research_view.is_some());
        assert_eq!(app.session.input, "");
    }
}

/// The research viewer's markdown lines: app-core's `markdown_to_styled`,
/// which the terminal's `markdown_to_lines_testable` wrapped and the GUI's
/// research viewer draws.
fn markdown_to_lines_testable(
    markdown: &str,
    base_dir: &std::path::Path,
    wrap_width: usize,
) -> Vec<codemason_app_core::styled::StyledLine> {
    codemason_app_core::markdown::markdown_to_styled(markdown, base_dir, wrap_width)
}

/// A styled line as a terminal shows it: each non-empty span's text with its
/// colours and bold/italic/underline/dim, without the `monospace` flag.
type TerminalSpan = (
    String,
    Option<codemason_app_core::styled::ThemeColor>,
    Option<codemason_app_core::styled::ThemeColor>,
    [bool; 4],
);

/// [`TerminalSpan`]s of every line.
fn terminal_view(lines: &[codemason_app_core::styled::StyledLine]) -> Vec<Vec<TerminalSpan>> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .filter(|span| !span.text.is_empty())
                .map(|span| {
                    let s = &span.style;
                    (
                        span.text.clone(),
                        s.fg,
                        s.bg,
                        [s.bold, s.italic, s.underline, s.dim],
                    )
                })
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Markdown rendering (restored: the terminal tests reached app-core's
// `markdown_to_styled` through the `layout::markdown_to_lines_testable`
// wrapper)
// ---------------------------------------------------------------------------

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

#[test]
fn test_markdown_to_lines_exact_styles_are_stable() {
    use codemason_app_core::styled::{
        StyledLine as Line, StyledSpan as Span, TextStyle as Style, ThemeColor as Color,
    };

    let base = PathBuf::from("/");
    let md = "# H1\n\n## H2\n\n### H3\n\n- item `x`\n\n```\ncode\n```\n\n---\n\n\
              [t](u)\n\n```mermaid\nA-->B\n```\n";
    let lines = markdown_to_lines_testable(md, &base, 10);
    let expected: Vec<Line> = vec![
        Line::from(Span::styled(
            "H1",
            Style::default().fg(Color::Heading).bold().underline(),
        )),
        Line::from(Span::styled(
            "H2",
            Style::default().fg(Color::Heading).bold(),
        )),
        Line::from(Span::styled("H3", Style::default().fg(Color::Text).bold())),
        Line::from(vec![
            Span::raw("* item "),
            Span::styled(
                " `x` ",
                Style::default().fg(Color::Code).bg(Color::Raised).bold(),
            ),
        ]),
        Line::from(Span::styled("  code", Style::default().fg(Color::Code))),
        Line::from(Span::styled(
            "\u{2500}".repeat(10),
            Style::default().fg(Color::Subtle),
        )),
        Line::from(vec![
            Span::styled("[t]", Style::default().fg(Color::Text)),
            Span::styled("(u)", Style::default().fg(Color::Link).underline()),
        ]),
        Line::from(Span::styled(
            "[Mermaid diagram - rendered as text below]",
            Style::default().fg(Color::Accent).bold(),
        )),
        Line::from(Span::styled("  A-->B", Style::default().fg(Color::Info))),
        Line::from(Vec::<Span>::new()),
        Line::from(Span::styled(
            "[Esc to close * images shown as placeholders * links are plain text]",
            Style::default().fg(Color::Subtle),
        )),
    ];
    // The terminal compared these lines after its lossless style adapter,
    // which drops `monospace` (a terminal is always monospace); compare what
    // it compared.
    assert_eq!(terminal_view(&lines), terminal_view(&expected));
}

// Restored from `test_layout_deep.rs` and `test_layout_extra.rs`.
#[test]
fn markdown_to_lines_covers_code_blocks_headings_links_images() {
    let base = std::path::PathBuf::from("/");
    let md = "# H1\n\n## H2\n\n### H3\n\n```rust\nfn main() {}\n```\n\n\
              ```mermaid\ngraph TD; A-->B\n```\n\n    indented code\n\n\
              A [link text](https://example.org/page) inline.\n\n\
              An ![alt text](/does/not/exist.png) image.\n\n\
              Inline `code` and <b>raw html</b>.\n\n---\n\n\
              - one\n- two\n  1. nested\n";
    let lines = markdown_to_lines_testable(md, &base, 40);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join("\n");
    assert!(joined.contains("H1"));
    assert!(joined.contains("H2"));
    assert!(joined.contains("H3"));
    assert!(joined.contains("fn main()"));
    assert!(joined.contains("Mermaid diagram"));
    assert!(joined.contains("link text"));
    assert!(joined.contains("example.org/page"));
    assert!(joined.contains("image"));
    assert!(joined.contains("Esc to close"));
}

#[test]
fn markdown_to_lines_testable_more() {
    let base = std::path::PathBuf::from("/");
    let md = "# H1\n\n- item one\n- item two\n  - nested\n\n1. first\n2. second\n\n> quote\n\n---\n\n**bold** and *italic* and `code` and ~~strike~~\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] todo\n- [x] done";
    let lines = markdown_to_lines_testable(md, &base, 60);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join("\n");
    assert!(joined.contains("H1"));
    assert!(joined.contains("item one"));
    assert!(joined.contains("nested"));
    assert!(joined.contains("quote"));
}
