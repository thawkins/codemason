//! What a permission request will do, for the permission dialog.
//!
//! A permission request raised for a tool call carries that call: its id,
//! name and the input it will run with, after any `pre_tool_use` hook (the
//! `tool_call` entry of the request's metadata, from
//! `Event::PermissionRequested`). [`permission_preview`] previews exactly
//! that input: the diffs an edit will make, or the full command a `Bash`
//! call will run. A `Write` or `create` of a file that exists is diffed
//! against the file's current contents. A request without a call has no
//! preview, and the dialog shows the request text.
//!
//! Depends on `codemason-config` (the request type), `serde_json`,
//! [`crate::diff_view`] and [`crate::message_text`].

use std::path::Path;

use codemason_config::permission::PermissionRequest;
use serde_json::Value;

use crate::diff_view::{FileDiff, diffs_from_tool_input, file_diff};
use crate::message_text::canonical_tool_name;

/// Largest existing file read to diff a `Write` against, in bytes.
const MAX_CURRENT_BYTES: u64 = 1024 * 1024;

/// A preview of what a permission request will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionPreview {
    /// The files an edit will change.
    Diffs(Vec<FileDiff>),
    /// The full command a shell call will run.
    Command(String),
}

/// The tool name and input of the call `request` is about, if it carries
/// one.
fn request_call(request: &PermissionRequest) -> Option<(&str, &Value)> {
    let call = request.metadata.get("tool_call")?;
    Some((call.get("name")?.as_str()?, call.get("input")?))
}

/// `diff` of a whole-file write, redone against the file's current
/// contents when it exists under `working_dir` and is small enough to read.
fn against_current(diff: FileDiff, working_dir: &Path, content: &str) -> FileDiff {
    let path = working_dir.join(&diff.path);
    let current = std::fs::metadata(&path)
        .ok()
        .filter(|meta| meta.is_file() && meta.len() <= MAX_CURRENT_BYTES)
        .and_then(|_| std::fs::read_to_string(&path).ok());
    match current {
        Some(current) => file_diff(&diff.path, &current, content),
        // A file that does not exist yet: every line is new.
        None if !path.exists() => FileDiff {
            whole_file: false,
            ..diff
        },
        None => diff,
    }
}

/// The preview of `request`.
///
/// # Arguments
///
/// * `request` - the permission request.
/// * `working_dir` - the directory a relative path in the call is under.
///
/// # Returns
///
/// The diffs of an edit call, or the command of a `Bash` call; `None` for a
/// request without a call, or for any other tool.
#[must_use]
pub fn permission_preview(
    request: &PermissionRequest,
    working_dir: &Path,
) -> Option<PermissionPreview> {
    let (name, input) = request_call(request)?;
    let diffs = diffs_from_tool_input(name, input);
    if !diffs.is_empty() {
        let content = input
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let diffs = diffs
            .into_iter()
            .map(|diff| {
                if diff.whole_file {
                    against_current(diff, working_dir, content)
                } else {
                    diff
                }
            })
            .collect();
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
