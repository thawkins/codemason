//! Per-item token breakdown of the Context panel's partitions.
//!
//! The Context panel shows each partition (tools, skills, memory,
//! instruction files, history) as a collapsible list of the items inside
//! it. This module turns the raw inputs the snapshot builder already has
//! (tool definitions, the rendered skills and memory prompt sections, the
//! conversation messages) into [`ContextItem`] lists, each sorted largest
//! first. Every estimate uses the same ~4-bytes-per-token heuristic as the
//! partition totals ([`bytes_to_tokens`]), so items compare with them.
//!
//! The history is costed as the agent's request carries it: tool results
//! are only their text, cut to the agent's tool-result budget, so a long
//! session with large tool outputs is not counted many times over.
//!
//! Dependencies: `codemason-agent` for the message types and the
//! tool-result estimate, and
//! `codemason-types` for [`ToolDefinition`].

use std::collections::BTreeMap;

use codemason_agent::message::{Message, MessagePart, Role};
use codemason_agent::session::history::tool_result_bytes_for_llm;
use codemason_types::llm::ToolDefinition;

use crate::app::ContextItem;

/// Bytes per token of the partition estimates (mixed English, code and
/// JSON), so the estimates compare with provider-reported prompt tokens.
const BYTES_PER_TOKEN: u64 = 4;

/// Fixed per-definition JSON overhead, as
/// `codemason_agent::session::processor::estimate_tool_definition_bytes`
/// counts it.
const TOOL_DEFINITION_OVERHEAD: usize = 60;

/// Fixed per-message JSON overhead of the history estimate.
const MESSAGE_OVERHEAD: usize = 40;

/// Name of the history item holding tool calls and their results.
pub const TOOL_CALLS_ITEM: &str = "Tool calls & results";

/// Longest item name kept from a memory entry, in characters.
const MEMORY_NAME_CHARS: usize = 48;

/// Approximate token count of `bytes` UTF-8 bytes.
///
/// # Arguments
///
/// * `bytes` - the byte length to convert.
///
/// # Returns
///
/// `bytes / 4`, rounded down.
#[must_use]
pub fn bytes_to_tokens(bytes: usize) -> u64 {
    bytes as u64 / BYTES_PER_TOKEN
}

/// Sort `items` largest first, ties by name, so the lists are stable.
fn sorted(mut items: Vec<ContextItem>) -> Vec<ContextItem> {
    items.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));
    items
}

/// Split the tool catalog into built-in and MCP tools.
///
/// Each definition is costed as the catalog total costs it (name,
/// description, parameter schema and a fixed overhead), so the two lists
/// sum to the catalog partition.
///
/// # Arguments
///
/// * `defs` - the visible tool definitions.
/// * `mcp_name` - the display name of an MCP-bridged tool (e.g.
///   `github: create_issue`), or `None` for a built-in tool.
///
/// # Returns
///
/// `(built_in, mcp)`, each sorted largest first.
#[must_use]
pub fn tool_items(
    defs: &[ToolDefinition],
    mcp_name: impl Fn(&str) -> Option<String>,
) -> (Vec<ContextItem>, Vec<ContextItem>) {
    let (mcp, built_in): (Vec<_>, Vec<_>) = defs
        .iter()
        .map(|def| {
            let bytes = def.name.len()
                + def.description.len()
                + def.parameters.to_string().len()
                + TOOL_DEFINITION_OVERHEAD;
            let tokens = bytes_to_tokens(bytes);
            match mcp_name(&def.name) {
                Some(name) => (true, ContextItem { name, tokens }),
                None => (
                    false,
                    ContextItem {
                        name: def.name.clone(),
                        tokens,
                    },
                ),
            }
        })
        .partition(|(is_mcp, _)| *is_mcp);
    let strip = |items: Vec<(bool, ContextItem)>| items.into_iter().map(|(_, item)| item).collect();
    (sorted(strip(built_in)), sorted(strip(mcp)))
}

/// The skills listed in a rendered skills prompt section.
///
/// The section lists one skill per line as ``- `/name hint`  - text``; each
/// such line is one item named after the skill.
///
/// # Arguments
///
/// * `section` - the section `codemason_agent::agent::skills_prompt_section`
///   rendered.
///
/// # Returns
///
/// One item per skill, sorted largest first.
#[must_use]
pub fn skill_items(section: &str) -> Vec<ContextItem> {
    sorted(
        section
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("- `/")?;
                let name = rest.split([' ', '`']).next().filter(|n| !n.is_empty())?;
                Some(ContextItem {
                    name: name.to_string(),
                    tokens: bytes_to_tokens(line.len() + 1),
                })
            })
            .collect(),
    )
}

