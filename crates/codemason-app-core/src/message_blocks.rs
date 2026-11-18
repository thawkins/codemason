//! The transcript block model: a message as a list of blocks.
//!
//! [`message_to_blocks`] splits a [`Message`] into [`TranscriptBlock`]s that a
//! frontend draws with its own widgets: the assistant's text as Markdown
//! source with its top-level fenced code blocks split out as
//! [`TranscriptBlock::Code`], the user's text as plain text, reasoning,
//! images, and tool calls and agent notices as the same styled lines
//! [`crate::message_lines`] renders for them. Blocks carry meaning, not layout; how a block looks is
//! the frontend's choice.
//!
//! Every text a block carries is passed through `sanitize_terminal` first
//! (SEC-080), as the line renderer does.
//!
//! Depends on `codemason-types` (messages, `sanitize_terminal`),
//! `pulldown-cmark` (finding code blocks) and [`crate::message_lines`]
//! (tool-call and notice lines).

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::PathBuf;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use codemason_types::sanitize_terminal::sanitize_terminal;

use crate::message_lines::part_lines;
use crate::message_text::{capitalize_tool_name, is_agent_notice};
use crate::styled::StyledLine;

/// Where a tool call is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    /// Waiting to run, or running.
    Running,
    /// Finished.
    Done,
    /// Failed.
    Failed,
}

/// One block of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptBlock {
    /// Markdown source: the assistant's text, or a compaction summary, less
    /// its top-level code blocks.
    Markdown(String),
    /// A top-level code block of the assistant's Markdown.
    Code {
        /// The language named after the opening fence (its first word), if
        /// any.
        lang: Option<String>,
        /// The code, without the fences or a final newline.
        text: String,
    },
    /// Text shown as written: what the user typed.
    Plain(String),
    /// An agent notice, as styled lines.
    Notice(Vec<StyledLine>),
    /// A tool call: its header and result lines, and its status label
    /// (`"<Tool>: <status>"`, for assistive technology).
    Tool {
        /// The call's lines, from [`part_lines`].
        lines: Vec<StyledLine>,
        /// `"<Tool>: <status>"`.
        status_label: String,
        /// The tool's name as called.
        name: String,
        /// Where the call is in its lifecycle.
        state: ToolState,
        /// How long the call ran, once it finished.
        duration_ms: Option<u64>,
    },
    /// A run of tool calls and reasoning folded into one row (see
    /// [`crate::activity::group_activity`]).
    Activity(crate::activity::ActivityGroup),
    /// The changes a finished edit call made to one file, after its
    /// [`TranscriptBlock::Tool`] block.
    Diff(crate::diff_view::FileDiff),
    /// The model's reasoning, as styled lines (narrated tool-call markup
    /// already removed).
    Thinking(Vec<StyledLine>),
    /// An attached image.
    Image {
        /// The file name shown for it.
        name: String,
        /// Where the image is stored.
        path: PathBuf,
    },
}

impl TranscriptBlock {
    /// The block's text without styling, one line per line, for copying and
    /// for assistive technology.
    #[must_use]
    pub fn plain_text(&self) -> String {
        match self {
            Self::Markdown(text) | Self::Plain(text) | Self::Code { text, .. } => text.clone(),
            Self::Notice(lines) | Self::Thinking(lines) | Self::Tool { lines, .. } => lines
                .iter()
                .map(StyledLine::text)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Image { name, .. } => format!("[image: {name}]"),
            Self::Activity(group) => group.summary.label(),
            Self::Diff(diff) => {
                let mut text = diff.path.clone();
                for hunk in &diff.hunks {
                    text.push('\n');
                    text.push_str(&hunk.header);
                    for line in &hunk.lines {
                        let mark = match line.kind {
                            crate::diff_view::DiffLineKind::Context => ' ',
                            crate::diff_view::DiffLineKind::Added => '+',
                            crate::diff_view::DiffLineKind::Removed => '-',
                        };
                        text.push('\n');
                        text.push(mark);
                        text.push_str(&line.text);
                    }
                }
                text
            }
        }
    }
}

/// Split `msg` into blocks, one or more per part, in part order.
///
/// # Arguments
///
/// * `msg` - the message.
/// * `tool_step_map` - the `[step.substep]` tag of each tool call id.
/// * `cwd` - the working directory tool-call paths are shown relative to.
///
/// # Returns
///
/// The message's blocks. The assistant's text is [`TranscriptBlock::Markdown`]
/// with each top-level code block as a [`TranscriptBlock::Code`] between the
/// Markdown around it (an agent notice is [`TranscriptBlock::Notice`]), the
/// user's text [`TranscriptBlock::Plain`], and a compaction summary Markdown.
/// A finished edit call is followed by a [`TranscriptBlock::Diff`] per file
/// it changed. Text that is empty after trimming produces no block.
pub fn message_to_blocks<S: BuildHasher>(
    msg: &Message,
    tool_step_map: &HashMap<String, (String, u32, u32), S>,
    cwd: &str,
) -> Vec<TranscriptBlock> {
    msg.parts
        .iter()
        .flat_map(|part| {
            let block = part_block(&msg.role, part, tool_step_map, cwd);
            let diffs = match part {
                // A dry run reports the change without making it.
                MessagePart::ToolCall { tool, state, .. }
                    if state.status == ToolCallStatus::Completed
                        && !crate::diff_view::is_dry_run(&state.input) =>
                {
                    crate::diff_view::diffs_from_tool_input(tool, &state.input)
                }
                MessagePart::ToolCall { .. }
                | MessagePart::Text { .. }
                | MessagePart::Reasoning { .. }
                | MessagePart::Image(_) => Vec::new(),
            };
            block
                .into_iter()
                .chain(diffs.into_iter().map(TranscriptBlock::Diff))
        })
        .flat_map(|block| match block {
            TranscriptBlock::Markdown(text) => split_code_blocks(&text),
            other => vec![other],
        })
        .collect()
}

