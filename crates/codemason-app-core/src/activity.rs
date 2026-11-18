//! Activity grouping: folding a turn's tool calls and reasoning into one
//! row.
//!
//! A long agent run is mostly tool calls. [`group_activity`] folds each run
//! of consecutive [`TranscriptBlock::Tool`] and [`TranscriptBlock::Thinking`]
//! blocks into one [`TranscriptBlock::Activity`] whose [`ActivitySummary`]
//! reads as one line ("Read 4 files, edited 2 files, ran 1 command (12 s)"),
//! so the answer around it stays visible. How much is folded follows the
//! transcript [`Verbosity`]. A frontend shows the group collapsed and lists
//! its items when it is opened.
//!
//! Tools are counted by category from their name (after resolving retired
//! spellings with [`canonical_tool_name`]).
//!
//! Depends on [`crate::message_blocks`], [`crate::message_text`] and
//! [`crate::transcript_prefs`].

use crate::message_blocks::{ToolState, TranscriptBlock};
use crate::message_text::canonical_tool_name;
use crate::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use crate::transcript_prefs::Verbosity;

/// Most result lines a tool block keeps under its header; the rest are
/// counted in one "... N more lines" line.
pub const RESULT_PREVIEW_MAX_LINES: usize = 20;

/// What a group of activity did, by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActivitySummary {
    /// Reads and listings.
    pub reads: u32,
    /// File edits, writes and other file changes.
    pub edits: u32,
    /// Shell commands.
    pub commands: u32,
    /// Searches.
    pub searches: u32,
    /// Other tool calls.
    pub other: u32,
    /// Reasoning blocks and `think` calls.
    pub thoughts: u32,
    /// The tool calls' total run time.
    pub duration_ms: u64,
    /// Failed tool calls.
    pub errors: u32,
}

/// A run of tool calls and reasoning shown as one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityGroup {
    /// What the run did.
    pub summary: ActivitySummary,
    /// The folded blocks, in order.
    pub items: Vec<TranscriptBlock>,
    /// Whether a tool call in the run is still running.
    pub running: bool,
}

/// The kind a tool is counted as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Read,
    Edit,
    Command,
    Search,
    Thought,
    Other,
}

/// The kind of the tool called `name`.
fn category(name: &str) -> Category {
    match canonical_tool_name(name) {
        "read" | "list" | "Glob" | "file_info" | "open" | "diff_files" => Category::Read,
        "Edit" | "Write" | "multi_edit" | "multiedit" | "patch" | "apply_patch" | "create"
        | "append_to_file" | "copy_file" | "move_file" | "rm" | "make_directory" => Category::Edit,
        "Bash" => Category::Command,
        "Grep" | "websearch" | "web_search" => Category::Search,
        "think" => Category::Thought,
        other if other.starts_with("codeindex_") => Category::Search,
        _ => Category::Other,
    }
}

/// `count` of `one`/`many`: "1 file", "3 files".
fn counted(count: u32, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// A run time: "12 s", or "2 m 05 s" from a minute.
fn duration(ms: u64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs} s")
    } else {
        format!("{} m {:02} s", secs / 60, secs % 60)
    }
}

impl ActivitySummary {
    /// Count one block into the summary.
    fn add(&mut self, block: &TranscriptBlock) {
        match block {
            TranscriptBlock::Tool {
                name,
                state,
                duration_ms,
                ..
            } => {
                match category(name) {
                    Category::Read => self.reads += 1,
                    Category::Edit => self.edits += 1,
                    Category::Command => self.commands += 1,
                    Category::Search => self.searches += 1,
                    Category::Thought => self.thoughts += 1,
                    Category::Other => self.other += 1,
                }
                if *state == ToolState::Failed {
                    self.errors += 1;
                }
                self.duration_ms += duration_ms.unwrap_or(0);
            }
            TranscriptBlock::Thinking(_) => self.thoughts += 1,
            TranscriptBlock::Markdown(_)
            | TranscriptBlock::Code { .. }
            | TranscriptBlock::Plain(_)
            | TranscriptBlock::Notice(_)
            | TranscriptBlock::Image { .. }
            | TranscriptBlock::Activity(_) => {}
        }
    }