/// The memory entries in a rendered memory prompt section.
///
/// Each entry starts with a `- [category] content` line; the indented lines
/// after it (its tags) belong to it. An item is named after the start of
/// its first line.
///
/// # Arguments
///
/// * `section` - the section `codemason_agent::agent::build_memory_prompt_section`
///   rendered.
///
/// # Returns
///
/// One item per memory entry, sorted largest first.
#[must_use]
pub fn memory_items(section: &str) -> Vec<ContextItem> {
    let mut items: Vec<ContextItem> = Vec::new();
    for line in section.lines() {
        let bytes = line.len() + 1;
        if let Some(entry) = line.strip_prefix("- ") {
            let name = match entry.char_indices().nth(MEMORY_NAME_CHARS) {
                Some((cut, _)) => format!("{}...", &entry[..cut]),
                None => entry.to_string(),
            };
            items.push(ContextItem {
                name,
                tokens: bytes_to_tokens(bytes),
            });
        } else if line.starts_with("  ")
            && let Some(last) = items.last_mut()
        {
            last.tokens += bytes_to_tokens(bytes);
        }
    }
    sorted(items)
}

/// The history item a message of `role` counts toward.
fn role_item(role: &Role) -> &'static str {
    match role {
        Role::User => "User messages",
        Role::Assistant => "Assistant messages",
        Role::Compaction => "Compaction summary",
    }
}

/// Bytes one part of a `role` message takes in the request to the model.
///
/// A tool call is sent as its tool-use block (id, name and input) and, on
/// an assistant message, as a tool-result block holding only the result
/// text cut to the agent's budget
/// ([`codemason_agent::session::history::tool_result_bytes_for_llm`]); the
/// rest of the recorded output (its metadata) is never sent. Reasoning is
/// sent only when the provider's wire format replays it (`echo_reasoning`).
fn part_bytes(part: &MessagePart, role: &Role, echo_reasoning: bool) -> usize {
    match part {
        MessagePart::Text { text } => text.len(),
        MessagePart::Reasoning { text, .. } if echo_reasoning => text.len(),
        MessagePart::Reasoning { .. } => 0,
        MessagePart::ToolCall {
            tool,
            call_id,
            state,
        } => {
            let tool_use = call_id.len() + tool.len() + state.input.to_string().len();
            let result = match role {
                Role::Assistant => call_id.len() + tool_result_bytes_for_llm(tool, state),
                Role::User | Role::Compaction => 0,
            };
            tool_use + result
        }
        MessagePart::Image(_) => 0,
    }
}

/// Bytes of the extra user message that carries an assistant message's
/// tool results in the request, or zero when it has none.
fn results_message_bytes(message: &Message) -> usize {
    let calls = message
        .parts
        .iter()
        .any(|part| matches!(part, MessagePart::ToolCall { .. }));
    match message.role {
        Role::Assistant if calls => "user".len() + MESSAGE_OVERHEAD,
        Role::Assistant | Role::User | Role::Compaction => 0,
    }
}

/// Bytes `message` takes in the request to the model: its role label, a
/// fixed JSON overhead and every content part, plus the message carrying
/// its tool results.
///
/// # Arguments
///
/// * `message` - the message to cost.
/// * `echo_reasoning` - whether the provider's wire format sends reasoning
///   back to the model; when it does not, reasoning costs nothing.
///
/// # Returns
///
/// The message's byte estimate.
#[must_use]
pub fn message_bytes(message: &Message, echo_reasoning: bool) -> usize {
    message.role.to_string().len()
        + MESSAGE_OVERHEAD
        + message
            .parts
            .iter()
            .map(|part| part_bytes(part, &message.role, echo_reasoning))
            .sum::<usize>()
        + results_message_bytes(message)
}

/// The conversation history by message role, with tool calls and their
/// results as one item of their own.
///
/// # Arguments
///
/// * `messages` - the conversation history.
/// * `echo_reasoning` - whether the provider's wire format sends reasoning
///   back to the model.
///
/// # Returns
///
/// One item per role present (and [`TOOL_CALLS_ITEM`] when any tool was
/// called), sorted largest first.
#[must_use]
pub fn history_items(messages: &[Message], echo_reasoning: bool) -> Vec<ContextItem> {
    let mut bytes: BTreeMap<String, usize> = BTreeMap::new();
    for message in messages {
        let (calls, other): (Vec<&MessagePart>, Vec<&MessagePart>) = message
            .parts
            .iter()
            .partition(|part| matches!(part, MessagePart::ToolCall { .. }));
        let own = message.role.to_string().len()
            + MESSAGE_OVERHEAD
            + other
                .into_iter()
                .map(|part| part_bytes(part, &message.role, echo_reasoning))
                .sum::<usize>();
        *bytes
            .entry(role_item(&message.role).to_string())
            .or_default() += own;
        if !calls.is_empty() {
            *bytes.entry(TOOL_CALLS_ITEM.to_string()).or_default() += calls
                .into_iter()
                .map(|part| part_bytes(part, &message.role, echo_reasoning))
                .sum::<usize>()
                + results_message_bytes(message);
        }
    }
    sorted(
        bytes
            .into_iter()
            .map(|(name, bytes)| ContextItem {
                name,
                tokens: bytes_to_tokens(bytes),
            })
            .collect(),
    )
}
