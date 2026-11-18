//! Line diffs of the files an edit tool call changes, for display.
//!
//! [`file_diff`] turns an old and a new text into a [`FileDiff`]: unified
//! hunks with three lines of context, each line numbered on the old and new
//! side, plus the added and removed counts. [`diffs_from_tool_input`] reads
//! the diffs of an `Edit`, `multi_edit`, `Write` or `create` call from its
//! input. For `Edit` and `multi_edit` the input holds only the replaced
//! text, so the line numbers count from that snippet, not the file; for
//! `Write` and `create` every line is added.
//!
//! A diff stops after [`DIFF_MAX_LINES`] lines and says so
//! ([`FileDiff::truncated`]), so a rewrite of a large file stays cheap to
//! draw.
//!
//! Depends on `similar` (line diffing), `serde_json` (tool input) and
//! [`crate::message_text`] (tool names).

use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use crate::message_text::canonical_tool_name;

/// Most lines a [`FileDiff`] holds across its hunks.
pub const DIFF_MAX_LINES: usize = 2_000;

/// Lines of unchanged context around each change.
const CONTEXT_LINES: usize = 3;

/// What a diff line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Unchanged.
    Context,
    /// Only in the new text.
    Added,
    /// Only in the old text.
    Removed,
}

/// One line of a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// What the line is.
    pub kind: DiffLineKind,
    /// Its 1-based number in the old text, unless it was added.
    pub old_no: Option<u32>,
    /// Its 1-based number in the new text, unless it was removed.
    pub new_no: Option<u32>,
    /// The line, without its line ending.
    pub text: String,
}

/// A run of changes with its context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// The unified header, e.g. `@@ -3,7 +3,7 @@`.
    pub header: String,
    /// The hunk's lines.
    pub lines: Vec<DiffLine>,
}

/// The changes to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// The file's path as the tool call named it.
    pub path: String,
    /// The hunks, in order.
    pub hunks: Vec<DiffHunk>,
    /// Lines added, over the whole diff.
    pub added: u32,
    /// Lines removed, over the whole diff.
    pub removed: u32,
    /// Whether lines were left out after [`DIFF_MAX_LINES`].
    pub truncated: bool,
}

/// A 1-based line number from `similar`'s 0-based index.
fn line_no(index: Option<usize>) -> Option<u32> {
    index.map(|i| u32::try_from(i + 1).unwrap_or(u32::MAX))
}

/// The diff of `old` to `new`, as the file at `path`.
///
/// Hunks carry three lines of context. Lines past [`DIFF_MAX_LINES`] are
/// left out (the counts still cover them) and `truncated` is set.
#[must_use]
pub fn file_diff(path: &str, old: &str, new: &str) -> FileDiff {
    let diff = TextDiff::from_lines(old, new);
    let mut out = FileDiff {
        path: path.to_string(),
        hunks: Vec::new(),
        added: 0,
        removed: 0,
        truncated: false,
    };
    let mut kept = 0usize;
    for hunk in diff
        .unified_diff()
        .context_radius(CONTEXT_LINES)
        .iter_hunks()
    {
        let mut lines = Vec::new();
        for change in hunk.iter_changes() {
            let kind = match change.tag() {
                ChangeTag::Equal => DiffLineKind::Context,
                ChangeTag::Insert => {
                    out.added += 1;
                    DiffLineKind::Added
                }
                ChangeTag::Delete => {
                    out.removed += 1;
                    DiffLineKind::Removed
                }
            };
            if kept >= DIFF_MAX_LINES {
                out.truncated = true;
                continue;
            }
            kept += 1;
            lines.push(DiffLine {
                kind,
                old_no: line_no(change.old_index()),
                new_no: line_no(change.new_index()),
                text: change.value().trim_end_matches(['\n', '\r']).to_string(),
            });
        }
        if !lines.is_empty() {
            out.hunks.push(DiffHunk {
                header: hunk.header().to_string(),
                lines,
            });
        }
    }
    out
}

/// The first of `keys` that `value` holds as a string.
fn string_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
}

/// The diff one `Edit`-shaped object describes, if it names a file.
fn edit_diff(edit: &Value) -> Option<FileDiff> {
    let path = string_field(edit, &["file_path", "path"])?;
    let old = string_field(edit, &["old_string", "old_str"]).unwrap_or_default();
    let new = string_field(edit, &["new_string", "new_str"]).unwrap_or_default();
    Some(file_diff(path, old, new))
}

/// The diffs a call of `tool` with `input` makes.
///
/// `Edit` gives one diff of its `old_string` to `new_string`; `multi_edit`
/// one per entry of `edits`; `Write` and `create` the whole `content` as
/// added lines. Any other tool, or input without a path, gives none.
#[must_use]
pub fn diffs_from_tool_input(tool: &str, input: &Value) -> Vec<FileDiff> {
    match canonical_tool_name(tool) {
        "Edit" => edit_diff(input).into_iter().collect(),
        "multi_edit" | "multiedit" => input
            .get("edits")
            .and_then(Value::as_array)
            .map(|edits| edits.iter().filter_map(edit_diff).collect())
            .unwrap_or_default(),
        "Write" | "create" => string_field(input, &["path", "file_path"])
            .map(|path| {
                let content = string_field(input, &["content"]).unwrap_or_default();
                vec![file_diff(path, "", content)]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