/// Split Markdown into its top-level code blocks and the Markdown between
/// them.
///
/// A code block inside a list item or block quote stays in its Markdown, so
/// the list or quote is not cut in two. An unclosed fence (a reply still
/// streaming) is a code block up to the end of the text, as CommonMark
/// reads it. Once a code block is split out, the message's link reference
/// definitions are appended to each Markdown fragment, so a reference link
/// still resolves when its definition is on the other side of the code.
///
/// # Returns
///
/// The blocks in source order; Markdown that is only whitespace is dropped.
#[must_use]
pub fn split_code_blocks(markdown: &str) -> Vec<TranscriptBlock> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    // Each Markdown fragment is parsed on its own, so a reference link
    // (`[docs][d]`) on one side of a code block needs the definitions from
    // the whole message: they are appended to every fragment once a code
    // block is split out (a definition renders as nothing).
    let definitions: Vec<&str> = Parser::new_ext(markdown, options)
        .reference_definitions()
        .iter()
        .filter_map(|(_, def)| markdown.get(def.span.clone()))
        .map(str::trim)
        .collect();
    let mut blocks = Vec::new();
    let mut split = false;
    let push_markdown = |blocks: &mut Vec<TranscriptBlock>, text: &str, split: bool| {
        if text.trim().is_empty() {
            return;
        }
        let mut text = text.trim_matches('\n').to_string();
        if split && !definitions.is_empty() {
            text.push_str("\n\n");
            text.push_str(&definitions.join("\n"));
        }
        blocks.push(TranscriptBlock::Markdown(text));
    };
    let mut depth = 0usize;
    let mut copied_to = 0usize;
    let mut code: Option<(Option<String>, String, usize)> = None;
    let parser = Parser::new_ext(markdown, options);
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) if depth == 0 => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(str::to_string)
                        .filter(|l| !l.is_empty()),
                    CodeBlockKind::Indented => None,
                };
                code = Some((lang, String::new(), range.start));
            }
            Event::Text(text) if code.is_some() => {
                if let Some((_, body, _)) = code.as_mut() {
                    body.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) if code.is_some() => {
                if let Some((lang, body, start)) = code.take() {
                    split = true;
                    push_markdown(&mut blocks, &markdown[copied_to..start], split);
                    let text = body.strip_suffix('\n').unwrap_or(&body).to_string();
                    blocks.push(TranscriptBlock::Code { lang, text });
                    copied_to = range.end;
                }
            }
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    push_markdown(&mut blocks, &markdown[copied_to..], split);
    blocks
}

/// The block of one part, or `None` for an empty text part.
fn part_block<S: BuildHasher>(
    role: &Role,
    part: &MessagePart,
    tool_step_map: &HashMap<String, (String, u32, u32), S>,
    cwd: &str,
) -> Option<TranscriptBlock> {
    let lines = || {
        let mut lines = part_lines(role, part, tool_step_map, cwd);
        while lines
            .last()
            .is_some_and(|l| l.spans.iter().all(|s| s.text.trim().is_empty()))
        {
            lines.pop();
        }
        lines
    };
    match part {
        MessagePart::Text { text } => {
            if *role == Role::Assistant && is_agent_notice(text) {
                return Some(TranscriptBlock::Notice(lines()));
            }
            let text = sanitize_terminal(text);
            if text.trim().is_empty() {
                return None;
            }
            let text = text.into_owned();
            Some(match role {
                Role::User => TranscriptBlock::Plain(text),
                Role::Assistant | Role::Compaction => TranscriptBlock::Markdown(text),
            })
        }
        MessagePart::ToolCall { tool, state, .. } => Some(TranscriptBlock::Tool {
            lines: crate::activity::truncate_result_lines(lines()),
            status_label: format!("{}: {}", capitalize_tool_name(tool), state.status),
            name: tool.clone(),
            state: match state.status {
                ToolCallStatus::Pending | ToolCallStatus::Running => ToolState::Running,
                ToolCallStatus::Completed => ToolState::Done,
                ToolCallStatus::Error => ToolState::Failed,
            },
            duration_ms: state.duration_ms,
        }),
        MessagePart::Reasoning { .. } => {
            let lines = lines();
            (!lines.is_empty()).then_some(TranscriptBlock::Thinking(lines))
        }
        MessagePart::Image(img) => Some(TranscriptBlock::Image {
            name: img
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image")
                .to_string(),
            path: img.path.clone(),
        }),
    }
}
