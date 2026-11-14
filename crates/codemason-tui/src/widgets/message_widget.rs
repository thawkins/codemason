//! Widget for rendering a single chat message.
//!
//! Formats user and assistant messages with role-colored prefixes, renders
//! tool call status indicators and reasoning blocks with distinct styles.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use codemason_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use codemason_types::sanitize_terminal::sanitize_terminal;

use crate::adapters::to_ratatui_line;
use crate::app::short_id;
pub use crate::message_text::*;

/// Render an agent-notice bubble as ratatui lines (see
/// [`crate::message_text::render_agent_notice_lines`]).
pub(crate) fn agent_notice_ratatui_lines(text: &str) -> Vec<Line<'static>> {
    crate::message_text::render_agent_notice_lines(text)
        .iter()
        .map(to_ratatui_line)
        .collect()
}

/// [`scrub_narrated_toolcalls_by`] over ratatui [`Line`]s.
pub(crate) fn scrub_narrated_ratatui_lines(lines: Vec<Line<'_>>) -> Vec<Line<'_>> {
    scrub_narrated_toolcalls_by(lines, |line| {
        line.spans.iter().map(|s| s.content.to_string()).collect()
    })
}

/// Parse a `[red]...[/red]` span marker into a red-styled [`Line`], or `None`
/// when the line has no complete marker pair.
///
/// Slash-command output (e.g. the `/tools` list of disabled tools) uses
/// `[red]` markers instead of markdown, because the markdown pass strips
/// unknown tags to plain text.
pub(crate) fn parse_red_marker(line: &str) -> Option<Line<'static>> {
    const OPEN: &str = "[red]";
    const CLOSE: &str = "[/red]";
    let start = line.find(OPEN)?;
    let body_start = start + OPEN.len();
    let rel_end = line[body_start..].find(CLOSE)?;
    let body = line[body_start..body_start + rel_end].to_string();
    if body.is_empty() {
        return None;
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    if start > 0 {
        spans.push(Span::raw(line[..start].to_string()));
    }
    spans.push(Span::styled(body, Style::default().fg(Color::Red)));
    let tail = &line[body_start + rel_end + CLOSE.len()..];
    if !tail.is_empty() {
        spans.push(Span::raw(tail.to_string()));
    }
    Some(Line::from(spans))
}

use crate::theme;

pub use crate::text_utils::{pluralize, truncate_str};

/// A widget that renders a single `Message` as a list of styled lines.
pub struct MessageWidget<'a> {
    message: &'a Message,
    cwd: &'a str,
    tool_step_map: &'a std::collections::HashMap<String, (String, u32, u32)>,
}

impl<'a> MessageWidget<'a> {
    /// Create a new [`MessageWidget`] for the given message reference.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to render.
    /// * `cwd` - Current working directory, used to make file paths relative.
    /// * `tool_step_map` - Mapping from tool call IDs to `(short_session_id, step_number, sub_step)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use codemason_agent::message::{Message, MessagePart, Role};
    /// use codemason_tui::widgets::message_widget::MessageWidget;
    ///
    /// let msg = Message::new(
    ///     "session-1",
    ///     Role::User,
    ///     vec![MessagePart::Text { text: "Hello!".into() }],
    /// );
    /// let map = HashMap::new();
    /// let widget = MessageWidget::new(&msg, "/home/user/project", &map);
    /// ```
    pub fn new(
        message: &'a Message,
        cwd: &'a str,
        tool_step_map: &'a std::collections::HashMap<String, (String, u32, u32)>,
    ) -> Self {
        Self {
            message,
            cwd,
            tool_step_map,
        }
    }

