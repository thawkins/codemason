//! Frontend-neutral markdown rendering into [`StyledLine`]s.
//!
//! [`markdown_to_styled`] turns a markdown document (a `RESEARCH.md` body,
//! for the `/research open` viewer) into styled lines: headings, lists,
//! code blocks (monospace), inline code (monospace), links, images as
//! placeholders, rules and a closing viewer footer. The terminal frontend
//! converts the lines to ratatui lines; the GUI lays them out with egui.
//!
//! Dependencies: `pulldown-cmark` (parsing) and [`crate::app`]
//! (`sanitize_for_display`, `image_dimensions_or_placeholder`).

use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event as MdEvent, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::app::{image_dimensions_or_placeholder, sanitize_for_display};
use crate::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};

/// The horizontal-rule glyph (U+2500, box drawings light horizontal).
const RULE_GLYPH: &str = "\u{2500}";

/// Inline emphasis kinds tracked while parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Emphasis {
    Bold,
    Italic,
}

/// Convert a markdown body into styled lines.
///
/// # Arguments
///
/// * `markdown` - the markdown source; it is sanitised for display first.
/// * `base_dir` - directory that relative image paths resolve against, for
///   the image-dimension placeholder.
/// * `wrap_width` - the display width in columns; a rule is drawn
///   `min(wrap_width, 80)` glyphs wide.
///
/// # Returns
///
/// The rendered lines, ending with a blank line and the viewer footer.
/// Code-block lines and inline code spans carry `monospace`.
pub fn markdown_to_styled(markdown: &str, base_dir: &Path, wrap_width: usize) -> Vec<StyledLine> {
    let sanitized = sanitize_for_display(markdown);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&sanitized, opts);
    let mut lines: Vec<StyledLine> = Vec::new();
    let mut current_spans: Vec<StyledSpan> = Vec::new();
    let mut list_stack: Vec<u8> = Vec::new();
    let mut in_code_block: Option<String> = None;
    let mut code_buffer = String::new();
    let mut pending_text = String::new();
    // Inline link/image state: (url, accumulated_visible_text)
    let mut link_state: Option<(String, String)> = None;
    let mut image_state: Option<(String, String)> = None;
    // Inline emphasis state stack.
    let mut style_stack: Vec<Emphasis> = Vec::new();

    for event in parser {
        match event {
            MdEvent::Start(tag) => match tag {
                Tag::CodeBlock(lang) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    let lang_str = match lang {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    in_code_block = Some(lang_str);
                    code_buffer.clear();
                }
                Tag::List(start) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    list_stack.push(start.unwrap_or(1) as u8);
                }
                Tag::Item => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                }
                Tag::Link { dest_url, .. } => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    link_state = Some((dest_url.to_string(), String::new()));
                }
                Tag::Image { dest_url, .. } => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    image_state = Some((dest_url.to_string(), String::new()));
                }
                Tag::Emphasis => style_stack.push(Emphasis::Italic),
                Tag::Strong => style_stack.push(Emphasis::Bold),
                _ => {}
            },
            MdEvent::End(tag_end) => match tag_end {
                TagEnd::CodeBlock => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    if let Some(ref lang) = in_code_block {
                        let is_mermaid = lang.eq_ignore_ascii_case("mermaid");
                        let label = if is_mermaid {
                            "[Mermaid diagram - rendered as text below]"
                        } else {
                            ""
                        };
                        if is_mermaid && !code_buffer.trim().is_empty() {
                            lines.push(StyledLine::from(StyledSpan::styled(
                                label.to_string(),
                                TextStyle::new().fg(ThemeColor::Magenta).bold(),
                            )));
                        }
                        let mut code_style = if is_mermaid {
                            TextStyle::new().fg(ThemeColor::Cyan)
                        } else {
                            TextStyle::new().fg(ThemeColor::DarkGray)
                        };
                        code_style.monospace = true;
                        for raw in code_buffer.lines() {
                            let line = sanitize_for_display(raw);
                            lines.push(StyledLine::from(StyledSpan::styled(
                                format!("  {line}"),
                                code_style,
                            )));
                        }
                    }
                    in_code_block = None;
                    code_buffer.clear();
                }
                TagEnd::List(_) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    list_stack.pop();
                }
                TagEnd::Heading(level) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    let idx = lines.len().saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        let style = match level {
                            HeadingLevel::H1 => TextStyle::new()
                                .fg(ThemeColor::White)
                                .bg(ThemeColor::Blue)
                                .bold(),
                            HeadingLevel::H2 => TextStyle::new().fg(ThemeColor::Cyan).bold(),
                            HeadingLevel::H3
                            | HeadingLevel::H4
                            | HeadingLevel::H5
                            | HeadingLevel::H6 => TextStyle::new().fg(ThemeColor::Yellow).bold(),
                        };
                        for span in &mut line.spans {
                            span.style = style;
                        }
                    }
                }
                TagEnd::Paragraph => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::BlockQuote(_) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Link => {
                    if let Some((url, text)) = link_state.take() {
                        flush_text_spans(&mut pending_text, &mut current_spans);
                        current_spans.push(StyledSpan::styled(
                            format!("[{text}]"),
                            TextStyle::new().fg(ThemeColor::White),
                        ));
                        current_spans.push(StyledSpan::styled(
                            format!("({url})"),
                            TextStyle::new().fg(ThemeColor::Cyan).underline(),
                        ));
                    }
                }
                TagEnd::Image => {
                    if let Some((url, alt)) = image_state.take() {
                        flush_text_spans(&mut pending_text, &mut current_spans);
                        let placeholder = image_dimensions_or_placeholder(&alt, &url, base_dir);
                        current_spans.push(StyledSpan::styled(
                            placeholder,
                            TextStyle::new().fg(ThemeColor::Yellow).italic(),
                        ));
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    style_stack.pop();
                }
                _ => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                }
            },
            MdEvent::Text(text) => {
                if in_code_block.is_some() {
                    code_buffer.push_str(&text);
                } else if let Some(ls) = link_state.as_mut() {
                    ls.1.push_str(&text);
                } else if let Some(is) = image_state.as_mut() {
                    is.1.push_str(&text);
                } else {
                    let prefix = list_prefix(&list_stack);
                    if !prefix.is_empty() && pending_text.is_empty() && current_spans.is_empty() {
                        pending_text.push_str(&prefix);
                    }
                    pending_text.push_str(&text);
                }
            }
            MdEvent::Code(code) => {
                if in_code_block.is_none() {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    let mut style = TextStyle::new()
                        .fg(ThemeColor::Green)
                        .bg(ThemeColor::Black)
                        .bold();
                    style.monospace = true;
                    current_spans.push(StyledSpan::styled(format!(" `{code}` "), style));
                } else {
                    code_buffer.push_str(&code);
                }
            }
            MdEvent::Html(html) => {
                if in_code_block.is_none() {
                    pending_text.push_str(&html);
                }
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                if in_code_block.is_none() {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                } else {
                    code_buffer.push('\n');
                }
            }
            MdEvent::Rule => {
                flush_text_spans(&mut pending_text, &mut current_spans);
                flush_line(&mut lines, &mut current_spans);
                lines.push(StyledLine::from(StyledSpan::styled(
                    RULE_GLYPH.repeat(wrap_width.min(80)),
                    TextStyle::new().fg(ThemeColor::DarkGray),
                )));
            }
            _ => {}
        }
    }

    flush_text_spans(&mut pending_text, &mut current_spans);
    flush_line(&mut lines, &mut current_spans);

    // Footer note about terminal limitations for images and links.
    lines.push(StyledLine::raw(""));
    lines.push(StyledLine::from(StyledSpan::styled(
        "[Esc to close * images shown as placeholders * links are plain text]".to_string(),
        TextStyle::new().fg(ThemeColor::DarkGray),
    )));

    lines
}

/// Move pending plain text into `spans` as an unstyled span.
fn flush_text_spans(text: &mut String, spans: &mut Vec<StyledSpan>) {
    if !text.is_empty() {
        spans.push(StyledSpan::raw(std::mem::take(text)));
    }
}

/// Close the current line: move `spans` into `lines` when non-empty.
fn flush_line(lines: &mut Vec<StyledLine>, spans: &mut Vec<StyledSpan>) {
    if !spans.is_empty() {
        lines.push(StyledLine::from(std::mem::take(spans)));
    }
}

/// The bullet prefix for the current list nesting depth.
fn list_prefix(list_stack: &[u8]) -> String {
    let depth = list_stack.len();
    if depth == 0 {
        return String::new();
    }
    let indent = "  ".repeat(depth.saturating_sub(1));
    let marker = if list_stack.last().copied().unwrap_or(1) == 0 {
        "* "
    } else {
        "* "
    };
    format!("{indent}{marker}")
}
