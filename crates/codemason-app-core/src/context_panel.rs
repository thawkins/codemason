//! Frontend-neutral view model of the Context panel (spec `contextpanel`).
//!
//! [`context_view`] turns a [`ContextPartitionSnapshot`] into what the panel
//! shows: how much of the model's context window is used (FR-010/FR-012),
//! the tokens sent on the last turn, one [`ContextRow`] per partition with
//! its share of the window and the items inside it (FR-005..FR-009), the
//! history message count (FR-008), the free headroom (FR-012) and the
//! stream I/O byte counts. Ratios are `None` when the model advertises no
//! window (FR-011).
//!
//! The rows do not overlap, so they stack into one usage bar: the system
//! prompt row is the prompt without its skills, memory and instruction-file
//! parts, which have rows of their own, and the tool catalog is split into
//! built-in and MCP tools. The rows sum to
//! [`ContextPartitionSnapshot::total_tokens`].
//!
//! [`ContextView::text_lines`] is the panel's plain text, the copy source of
//! its text selection.
//!
//! The view is a UI-only artifact: building it reads the snapshot and never
//! touches the conversation (FR-016).
//!
//! Dependencies: [`crate::app::ContextPartitionSnapshot`] for the values.

use crate::app::{ContextItem, ContextPartitionSnapshot};

/// Compact a token estimate into a short form (`123`, `12.3k`, `1.2M`).
///
/// # Arguments
///
/// * `tokens` - the estimate to format.
///
/// # Returns
///
/// The count, with a `k` or `M` suffix from a thousand up.
#[must_use]
pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Format a share of the window as `12.3%`, or `unknown` without a window.
///
/// # Arguments
///
/// * `percent` - the share, from [`ContextPartitionSnapshot::percent_of_window`].
///
/// # Returns
///
/// The percentage with one decimal, or `unknown`.
#[must_use]
pub fn format_percent(percent: Option<f64>) -> String {
    percent.map_or_else(|| "unknown".to_string(), |pct| format!("{pct:.1}%"))
}

/// Which partition a [`ContextRow`] shows; a frontend keys its colours by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContextCategory {
    /// The system prompt, without the parts that have rows of their own.
    SystemPrompt,
    /// The skills listed in the system prompt.
    Skills,
    /// The memory entries injected into the system prompt.
    Memory,
    /// The instruction file (AGENTS.md, ...) in the system prompt.
    Instructions,
    /// The built-in tool definitions.
    BuiltInTools,
    /// The MCP tool definitions.
    McpTools,
    /// The provider's per-tool wire envelope.
    ToolMetadata,
    /// The conversation history.
    History,
}

impl ContextCategory {
    /// Every category, in the order the panel lists the rows.
    pub const ALL: [Self; 8] = [
        Self::History,
        Self::SystemPrompt,
        Self::Skills,
        Self::Memory,
        Self::Instructions,
        Self::BuiltInTools,
        Self::McpTools,
        Self::ToolMetadata,
    ];

    /// The row's label.
    ///
    /// # Returns
    ///
    /// A short title-case name, e.g. `"MCP tools"`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SystemPrompt => "System prompt",
            Self::Skills => "Skills",
            Self::Memory => "Memory",
            Self::Instructions => "Instruction files",
            Self::BuiltInTools => "Built-in tools",
            Self::McpTools => "MCP tools",
            Self::ToolMetadata => "Tool metadata",
            Self::History => "History",
        }
    }
}

/// One partition of the Context panel.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextRow {
    /// Which partition the row shows.
    pub category: ContextCategory,
    /// Token estimate of the partition.
    pub tokens: u64,
    /// Share of the context window, or `None` when the window is unknown.
    pub percent: Option<f64>,
    /// The items inside the partition, largest first; empty when the
    /// partition has no item breakdown.
    pub items: Vec<ContextItem>,
}

/// What the Context panel shows, built by [`context_view`].
#[derive(Clone, Debug, PartialEq)]
pub struct ContextView {
    /// Tokens of every partition together.
    pub used_tokens: u64,
    /// The model's context window, or `None` when unknown.
    pub window_tokens: Option<u64>,
    /// Share of the window used, or `None` when the window is unknown.
    pub used_percent: Option<f64>,
    /// Tokens left in the window, or `None` when the window is unknown.
    pub free_tokens: Option<u64>,
    /// Share of the window left, or `None` when the window is unknown.
    pub free_percent: Option<f64>,
    /// Provider-reported input tokens of the last turn (zero before it).
    pub sent_tokens: u64,
    /// Messages in the conversation history.
    pub message_count: usize,
    /// The partitions, in [`ContextCategory::ALL`] order.
    pub rows: Vec<ContextRow>,
    /// Bytes streamed to the model this session.
    pub stream_out_bytes: u64,
    /// Bytes streamed from the model this session.
    pub stream_in_bytes: u64,
}

