//! Frontend-neutral markdown rendering into [`StyledLine`]s, and
//! [`has_table`], which tells a frontend whether a document holds a table.
//!
//! [`split_standalone_images`] cuts a document around the images that
//! stand alone in a paragraph, so a frontend can draw them itself.
//!
//! [`markdown_to_styled`] turns a markdown document (a `RESEARCH.md` body,
//! for the `/research open` viewer) into styled lines: headings, lists,
//! code blocks (monospace), inline code (monospace), links, images as
//! placeholders, rules and a closing viewer footer. The GUI lays the lines
//! out with egui.
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

/// Whether `markdown` contains a table (GitHub table syntax).
///
/// A document without a `|` is answered without parsing, so the check is
/// cheap enough to make every frame for the visible transcript.
///
/// # Returns
///
/// `true` when the document parses to at least one table.
#[must_use]
pub fn has_table(markdown: &str) -> bool {
    markdown.contains('|')
        && Parser::new_ext(markdown, Options::ENABLE_TABLES)
            .any(|event| matches!(event, MdEvent::Start(Tag::Table(_))))
}

/// One piece of a markdown document split by [`split_standalone_images`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownPiece {
    /// Markdown to render as it is.
    Markdown(String),
    /// An image that stood alone in its paragraph (`![alt](src)` on its own
    /// line or lines).
    Image {
        /// The image's alt text.
        alt: String,
        /// The image's source as written: a path or a URL.
        src: String,
    },
}

/// Split `markdown` around its standalone images: images that make up a
/// whole top-level paragraph on their own. A frontend draws those itself
/// (sized and clickable) and the markdown between them as usual; an image
/// inside a sentence, list or table stays in the markdown.
///
/// A document without `![` is answered without parsing.
///
/// # Returns
///
/// The pieces in document order; markdown pieces that are only whitespace
/// are left out. A document with no standalone image is one markdown piece.
#[must_use]
pub fn split_standalone_images(markdown: &str) -> Vec<MarkdownPiece> {
    let whole = || {
        if markdown.trim().is_empty() {
            Vec::new()
        } else {
            vec![MarkdownPiece::Markdown(markdown.to_string())]
        }
    };
    if !markdown.contains("![") {
        return whole();
    }
    let mut pieces = Vec::new();
    let mut emitted = 0;
    // Nesting of block containers (lists, quotes, tables...) above the
    // current event; only a paragraph at depth zero is top level.
    let mut depth = 0usize;
    // The open top-level paragraph: its start, the images in it and
    // whether anything but one image is in it.
    let mut paragraph: Option<(usize, Vec<(String, String)>, bool)> = None;
    let mut in_image = false;
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    // Each piece is parsed on its own, so a reference link (`[docs][d]`) on
    // one side of an image needs the document's definitions: they are
    // appended to every piece once the document is split (a definition
    // renders as nothing), as `message_blocks::split_code_blocks` does.
    let definitions: Vec<&str> = Parser::new_ext(markdown, options)
        .reference_definitions()
        .iter()
        .filter_map(|(_, def)| markdown.get(def.span.clone()))
        .map(str::trim)
        .collect();
    let piece = |text: &str| {
        let mut text = text.to_string();
        if !definitions.is_empty() {
            text.push_str("\n\n");
            text.push_str(&definitions.join("\n"));
        }
        MarkdownPiece::Markdown(text)
    };
    for (event, range) in Parser::new_ext(markdown, options).into_offset_iter() {
        match event {
            MdEvent::Start(Tag::Paragraph) if depth == 0 => {
                paragraph = Some((range.start, Vec::new(), false));
            }
            MdEvent::End(TagEnd::Paragraph) if depth == 0 => {
                if let Some((start, images, other)) = paragraph.take()
                    && !other
                    && images.len() == 1
                    && let Some((alt, src)) = images.into_iter().next()
                {
                    let before = &markdown[emitted..start];
                    if !before.trim().is_empty() {
                        pieces.push(piece(before));
                    }
                    pieces.push(MarkdownPiece::Image { alt, src });
                    emitted = range.end;
                }
            }
            MdEvent::Start(Tag::Image { dest_url, .. }) => {
                in_image = true;
                if let Some((_, images, _)) = paragraph.as_mut() {
                    images.push((String::new(), dest_url.to_string()));
                }
            }
            MdEvent::End(TagEnd::Image) => in_image = false,
            MdEvent::Text(text) | MdEvent::Code(text) if in_image => {
                if let Some((_, images, _)) = paragraph.as_mut()
                    && let Some((alt, _)) = images.last_mut()
                {
                    alt.push_str(&text);
                }
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {}
            MdEvent::Text(text) if text.trim().is_empty() => {}
            MdEvent::Start(Tag::Paragraph) | MdEvent::End(TagEnd::Paragraph) => {}
            MdEvent::Start(_) => {
                depth += usize::from(paragraph.is_none());
                mark_other(&mut paragraph, in_image);
            }
            MdEvent::End(_) => {
                depth = depth.saturating_sub(usize::from(paragraph.is_none()));
            }
            _ => mark_other(&mut paragraph, in_image),
        }
    }
    if emitted == 0 {
        return whole();
    }
    let rest = &markdown[emitted..];
    if !rest.trim().is_empty() {
        pieces.push(piece(rest));
    }
    pieces
}

/// Note that the open paragraph holds something other than its image.
fn mark_other(paragraph: &mut Option<(usize, Vec<(String, String)>, bool)>, in_image: bool) {
    if !in_image && let Some((_, _, other)) = paragraph.as_mut() {
        *other = true;
    }
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
                                TextStyle::new().fg(ThemeColor::Accent).bold(),
                            )));
                        }
                        let mut code_style = if is_mermaid {
                            TextStyle::new().fg(ThemeColor::Info)
                        } else {
                            TextStyle::new().fg(ThemeColor::Code)
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
                            HeadingLevel::H1 => {
                                TextStyle::new().fg(ThemeColor::Heading).bold().underline()
                            }
                            HeadingLevel::H2 => TextStyle::new().fg(ThemeColor::Heading).bold(),
                            HeadingLevel::H3
                            | HeadingLevel::H4
                            | HeadingLevel::H5
                            | HeadingLevel::H6 => TextStyle::new().fg(ThemeColor::Text).bold(),
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
                            TextStyle::new().fg(ThemeColor::Text),
                        ));
                        current_spans.push(StyledSpan::styled(
                            format!("({url})"),
                            TextStyle::new().fg(ThemeColor::Link).underline(),
                        ));
                    }
                }
                TagEnd::Image => {
                    if let Some((url, alt)) = image_state.take() {
                        flush_text_spans(&mut pending_text, &mut current_spans);
                        let placeholder = image_dimensions_or_placeholder(&alt, &url, base_dir);
                        current_spans.push(StyledSpan::styled(
                            placeholder,
                            TextStyle::new().fg(ThemeColor::Muted).italic(),
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
                        .fg(ThemeColor::Code)
                        .bg(ThemeColor::Raised)
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
                    TextStyle::new().fg(ThemeColor::Subtle),
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
        TextStyle::new().fg(ThemeColor::Subtle),
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