    /// Convert the message into rendered ratatui [`Line`]s.
    pub fn to_lines(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        for part in &self.message.parts {
            match part {
                MessagePart::Text { text } => {
                    // Agent notices are rendered in bright yellow, one item per line.
                    if self.message.role == Role::Assistant && is_agent_notice(text) {
                        lines.extend(agent_notice_ratatui_lines(text));
                        continue;
                    }
                    // Detect the model "thinking out loud" with minimal text like "..." or "......"
                    // and render it as a distinct greyed-out thinking indicator.
                    let trimmed = text.trim();
                    let is_thinking_placeholder = trimmed.chars().all(|c| c == '.' || c == ' ')
                        && !trimmed.is_empty()
                        && trimmed.len() <= 6;
                    if is_thinking_placeholder && self.message.role == Role::Assistant {
                        lines.push(Line::from(vec![
                            Span::styled("", theme::think()),
                            Span::styled("Thinking ...", theme::think()),
                        ]));
                        continue;
                    }
                    let (dot, dot_style, indent) = match self.message.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        Role::Compaction => (
                            // Sub-agent / primary context-refresh summaries are
                            // NOT the agent's own output: label them so a
                            // mid-run compaction summary is never mistaken for
                            // a completion report in the output overlay.
                            "[compaction] ",
                            Style::default().fg(Color::DarkGray),
                            13,
                        ),
                        Role::Assistant => (
                            "",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            0,
                        ),
                    };
                    // SEC-codemason-types-001 (SECTASKS T-023): assistant text is
                    // model output that can embed an escape sequence from a
                    // tool result or fetched page. Neutralise it at this render
                    // boundary before it reaches the terminal.
                    let sanitised_text = sanitize_terminal(text);
                    for (i, line) in sanitised_text.as_ref().lines().enumerate() {
                        // A `[red]...[/red]` span marker (e.g. a tool disabled by
                        // a visibility switch in `/tools`) renders red and
                        // takes precedence over the plain-raw path below.
                        if let Some(styled) = parse_red_marker(line) {
                            let mut spans = Vec::with_capacity(styled.spans.len() + 2);
                            if i == 0 {
                                if self.message.role == Role::User && !lines.is_empty() {
                                    lines.push(Line::from(String::new()));
                                }
                                spans.push(Span::styled(dot, dot_style));
                            } else {
                                spans.push(Span::raw(&INDENT_SPACES[..indent]));
                            }
                            spans.extend(styled.spans);
                            lines.push(Line::from(spans));
                            continue;
                        }
                        if i == 0 {
                            // Blank line before the "You:" prompt for visual
                            // separation - but not when this is the very
                            // first rendered line of the transcript (a
                            // leading blank row would push everything down).
                            if self.message.role == Role::User && !lines.is_empty() {
                                lines.push(Line::from(String::new()));
                            }
                            lines.push(Line::from(vec![
                                Span::styled(dot, dot_style),
                                Span::raw(line.to_string()),
                            ]));
                        } else {
                            // PERF-047: indentation is emitted as its own
                            // borrowed span instead of `format!`-ing a fresh
                            // String with `repeat()` per line (two allocations
                            // saved per continuation line).
                            lines.push(Line::from(vec![
                                Span::raw(&INDENT_SPACES[..indent]),
                                Span::raw(line.to_string()),
                            ]));
                        }
                    }
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(_sid, step, substep)| format!("[{step}.{substep}] "))
                        .unwrap_or_default();
                    let (indicator, ind_style, name_style) = match state.status {
                        ToolCallStatus::Completed => (
                            "",
                            Style::default().fg(Color::Green),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Error => (
                            "",
                            Style::default().fg(Color::Red),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "",
                            Style::default().fg(Color::DarkGray),
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, self.cwd);

                    // Build the inline diff stats for edit tool (e.g. "(+25 -5)")
                    let inline_diff = if state.status == ToolCallStatus::Completed {
                        tool_inline_diff(tool, &state.output)
                    } else {
                        None
                    };

                    let mut spans = vec![
                        Span::styled(indicator, ind_style),
                        Span::styled(
                            step_tag,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    // The tool name always comes immediately after the step
                    // counter, with every other parameter rendered after it.
                    //
                    // A `think` call renders its own thought under the header
                    // from its tool state, so the header keeps the name only -
                    // unlike the other tools there is no input summary to
                    // print, and printing the model's full argument inline
                    // would duplicate the lines that follow.
                    let (name_span, argument_span) = if canonical_tool_name(tool) == "think" {
                        (display_name, None)
                    } else if summary.is_empty() {
                        (display_name, None)
                    } else {
                        (
                            format!("{} ", display_name),
                            Some(Span::styled(summary, Style::default().fg(Color::DarkGray))),
                        )
                    };
                    spans.push(Span::styled(name_span, name_style));
                    if let Some(argument) = argument_span {
                        spans.push(argument);
                    }
                    // Show line range for read tool (and aliases) in bold
                    if canonical_tool_name(tool) == "read" {
                        if let Some(range) = read_line_range(&state.output) {
                            spans.push(Span::styled(
                                format!(" {}", range),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                    if let Some((added, removed)) = inline_diff {
                        spans.push(Span::styled(" (", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("+{}", added),
                            Style::default().fg(Color::Green),
                        ));
                        spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("-{}", removed),
                            Style::default().fg(Color::Red),
                        ));
                        spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
                    }
                    // Add duration_ms display for completed tool calls
                    if state.status == ToolCallStatus::Completed {
                        if let Some(duration_ms) = state.duration_ms {
                            let duration_str = if duration_ms < 1000 {
                                format!(" ({}ms)", duration_ms)
                            } else {
                                format!(" ({:.1}s)", duration_ms as f64 / 1000.0)
                            };
                            spans.push(Span::styled(
                                duration_str,
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    lines.push(Line::from(spans));
                    if state.status == ToolCallStatus::Completed {
                        if tool == "wait_agents" {
                            // Special handling for wait_agents: show indented list of tasks
                            if let Some(tasks_array) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("tasks"))
                                .and_then(|t| t.as_array())
                            {
                                for task in tasks_array {
                                    let agent = task
                                        .get("agent")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("unknown");
                                    let task_id = task
                                        .get("id")
                                        .and_then(|id| id.as_str())
                                        .map(short_id)
                                        .unwrap_or_else(|| "unknown".to_string());
                                    let elapsed_ms = task
                                        .get("elapsed_ms")
                                        .and_then(|e| e.as_u64())
                                        .unwrap_or(0);
                                    let output_lines = task
                                        .get("output_lines")
                                        .and_then(|l| l.as_u64())
                                        .unwrap_or(0)
                                        as usize;
                                    let task_status = task
                                        .get("status")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("unknown");

                                    let status_icon = if task_status == "completed" {
                                        "✓"
                                    } else {
                                        "✗"
                                    };

                                    let elapsed_sec = elapsed_ms as f64 / 1000.0;
                                    let task_line = format!(
                                        "  {} {} ({}): {}s, {} line(s)",
                                        status_icon, agent, task_id, elapsed_sec, output_lines
                                    );

                                    lines.push(Line::from(Span::styled(
                                        task_line,
                                        if task_status == "completed" {
                                            Style::default().fg(Color::Green)
                                        } else {
                                            Style::default().fg(Color::Red)
                                        },
                                    )));
                                }
                            }
                        } else if tool == "think" {
                            // Render full thought text multi-line
                            if let Some(thought) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("thought"))
                                .and_then(|v| v.as_str())
                                .or_else(|| {
                                    state
                                        .output
                                        .as_ref()
                                        .and_then(|out| out.get("thinking"))
                                        .and_then(|v| v.as_str())
                                })
                                .or_else(|| {
                                    state
                                        .output
                                        .as_ref()
                                        .and_then(|out| out.get("text"))
                                        .and_then(|v| v.as_str())
                                })
                                .or_else(|| {
                                    state
                                        .input
                                        .get("thought")
                                        .and_then(|v| v.as_str())
                                        .filter(|_| state.output.is_none())
                                })
                            {
                                let mut thought_lines = Vec::new();
                                for line in thought.lines() {
                                    thought_lines.push(Line::from(Span::styled(
                                        format!("  {}", line),
                                        theme::think(),
                                    )));
                                }
                                // A model may narrate its next tool call as
                                // markup INSIDE the thought; the loop recovers
                                // and executes that call, so the transcript
                                // shows the executed call instead of the raw
                                // narration.
                                lines.extend(scrub_narrated_ratatui_lines(thought_lines));
                            }
                        } else if tool == "agent_complete" {
                            // Render the full task completion summary
                            if let Some(summary) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("summary"))
                                .and_then(|v| v.as_str())
                            {
                                for line in summary.lines() {
                                    lines.push(Line::from(Span::styled(
                                        format!("  {}", line),
                                        Style::default().fg(Color::Green),
                                    )));
                                }
                            }
                        } else if tool == "multiedit" || tool == "multi_edit" {
                            // Render per-file edit stats as a tabular list
                            if let Some(file_stats) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("file_stats"))
                                .and_then(|v| v.as_array())
                            {
                                // Find the longest relative path for alignment
                                let rel_paths: Vec<String> = file_stats
                                    .iter()
                                    .map(|fs| {
                                        fs.get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| make_relative_path(p, self.cwd))
                                            .unwrap_or_default()
                                    })
                                    .collect();
                                let max_len = rel_paths.iter().map(|p| p.len()).max().unwrap_or(0);
                                for (fs, rel_path) in file_stats.iter().zip(rel_paths.iter()) {
                                    let added =
                                        fs.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let removed =
                                        fs.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let padding =
                                        " ".repeat(max_len.saturating_sub(rel_path.len()));
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            format!("  └ {}{} ", rel_path, padding),
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(
                                            format!("+{}", added),
                                            Style::default().fg(Color::Green),
                                        ),
                                        Span::styled(" ", Style::default()),
                                        Span::styled(
                                            format!("-{}", removed),
                                            Style::default().fg(Color::Red),
                                        ),
                                    ]));
                                }
                            } else if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, self.cwd)
                            {
                                lines.push(Line::from(Span::styled(
                                    format!("  └ {}", result),
                                    Style::default().fg(Color::Gray),
                                )));
                            }
                        } else if tool != "Edit" && tool != "think" {
                            // Skip result summary for edit tool on success (already shows inline diff)
                            // Skip result summary for think tool (full thought rendered above,
                            // same as Reasoning parts)
                            if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, self.cwd)
                            {
                                lines.push(Line::from(Span::styled(
                                    format!("  └ {}", result),
                                    Style::default().fg(Color::Gray),
                                )));
                            }
                        }
                    }
                    if state.status == ToolCallStatus::Error {
                        let err_msg = state
                            .error
                            .as_deref()
                            .unwrap_or("Tool execution failed (no error details available)");
                        lines.push(Line::from(Span::styled(
                            format!("  └ Error: {}", err_msg),
                            Style::default().fg(Color::Red),
                        )));
                    }
                }
                MessagePart::Reasoning { text, .. } => {
                    let mut reasoning_lines = Vec::new();
                    for line in text.lines() {
                        reasoning_lines.push(Line::from(Span::styled(
                            format!("  {}", line),
                            theme::think(),
                        )));
                    }
                    // Reasoning text is also a place models narrate tool-call
                    // markup into; scrub it the same way as think outputs.
                    lines.extend(scrub_narrated_ratatui_lines(reasoning_lines));
                }
                MessagePart::Image(img) => {
                    let name = img
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("image");
                    lines.push(Line::from(Span::styled(
                        format!("  [image: {}]", name),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
        }

        lines
    }
}

impl Widget for MessageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.to_lines();
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}
