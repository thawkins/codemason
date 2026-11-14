//! Rendering of chat messages into styled lines, and the copy-row refresh.
//!
//! Turns [`Message`]s into frontend-neutral [`StyledLine`]s (role prefixes,
//! tool-call headers and summaries, reasoning, images) and keeps the
//! per-message line cache's plain-text copy rows fresh
//! ([`ensure_message_copy_lines`]). The terminal frontend paints the cached
//! rows; this module never paints.
//!
//! Depends on `codemason-agent` (messages), `codemason-types`
//! (`sanitize_terminal`), [`crate::message_text`] (tool summaries) and
//! [`crate::line_wrap`] (row wrapping).

use std::collections::HashMap;
use std::hash::BuildHasher;

use crate::styled::{StyledLine, StyledSpan, TextStyle, ThemeColor};
use codemason_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use codemason_types::sanitize_terminal::sanitize_terminal;

use crate::app::App;
use crate::line_wrap::{wrap_line_styled, wrapped_lines_to_strings};
use crate::message_text::{
    ICON_ATTACHMENT, INDENT_SPACES, THINK_STYLE, canonical_tool_name, capitalize_tool_name,
    is_agent_notice, make_relative_path, read_line_range, render_agent_notice_lines,
    scrub_narrated_toolcalls, tool_inline_diff, tool_input_summary, tool_result_summary,
};

/// Render a single message into formatted lines (FR-003, FR-006).
///
/// Delegates to `messages_to_lines` with a single-message slice so the
/// per-message line cache can re-render only the changed message during
/// streaming instead of rebuilding the entire timeline.
pub fn message_to_lines<S1: BuildHasher, S2: BuildHasher>(
    msg: &Message,
    tool_step_map: &HashMap<String, (String, u32, u32), S1>,
    sid_to_display: &HashMap<String, String, S2>,
    cwd: &str,
) -> Vec<StyledLine> {
    messages_to_lines(
        std::slice::from_ref(msg),
        tool_step_map,
        sid_to_display,
        cwd,
    )
}

/// The status label of each tool call in `msg`, in part order.
///
/// Each label is `"<Tool>: <status>"`, the tool name as the transcript
/// header capitalises it and the status as [`ToolCallStatus`] displays it
/// (`pending`, `running`, `completed`, `error`). The terminal shows the
/// status by colour and glyph only; a GUI uses these labels to name it in
/// text (for example for assistive technology).
pub fn tool_call_status_labels(msg: &Message) -> Vec<String> {
    msg.parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall { tool, state, .. } => {
                Some(format!("{}: {}", capitalize_tool_name(tool), state.status))
            }
            MessagePart::Text { .. } | MessagePart::Reasoning { .. } | MessagePart::Image(_) => {
                None
            }
        })
        .collect()
}

/// Build a `(call_id -> (short_sid, step, substep))` map from the
/// message transcript itself. This lets storage-backed sessions (sub-agents,
/// teammates, resumed sessions) render the same `[N.M]` step/parallel-tool
/// prefixes as the primary live session, without relying on the transient
/// in-memory `tool_step_map` populated from `ToolCallStart` events.
pub fn build_step_map_for_messages(
    messages: &[Message],
) -> std::collections::HashMap<String, (String, u32, u32)> {
    let mut map = std::collections::HashMap::new();
    let mut step = 0u32;
    for msg in messages {
        if msg.role != Role::Assistant {
            continue;
        }
        let tool_call_count = msg
            .parts
            .iter()
            .filter(|p| matches!(p, MessagePart::ToolCall { .. }))
            .count();
        if tool_call_count == 0 {
            continue;
        }
        step += 1;
        let mut substep = 0u32;
        for part in &msg.parts {
            if let MessagePart::ToolCall { call_id, .. } = part {
                substep += 1;
                let short_sid = {
                    let s = &msg.session_id;
                    let start = s.char_indices().rev().nth(7).map(|(i, _)| i).unwrap_or(0);
                    s[start..].to_string()
                };
                map.insert(call_id.clone(), (short_sid, step, substep));
            }
        }
    }
    map
}