impl ContextView {
    /// The panel's text, one string per line, as the copy source of its
    /// text selection.
    ///
    /// # Returns
    ///
    /// The header, the sent tokens, each row followed by its items
    /// (indented), the message count, the free space and the stream
    /// counters.
    #[must_use]
    pub fn text_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Context window  {}", self.usage_text()),
            format!("Sent to model  {}", format_tokens(self.sent_tokens)),
        ];
        for row in &self.rows {
            lines.push(format!(
                "{}  {}  {}",
                row.category.label(),
                format_tokens(row.tokens),
                format_percent(row.percent)
            ));
            lines.extend(
                row.items
                    .iter()
                    .map(|item| format!("  {}  {}", item.name, format_tokens(item.tokens))),
            );
            if row.category == ContextCategory::History {
                lines.push(format!("  {}", self.messages_text()));
            }
        }
        lines.push(format!(
            "Free space  {}  {}",
            self.free_tokens
                .map_or_else(|| "unknown".to_string(), format_tokens),
            format_percent(self.free_percent)
        ));
        lines.push(format!("Streamed  {}", self.streamed_text()));
        lines
    }

    /// The usage summary, e.g. `31% used  311.5k / 1.0M`, or
    /// `311.5k used  window unknown`.
    ///
    /// # Returns
    ///
    /// The header's text.
    #[must_use]
    pub fn usage_text(&self) -> String {
        match (self.used_percent, self.window_tokens) {
            (Some(pct), Some(window)) => format!(
                "{pct:.0}% used  {} / {}",
                format_tokens(self.used_tokens),
                format_tokens(window)
            ),
            _ => format!("{} used  window unknown", format_tokens(self.used_tokens)),
        }
    }

    /// The history message count, e.g. `3 messages`.
    ///
    /// # Returns
    ///
    /// The count with its unit.
    #[must_use]
    pub fn messages_text(&self) -> String {
        format!("{} messages", self.message_count)
    }

    /// The stream counters, e.g. `1.0 KB up  2.0 KB down`.
    ///
    /// # Returns
    ///
    /// The bytes streamed each way, in kilobytes.
    #[must_use]
    pub fn streamed_text(&self) -> String {
        let kb = |bytes: u64| format!("{:.1} KB", bytes as f64 / 1024.0);
        format!(
            "{} up  {} down",
            kb(self.stream_out_bytes),
            kb(self.stream_in_bytes)
        )
    }
}

/// Build the Context panel's view model from `snapshot`.
///
/// # Arguments
///
/// * `snapshot` - the context partitions, normally
///   `App::context_effective_snapshot`.
/// * `stream_out_bytes` - bytes streamed to the model this session.
/// * `stream_in_bytes` - bytes streamed from the model this session.
///
/// # Returns
///
/// The view; its rows sum to `snapshot.total_tokens()`.
#[must_use]
pub fn context_view(
    snapshot: &ContextPartitionSnapshot,
    stream_out_bytes: u64,
    stream_in_bytes: u64,
) -> ContextView {
    let items = &snapshot.items;
    let mcp_tokens: u64 = items.mcp_tools.iter().map(|item| item.tokens).sum();
    let prompt_parts = snapshot.skills_tokens + snapshot.memory_tokens + snapshot.agents_md_tokens;
    let rows = ContextCategory::ALL
        .into_iter()
        .map(|category| {
            let (tokens, items) = match category {
                ContextCategory::SystemPrompt => (
                    snapshot.system_prompt_tokens.saturating_sub(prompt_parts),
                    Vec::new(),
                ),
                ContextCategory::Skills => (snapshot.skills_tokens, items.skills.clone()),
                ContextCategory::Memory => (snapshot.memory_tokens, items.memories.clone()),
                ContextCategory::Instructions => {
                    (snapshot.agents_md_tokens, items.instruction_files.clone())
                }
                ContextCategory::BuiltInTools => (
                    snapshot.tool_catalog_tokens.saturating_sub(mcp_tokens),
                    items.built_in_tools.clone(),
                ),
                ContextCategory::McpTools => (mcp_tokens, items.mcp_tools.clone()),
                ContextCategory::ToolMetadata => (snapshot.tool_metadata_tokens, Vec::new()),
                ContextCategory::History => (snapshot.history_tokens, items.history.clone()),
            };
            ContextRow {
                category,
                tokens,
                percent: snapshot.percent_of_window(tokens),
                items,
            }
        })
        .collect();
    let free_tokens = snapshot.remaining_tokens();
    ContextView {
        used_tokens: snapshot.total_tokens(),
        window_tokens: snapshot.context_window_tokens.map(|w| w as u64),
        used_percent: snapshot.total_percent(),
        free_tokens,
        free_percent: free_tokens.and_then(|free| snapshot.percent_of_window(free)),
        sent_tokens: snapshot.last_input_tokens,
        message_count: snapshot.history_message_count,
        rows,
        stream_out_bytes,
        stream_in_bytes,
    }
}
