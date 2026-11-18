//! Frontend-neutral lines of the Context panel (toggled with `Alt+C`,
//! spec `contextpanel`).
//!
//! [`context_panel_lines`] builds what the removed terminal UI's
//! `render_context_panel` drew, as [`StyledLine`]s a frontend paints: the
//! model's context window, the tokens sent on the last turn, every context
//! partition (the system prompt with its skills, memory and AGENTS.md
//! breakdown, the tool catalog, the tool metadata and the history) with a
//! percentage bar of the window (FR-005..FR-011), the history message count
//! (FR-008), the total and the free headroom (FR-012), and the stream I/O
//! byte counts. Ratios read `unknown` when the model advertises no window
//! (FR-011).
//!
//! The lines are a UI-only artifact: building them reads the snapshot and
//! never touches the conversation (FR-016).
//!
//! Dependencies: [`crate::app::ContextPartitionSnapshot`] for the values and
//! [`crate::styled`] for the line type.

use crate::app::ContextPartitionSnapshot;
use crate::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};

/// Left margin of every row, as in the TUI.
const MARGIN: &str = "    ";

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

/// An ASCII progress bar, e.g. `[#####.....]` for half of ten cells.
///
/// # Arguments
///
/// * `progress` - the filled fraction, clamped to `0.0..=1.0`.
/// * `width` - the number of cells between the brackets.
///
/// # Returns
///
/// `[`, the filled cells as `#`, the empty cells as `.`, then `]`.
#[must_use]
pub fn progress_bar(progress: f32, width: usize) -> String {
    let filled = ((progress.clamp(0.0, 1.0) * width as f32) as usize).min(width);
    format!(
        "[{}{}]",
        "#".repeat(filled),
        ".".repeat(width.saturating_sub(filled))
    )
}

/// The row styles, as the TUI colours them.
struct Styles {
    header: TextStyle,
    label: TextStyle,
    value: TextStyle,
    sub: TextStyle,
    dim: TextStyle,
    warn: TextStyle,
}

impl Styles {
    /// The TUI's Context panel colours.
    const fn new() -> Self {
        Self {
            header: TextStyle::new().fg(ThemeColor::Accent).bold(),
            label: TextStyle::new().fg(ThemeColor::Text),
            value: TextStyle::new().fg(ThemeColor::Info),
            sub: TextStyle::new().fg(ThemeColor::Subtle),
            dim: TextStyle::new().fg(ThemeColor::Muted),
            warn: TextStyle::new().fg(ThemeColor::Warning),
        }
    }
}

/// A `label  NNNtk [bar] PP%` row, or `label  NNNtk unknown` when the
/// window is unknown.
fn partition_row(
    snapshot: &ContextPartitionSnapshot,
    label: &str,
    tokens: u64,
    label_style: TextStyle,
    bar_width: usize,
    styles: &Styles,
) -> StyledLine {
    let mut spans = vec![
        StyledSpan::styled(format!("{MARGIN}{label:<14}"), label_style),
        StyledSpan::styled(format!("{:>8}tk ", format_tokens(tokens)), styles.value),
    ];
    match snapshot.percent_of_window(tokens) {
        Some(pct) => {
            spans.push(StyledSpan::styled(
                progress_bar((pct / 100.0) as f32, bar_width),
                styles.dim,
            ));
            spans.push(StyledSpan::styled(format!("{pct:4.0}%"), styles.value));
        }
        None => spans.push(StyledSpan::styled("unknown", styles.sub)),
    }
    StyledLine::from(spans)
}