    /// The one-line description of the run.
    ///
    /// The kinds that occurred, in a fixed order, then the failures and the
    /// run time (from one second):
    /// `"Read 4 files, edited 2 files, ran 1 command (12 s)"`,
    /// `"Ran 3 commands, 1 failed"`, `"Thought"`.
    #[must_use]
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.reads > 0 {
            parts.push(format!("read {}", counted(self.reads, "file", "files")));
        }
        if self.edits > 0 {
            parts.push(format!("edited {}", counted(self.edits, "file", "files")));
        }
        if self.commands > 0 {
            parts.push(format!(
                "ran {}",
                counted(self.commands, "command", "commands")
            ));
        }
        if self.searches > 0 {
            parts.push(format!(
                "ran {}",
                counted(self.searches, "search", "searches")
            ));
        }
        if self.other > 0 {
            parts.push(format!("used {}", counted(self.other, "tool", "tools")));
        }
        if self.thoughts > 0 {
            parts.push(match self.thoughts {
                1 => "thought".to_string(),
                n => format!("thought {n} times"),
            });
        }
        if self.errors > 0 {
            parts.push(format!("{} failed", self.errors));
        }
        let mut label = parts.join(", ");
        if let Some(first) = label.get(..1) {
            label = first.to_uppercase() + &label[1..];
        }
        if self.duration_ms >= 1000 {
            label.push_str(&format!(" ({})", duration(self.duration_ms)));
        }
        label
    }
}

impl ActivityGroup {
    /// A group of `items`, with its summary and running flag.
    #[must_use]
    pub fn new(items: Vec<TranscriptBlock>) -> Self {
        let mut summary = ActivitySummary::default();
        for item in &items {
            summary.add(item);
        }
        let running = items.iter().any(|item| {
            matches!(
                item,
                TranscriptBlock::Tool {
                    state: ToolState::Running,
                    ..
                }
            )
        });
        Self {
            summary,
            items,
            running,
        }
    }
}

/// Whether `verbosity` folds `block` into an activity group.
fn folds(block: &TranscriptBlock, verbosity: Verbosity) -> bool {
    match verbosity {
        Verbosity::Low => matches!(
            block,
            TranscriptBlock::Tool { .. } | TranscriptBlock::Thinking(_)
        ),
        Verbosity::Medium => matches!(block, TranscriptBlock::Thinking(_)),
        Verbosity::High => false,
    }
}

/// Fold runs of activity in `blocks` into [`TranscriptBlock::Activity`]
/// groups, as `verbosity` asks.
///
/// `Low` folds every maximal run of consecutive tool and reasoning blocks;
/// `Medium` folds only reasoning and keeps each tool call as its own block;
/// `High` returns `blocks` unchanged. Every other block stays where it is.
#[must_use]
pub fn group_activity(blocks: Vec<TranscriptBlock>, verbosity: Verbosity) -> Vec<TranscriptBlock> {
    if verbosity == Verbosity::High {
        return blocks;
    }
    let mut out = Vec::with_capacity(blocks.len());
    let mut run: Vec<TranscriptBlock> = Vec::new();
    for block in blocks {
        if folds(&block, verbosity) {
            run.push(block);
            continue;
        }
        if !run.is_empty() {
            out.push(TranscriptBlock::Activity(ActivityGroup::new(
                std::mem::take(&mut run),
            )));
        }
        out.push(block);
    }
    if !run.is_empty() {
        out.push(TranscriptBlock::Activity(ActivityGroup::new(run)));
    }
    out
}

/// Keep a tool block's header and at most [`RESULT_PREVIEW_MAX_LINES`]
/// lines under it, counting the rest in one "... N more lines" line.
#[must_use]
pub fn truncate_result_lines(mut lines: Vec<StyledLine>) -> Vec<StyledLine> {
    let keep = 1 + RESULT_PREVIEW_MAX_LINES;
    if lines.len() <= keep {
        return lines;
    }
    let more = lines.len() - keep;
    lines.truncate(keep);
    lines.push(StyledLine::from(StyledSpan::styled(
        format!("  ... {more} more lines"),
        TextStyle::new().fg(ThemeColor::Subtle),
    )));
    lines
}
