//! The transcript block model: a message as a list of blocks.
//!
//! [`message_to_blocks`] splits a [`Message`] into [`TranscriptBlock`]s that a
//! frontend draws with its own widgets: the assistant's text as Markdown
//! source, the user's text as plain text, reasoning, images, and tool calls
//! and agent notices as the same styled lines [`crate::message_lines`]
//! renders for them. Blocks carry meaning, not layout; how a block looks is
//! the frontend's choice.
//!
//! Every text a block carries is passed through `sanitize_terminal` first
//! (SEC-080), as the line renderer does.
//!
//! Depends on `codemason-types` (messages, `sanitize_terminal`) and
//! [`crate::message_lines`] (tool-call and notice lines).

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::PathBuf;

use codemason_agent::message::{Message, MessagePart, Role};
use codemason_types::sanitize_terminal::sanitize_terminal;

use crate::message_lines::part_lines;
use crate::message_text::{capitalize_tool_name, is_agent_notice};
use crate::styled::StyledLine;

/// One block of a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptBlock {
    /// Markdown source: the assistant's text, or a compaction summary.
    Markdown(String),
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
    },
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
            Self::Markdown(text) | Self::Plain(text) => text.clone(),
            Self::Notice(lines) | Self::Thinking(lines) | Self::Tool { lines, .. } => lines
                .iter()
                .map(StyledLine::text)
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Image { name, .. } => format!("[image: {name}]"),
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
/// (an agent notice is [`TranscriptBlock::Notice`]), the user's
/// [`TranscriptBlock::Plain`], and a compaction summary Markdown. Text parts
/// that are empty after trimming produce no block.
pub fn message_to_blocks<S: BuildHasher>(
    msg: &Message,
    tool_step_map: &HashMap<String, (String, u32, u32), S>,
    cwd: &str,
) -> Vec<TranscriptBlock> {
    msg.parts
        .iter()
        .filter_map(|part| part_block(&msg.role, part, tool_step_map, cwd))
        .collect()
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
            lines: lines(),
            status_label: format!("{}: {}", capitalize_tool_name(tool), state.status),
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