/// Build the Context panel's lines from `snapshot`.
///
/// # Arguments
///
/// * `snapshot` - the context partitions, normally
///   `App::context_effective_snapshot`.
/// * `stream_out_bytes` - bytes streamed to the model this session.
/// * `stream_in_bytes` - bytes streamed from the model this session.
/// * `bar_width` - cells in each percentage bar.
///
/// # Returns
///
/// The panel's rows, top to bottom, starting with two blank lines as in the
/// TUI.
#[must_use]
pub fn context_panel_lines(
    snapshot: &ContextPartitionSnapshot,
    stream_out_bytes: u64,
    stream_in_bytes: u64,
    bar_width: usize,
) -> Vec<StyledLine> {
    let styles = Styles::new();
    let mut lines = vec![StyledLine::default(), StyledLine::default()];

    // FR-010/FR-011: the window every percentage below is measured against.
    let window_label = "Context window";
    lines.push(StyledLine::from(vec![
        StyledSpan::styled(format!("{MARGIN}{window_label:<14}"), styles.label),
        match snapshot.context_window_tokens {
            Some(window) => StyledSpan::styled(
                format!("{:>8}tk", format_tokens(window as u64)),
                styles.value,
            ),
            None => StyledSpan::styled("unknown", styles.sub),
        },
    ]));

    // The provider-reported input of the last turn; zero (dimmed, no bar)
    // until the first turn completes.
    let sent = snapshot.last_input_tokens;
    let sent_label = "Sent to model";
    lines.push(match snapshot.percent_of_window(sent) {
        Some(_) if sent > 0 => {
            partition_row(snapshot, sent_label, sent, styles.label, bar_width, &styles)
        }
        Some(_) => StyledLine::from(vec![
            StyledSpan::styled(format!("{MARGIN}{sent_label:<14}"), styles.sub),
            StyledSpan::styled(format!("{:>8}tk", format_tokens(sent)), styles.sub),
        ]),
        None => StyledLine::from(vec![
            StyledSpan::styled(format!("{MARGIN}{sent_label:<14}"), styles.label),
            StyledSpan::styled(format!("{:>8}tk", format_tokens(sent)), styles.value),
        ]),
    });

    // FR-005..FR-009: the partitions; the indented rows break down the
    // system prompt.
    let rows: [(&str, u64, bool); 7] = [
        ("System prompt", snapshot.system_prompt_tokens, false),
        ("skills", snapshot.skills_tokens, true),
        ("memory", snapshot.memory_tokens, true),
        ("agents.md", snapshot.agents_md_tokens, true),
        ("Tool catalog", snapshot.tool_catalog_tokens, false),
        ("Tool metadata", snapshot.tool_metadata_tokens, false),
        ("History", snapshot.history_tokens, false),
    ];
    for (label, tokens, sub) in rows {
        let (label, style) = if sub {
            (format!("   {label}"), styles.sub)
        } else {
            (label.to_string(), styles.label)
        };
        lines.push(partition_row(
            snapshot, &label, tokens, style, bar_width, &styles,
        ));
    }

    // FR-008: the message count beside the history partition.
    lines.push(StyledLine::from(StyledSpan::styled(
        format!("{MARGIN}   {} messages", snapshot.history_message_count),
        styles.sub,
    )));
    lines.push(StyledLine::default());

    // FR-012: the total and the remaining headroom.
    lines.push(partition_row(
        snapshot,
        "Total",
        snapshot.total_tokens(),
        styles.header,
        bar_width,
        &styles,
    ));
    lines.push(match snapshot.remaining_tokens() {
        Some(remaining) => StyledLine::from(vec![
            StyledSpan::styled(format!("{MARGIN}{:<14}", "Free"), styles.label),
            StyledSpan::styled(
                format_tokens(remaining),
                if remaining == 0 {
                    styles.warn
                } else {
                    styles.value
                },
            ),
        ]),
        None => StyledLine::from(StyledSpan::styled(
            format!("{MARGIN}Free: unknown"),
            styles.sub,
        )),
    });

    // The stream I/O counters, two lines below.
    lines.push(StyledLine::default());
    lines.push(StyledLine::default());
    let kb = |bytes: u64| format!("{:.1}KB", bytes as f64 / 1024.0);
    lines.push(StyledLine::from(StyledSpan::styled(
        format!(
            "{MARGIN}io: ^{} v{}",
            kb(stream_out_bytes),
            kb(stream_in_bytes)
        ),
        styles.value,
    )));
    lines
}