/// Render a slice of messages into formatted lines using the rich format
/// from the primary Messages panel.  Both `render_messages` and
/// `render_output_view_overlay` delegate here so teammate output looks
/// identical to the lead agent's chat window.
pub fn messages_to_lines<S1: BuildHasher, S2: BuildHasher>(
    messages: &[Message],
    tool_step_map: &HashMap<String, (String, u32, u32), S1>,
    #[allow(clippy::used_underscore_binding)] _sid_to_display: &HashMap<String, String, S2>,
    cwd: &str,
) -> Vec<StyledLine> {
    let mut lines: Vec<StyledLine> = Vec::new();

    for msg in messages {
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    // Agent notices are rendered in bright yellow, one item per line.
                    if msg.role == Role::Assistant && is_agent_notice(text) {
                        lines.extend(render_agent_notice_lines(text));
                        continue;
                    }

                    // SEC-080: this is the LIVE transcript renderer, which
                    // previously emitted model/tool text via `Span::raw` with no
                    // neutralisation (only the log panel sanitised). Assistant
                    // text can carry an escape sequence copied from a tool result
                    // or fetched page, so neutralise it here as well.
                    let text = sanitize_terminal(text);
                    let text: &str = &text;

                    let (dot, dot_style, indent) = match msg.role {
                        Role::User => ("You: ", TextStyle::new().fg(ThemeColor::Cyan).bold(), 5),
                        // Context-compaction summaries are internal
                        // bookkeeping, not the agent's own output: label them
                        // so a mid-run auto-compaction summary (which uses the
                        // same "## Objective / ## Work State" shape as a
                        // sub-agent's final report) is never mistaken for the
                        // completion report in the output overlay.
                        Role::Compaction => (
                            "[compaction] ",
                            TextStyle::new().fg(ThemeColor::DarkGray),
                            13,
                        ),
                        Role::Assistant => {
                            ("* ", TextStyle::new().fg(ThemeColor::Magenta).bold(), 2)
                        }
                    };
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            // Blank line before the "You:" prompt for visual separation.
                            if msg.role == Role::User {
                                lines.push(StyledLine::default());
                            }
                            lines.push(StyledLine::from(vec![
                                StyledSpan::styled(dot, dot_style),
                                StyledSpan::raw(line.to_owned()),
                            ]));
                        } else {
                            // PERF-047: indentation is emitted as its own
                            // borrowed span instead of `format!`-ing a fresh
                            // String with `repeat()` per line (two allocations
                            // saved per continuation line).
                            lines.push(StyledLine::from(vec![
                                StyledSpan::raw(&INDENT_SPACES[..indent]),
                                StyledSpan::raw(line.to_string()),
                            ]));
                        }
                    }
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    let step_tag = if let Some((_sid, step, substep)) = tool_step_map.get(call_id) {
                        format!("[{step}.{substep}] ")
                    } else {
                        String::new()
                    };
                    let (indicator, ind_style, name_style) = match state.status {
                        ToolCallStatus::Completed => (
                            "* ",
                            TextStyle::new().fg(ThemeColor::Green),
                            TextStyle::new().fg(ThemeColor::Green).bold(),
                        ),
                        ToolCallStatus::Error => (
                            "[x] ",
                            TextStyle::new().fg(ThemeColor::Red),
                            TextStyle::new().fg(ThemeColor::Red).bold(),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "* ",
                            TextStyle::new().fg(ThemeColor::DarkGray),
                            TextStyle::new().fg(ThemeColor::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, cwd);

                    let inline_diff = if state.status == ToolCallStatus::Completed {
                        tool_inline_diff(tool, &state.output)
                    } else {
                        None
                    };

                    let mut spans = vec![
                        StyledSpan::styled(indicator, ind_style),
                        StyledSpan::styled(
                            step_tag,
                            TextStyle::new().fg(ThemeColor::Yellow).bold(),
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
                            Some(StyledSpan::styled(
                                summary,
                                TextStyle::new().fg(ThemeColor::DarkGray),
                            )),
                        )
                    };
                    spans.push(StyledSpan::styled(name_span, name_style));
                    if let Some(argument) = argument_span {
                        spans.push(argument);
                    }
                    if tool == "read" {
                        if let Some(range) = read_line_range(&state.output) {
                            spans.push(StyledSpan::styled(
                                format!(" {}", range),
                                TextStyle::new().fg(ThemeColor::White).bold(),
                            ));
                        }
                    }
                    if let Some((added, removed)) = inline_diff {
                        spans.push(StyledSpan::styled(
                            " (",
                            TextStyle::new().fg(ThemeColor::DarkGray),
                        ));
                        spans.push(StyledSpan::styled(
                            format!("+{}", added),
                            TextStyle::new().fg(ThemeColor::Green),
                        ));
                        spans.push(StyledSpan::styled(
                            " ",
                            TextStyle::new().fg(ThemeColor::DarkGray),
                        ));
                        spans.push(StyledSpan::styled(
                            format!("-{}", removed),
                            TextStyle::new().fg(ThemeColor::Red),
                        ));
                        spans.push(StyledSpan::styled(
                            ")",
                            TextStyle::new().fg(ThemeColor::DarkGray),
                        ));
                    }
                    // Add duration_ms display for completed tool calls
                    if state.status == ToolCallStatus::Completed {
                        if let Some(duration_ms) = state.duration_ms {
                            let duration_str = if duration_ms < 1000 {
                                format!(" ({}ms)", duration_ms)
                            } else {
                                format!(" ({:.1}s)", duration_ms as f64 / 1000.0)
                            };
                            spans.push(StyledSpan::styled(
                                duration_str,
                                TextStyle::new().fg(ThemeColor::DarkGray),
                            ));
                        }
                    }
                    lines.push(StyledLine::from(spans));

                    if state.status == ToolCallStatus::Completed {
                        if tool == "think" {
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
                                // SEC-080: `thought` is tool output; neutralise
                                // any embedded terminal escapes before rendering.
                                let thought = sanitize_terminal(thought);
                                let mut thought_lines = Vec::new();
                                for line in thought.as_ref().lines() {
                                    thought_lines.push(StyledLine::from(StyledSpan::styled(
                                        format!("  {}", line),
                                        THINK_STYLE,
                                    )));
                                }
                                // A model may narrate its next tool call as
                                // markup INSIDE the thought; the loop recovers
                                // and executes that call, so the transcript
                                // shows the executed call instead of the raw
                                // narration.
                                lines.extend(scrub_narrated_toolcalls(thought_lines));
                            }
                        } else if tool == "multiedit" || tool == "multi_edit" {
                            if let Some(file_stats) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("file_stats"))
                                .and_then(|v| v.as_array())
                            {
                                let rel_paths: Vec<String> = file_stats
                                    .iter()
                                    .map(|fs| {
                                        fs.get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| make_relative_path(p, cwd))
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
                                    lines.push(StyledLine::from(vec![
                                        StyledSpan::styled(
                                            format!("  └ {}{} ", rel_path, padding),
                                            TextStyle::new().fg(ThemeColor::DarkGray),
                                        ),
                                        StyledSpan::styled(
                                            format!("+{}", added),
                                            TextStyle::new().fg(ThemeColor::Green),
                                        ),
                                        StyledSpan::styled(" ", TextStyle::new()),
                                        StyledSpan::styled(
                                            format!("-{}", removed),
                                            TextStyle::new().fg(ThemeColor::Red),
                                        ),
                                    ]));
                                }
                            } else if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, cwd)
                            {
                                // SEC-080: tool output is untrusted; neutralise
                                // any terminal escapes before rendering.
                                let result = sanitize_terminal(&result);
                                lines.push(StyledLine::from(StyledSpan::styled(
                                    format!("  └ {}", result),
                                    TextStyle::new().fg(ThemeColor::DarkGray),
                                )));
                            }
                        } else if tool == "agent_complete" {
                            // Render the full task-completion summary one output line
                            // per rendered line. Embedding a multi-line summary in a
                            // single Line does not work: the renderer strips '\n'
                            // graphemes inside a Span, so the summary would render
                            // as one continuous paragraph.
                            let summary = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("summary"))
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            // SEC-080: summary is tool output; neutralise escapes.
                            let summary = sanitize_terminal(summary);
                            let summary: &str = &summary;
                            if summary.is_empty() {
                                lines.push(StyledLine::from(StyledSpan::styled(
                                    "  \\- Task complete",
                                    TextStyle::new().fg(ThemeColor::Green),
                                )));
                            } else {
                                for line in summary.lines() {
                                    lines.push(StyledLine::from(StyledSpan::styled(
                                        format!("  \\- {line}"),
                                        TextStyle::new().fg(ThemeColor::Green),
                                    )));
                                }
                            }
                        } else if tool != "Edit"
                            && let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, cwd)
                        {
                            // SEC-080: tool output is untrusted; neutralise escapes.
                            let result = sanitize_terminal(&result);
                            lines.push(StyledLine::from(StyledSpan::styled(
                                format!("  └ {}", result),
                                TextStyle::new().fg(ThemeColor::DarkGray),
                            )));
                        }
                    }
                    if state.status == ToolCallStatus::Error {
                        if let Some(ref err) = state.error {
                            // SEC-080: tool error text is untrusted; neutralise.
                            let err = sanitize_terminal(err);
                            lines.push(StyledLine::from(StyledSpan::styled(
                                format!("  └ {}", err),
                                TextStyle::new().fg(ThemeColor::Red),
                            )));
                        }
                    }
                }
                MessagePart::Reasoning { text, .. } => {
                    // SEC-080: reasoning text is model output and may embed an
                    // escape sequence; neutralise before rendering.
                    let text = sanitize_terminal(text);
                    let mut reasoning_lines = Vec::new();
                    for line in text.as_ref().lines() {
                        reasoning_lines.push(StyledLine::from(StyledSpan::styled(
                            format!("  {}", line),
                            THINK_STYLE,
                        )));
                    }
                    // Reasoning text is also a place models narrate tool-call
                    // markup into; scrub it the same way as think outputs.
                    lines.extend(scrub_narrated_toolcalls(reasoning_lines));
                }
                MessagePart::Image(img) => {
                    let name = img
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("image");
                    lines.push(StyledLine::from(StyledSpan::styled(
                        format!("  {} [image: {}]", ICON_ATTACHMENT, name),
                        TextStyle::new().fg(ThemeColor::Yellow),
                    )));
                }
            }

            lines.push(StyledLine::default());
        }
    }

    lines
}

