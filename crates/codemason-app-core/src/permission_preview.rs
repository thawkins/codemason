//! What a permission request will do, for the permission dialog.
//!
//! The agent's permission request names its tool and resource only
//! (`"<tool>: <resource>"`, where the resource is the input's `path`,
//! `command`, `url`, `pattern` or `query`, else `tool:<tool>`); it does not
//! carry the tool call. [`permission_preview`] finds the call it is about
//! among the transcript's unfinished tool calls and previews it: the diffs an
//! edit makes, or a command's full text. When no single unfinished call
//! matches, there is no preview and the dialog shows the request text.
//!
//! Depends on `codemason-types` (messages), [`crate::diff_view`] and
//! [`crate::message_text`].

use codemason_agent::message::{Message, MessagePart, ToolCallStatus};
use serde_json::Value;

use crate::diff_view::{FileDiff, diffs_from_tool_input};
use crate::message_text::canonical_tool_name;

/// A preview of what a permission request will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionPreview {
    /// The files an edit will change.
    Diffs(Vec<FileDiff>),
    /// The full command a shell call will run.
    Command(String),
}

/// The resource the agent derives from a call's input for its request
/// (mirrors `codemason_agent::session::permissions::extract_resource_from_input`).
fn resource_of(input: &Value, tool: &str) -> String {
    ["path", "command", "url", "pattern", "query"]
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str))
        .map_or_else(|| format!("tool:{tool}"), str::to_string)
}

/// The preview of the request described as `description`, from the
/// unfinished tool calls in `messages`.
///
/// # Returns
///
/// The diffs of an edit call, or the command of a `Bash` call, when exactly
/// one unfinished call matches the request's tool and resource; `None`
/// otherwise, or for any other tool.
#[must_use]
pub fn permission_preview(messages: &[Message], description: &str) -> Option<PermissionPreview> {
    let (tool, resource) = description.split_once(": ")?;
    let mut matches =
        messages
            .iter()
            .flat_map(|msg| msg.parts.iter())
            .filter_map(|part| match part {
                MessagePart::ToolCall {
                    tool: name, state, ..
                } if matches!(
                    state.status,
                    ToolCallStatus::Pending | ToolCallStatus::Running
                ) && canonical_tool_name(name) == canonical_tool_name(tool)
                    && resource_of(&state.input, name) == resource =>
                {
                    Some((name.as_str(), &state.input))
                }
                MessagePart::ToolCall { .. }
                | MessagePart::Text { .. }
                | MessagePart::Reasoning { .. }
                | MessagePart::Image(_) => None,
            });
    let (name, input) = matches.next()?;
    if matches.next().is_some() {
        // Two unfinished calls fit: the request could be either.
        return None;
    }
    let diffs = diffs_from_tool_input(name, input);
    if !diffs.is_empty() {
        return Some(PermissionPreview::Diffs(diffs));
    }
    if canonical_tool_name(name) == "Bash" {
        return input
            .get("command")
            .and_then(Value::as_str)
            .map(|command| PermissionPreview::Command(command.to_string()));
    }
    None
}