/// PERF-041: render any per-message cache group that is still behind its
/// message, then rebuild the flat plain-text copy buffer from the cache.
///
/// The render loop throttles re-rendering of an actively-streaming group
/// (PERF-042), so at the moment a copy is requested that group may still hold
/// the previous frame's rows.  Render the stale groups first so the copy
/// reads exactly what is on screen, then project the wrapped rows into
/// `message_content_lines`.
pub fn ensure_message_copy_lines(app: &mut App) {
    let w = app.session.message_cache_width.max(1) as usize;
    // Render only the groups whose `edit_seq` is behind (usually the single
    // streaming message), so a copy costs one group render at most.
    for i in 0..app.session.message_line_cache.len() {
        let stale = app
            .session
            .messages
            .get(i)
            .map(|msg| app.session.message_line_cache[i].edit_seq != msg.edit_seq)
            .unwrap_or(false);
        if !stale {
            continue;
        }
        let lines = message_to_lines(
            &app.session.messages[i],
            &app.tool_step_map,
            &app.sid_to_display_name,
            &app.cwd,
        );
        let edit_seq = app.session.messages[i].edit_seq;
        let group = &mut app.session.message_line_cache[i];
        group.lines = lines;
        group.edit_seq = edit_seq;
        group.wrapped_lines = group
            .lines
            .iter()
            .flat_map(|l| wrap_line_styled(l, w))
            .collect();
        group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
        group.wrapped_count = group.wrapped_lines.len() as u16;
    }
    app.session.message_cache_dirty_from = app.session.message_line_cache.len();
    let mut rebuilt = Vec::new();
    for group in &app.session.message_line_cache {
        rebuilt.extend(group.content_lines.iter().cloned());
    }
    app.session.message_content_lines = rebuilt;
    app.session.message_content_lines_dirty = false;
}
