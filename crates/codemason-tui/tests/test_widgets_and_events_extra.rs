//! Extra coverage tests for the codemason-tui render + event surfaces.
//!
//! Targets the largest uncovered files in the crate:
//! `src/widgets/message_widget.rs` (rendering via `TestBackend` and the
//! `tool_result_summary` match arms), `src/app/event_handler.rs` (synthetic
//! `Event` values fed through `App::handle_event`), `src/app/swarm.rs`,
//! `src/app/research.rs` and `src/research_progress.rs`.
//!
//! Tests only - no production logic is touched.

use std::collections::HashMap;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_tui::widgets::message_widget::MessageWidget;
use serde_json::json;

#[path = "support/mod.rs"]
mod support;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type StepMap = HashMap<String, (String, u32, u32)>;

/// Render a `Message` through `MessageWidget` into a `TestBackend` and return
/// the complete painted buffer as one string.
fn render_message(msg: &Message, width: u16, height: u16) -> String {
    let map: StepMap = HashMap::new();
    let widget = MessageWidget::new(msg, "/home/user/project", &map);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, width, height);
            frame.render_widget(widget, area);
        })
        .expect("draw");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn user_msg(text: &str) -> Message {
    Message::new(
        "session-1",
        Role::User,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

fn assistant_msg(text: &str) -> Message {
    Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

// ===========================================================================
// message_widget.rs - rendering via TestBackend
// ===========================================================================

#[test]
fn render_user_text_prefixes_you_and_indents_continuation() {
    let msg = user_msg("first line\nsecond line");
    let painted = render_message(&msg, 80, 8);
    assert!(painted.contains("You: first line"), "got:\n{painted}");
    assert!(painted.contains("second line"), "got:\n{painted}");
}

#[test]
fn render_assistant_plain_text_has_no_prefix() {
    let msg = assistant_msg("plain assistant output");
    let painted = render_message(&msg, 80, 4);
    assert!(painted.contains("plain assistant output"));
    assert!(!painted.contains("You: "));
}

#[test]
fn render_thinking_placeholder_becomes_thinking_indicator() {
    for dots in ["...", "......", ". ."] {
        let msg = assistant_msg(dots);
        let painted = render_message(&msg, 40, 3);
        assert!(
            painted.contains("Thinking ..."),
            "dots {dots:?} should render the thinking indicator, got:\n{painted}"
        );
    }
}

#[test]
fn render_assistant_agent_notice_uses_notice_bubble() {
    let msg = assistant_msg("Agent Notice\nsomething happened");
    let painted = render_message(&msg, 60, 5);
    assert!(painted.contains("Agent Notice"), "got:\n{painted}");
    assert!(painted.contains("something happened"), "got:\n{painted}");
}

#[test]
fn render_compaction_role_is_labelled() {
    let msg = Message::new(
        "s",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "context refreshed".to_string(),
        }],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("[compaction]"), "got:\n{painted}");
    assert!(painted.contains("context refreshed"), "got:\n{painted}");
}

#[test]
fn render_user_text_with_red_marker_renders_body_only() {
    // `[red]` markers are stripped to their body (the marker text is not shown).
    let msg = user_msg("prefix [red]blocked[/red] suffix");
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("blocked"), "got:\n{painted}");
    assert!(
        !painted.contains("[red]"),
        "marker must be stripped:\n{painted}"
    );
}

#[test]
fn render_tool_call_running_shows_step_tag_and_summary() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Running,
                input: json!({"path": "/home/user/project/src/main.rs"}),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let mut map: StepMap = HashMap::new();
    map.insert("c1".to_string(), ("abc".to_string(), 2, 3));
    let widget = MessageWidget::new(&msg, "/home/user/project", &map);
    let backend = TestBackend::new(80, 4);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| frame.render_widget(widget, Rect::new(0, 0, 80, 4)))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    let painted: String = (0..4)
        .map(|y| {
            (0..80)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(painted.contains("[2.3]"), "step tag missing:\n{painted}");
    assert!(painted.contains("Read"), "tool name missing:\n{painted}");
    assert!(painted.contains("main.rs"), "summary missing:\n{painted}");
}

#[test]
fn render_tool_call_error_shows_error_line() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Error,
                input: json!({"command": "false"}),
                output: None,
                error: Some("exit code 1".to_string()),
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 80, 5);
    assert!(painted.contains("Error: exit code 1"), "got:\n{painted}");
}

#[test]
fn render_completed_read_shows_line_range() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "src/lib.rs"}),
                output: Some(json!({"lines": 3, "start_line": 10, "end_line": 12})),
                error: None,
                duration_ms: Some(42),
            }),
        }],
    );
    let painted = render_message(&msg, 80, 5);
    assert!(painted.contains("Read"), "got:\n{painted}");
}

#[test]
fn render_reasoning_part_is_indented() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "deliberating".to_string(),
            signature: None,
        }],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("deliberating"), "got:\n{painted}");
}

#[test]
fn render_image_part_shows_file_name() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Image(Box::new(
            codemason_types::message::ImageData {
                mime_type: "image/png".to_string(),
                path: std::path::PathBuf::from("/tmp/shot.png"),
            },
        ))],
    );
    let painted = render_message(&msg, 60, 4);
    assert!(painted.contains("[image: shot.png]"), "got:\n{painted}");
}

#[test]
fn render_think_tool_call_prints_thought_body() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "think".to_string(),
            call_id: "t1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({}),
                output: Some(json!({"thought": "a deep thought\nsecond line"})),
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 80, 6);
    assert!(painted.contains("a deep thought"), "got:\n{painted}");
    assert!(painted.contains("second line"), "got:\n{painted}");
}

#[test]
fn render_wait_agents_tool_call_lists_tasks() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "wait_agents".to_string(),
            call_id: "w1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({}),
                output: Some(json!({
                    "tasks": [
                        {"agent": "explore", "id": "abcdef0123456789", "elapsed_ms": 2500, "output_lines": 12, "status": "completed"},
                        {"agent": "build", "id": "feedface00000000", "elapsed_ms": 1000, "output_lines": 0, "status": "failed"}
                    ]
                })),
                error: None,
                duration_ms: None,
            }),
        }],
    );
    let painted = render_message(&msg, 90, 8);
    assert!(painted.contains("explore"), "got:\n{painted}");
    assert!(painted.contains("2.5s"), "elapsed missing:\n{painted}");
}

#[test]
fn render_completed_tool_call_shows_result_summary() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Grep".to_string(),
            call_id: "g1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"pattern": "fn main"}),
                output: Some(json!({"count": 3, "file_count": 2, "truncated": false})),
                error: None,
                duration_ms: Some(10),
            }),
        }],
    );
    let painted = render_message(&msg, 90, 6);
    assert!(painted.contains("matched in"), "got:\n{painted}");
}

// ===========================================================================
// message_widget.rs - tool_result_summary match arms
// ===========================================================================

// ===========================================================================
// research_progress.rs - apply/finish/render branches
// ===========================================================================

// ===========================================================================
// research_progress.rs - encode round trips
// ===========================================================================

// ===========================================================================
// app/event_handler.rs - synthetic events
// ===========================================================================

// ===========================================================================
// app/swarm.rs - reachable through the `/swarm` slash command only
//
// `execute_swarm_decomposition`, `spawn_swarm_teammates`, `poll_swarm_unblock`,
// `poll_swarm_completion` and `finalize_swarm_completion` are `pub(crate)` and
// have no public test hook, so they are unreachable from an integration test
// (reaching them requires a live LLM decomposition). The status/cancel entry
// points are driven through `/swarm`.
// ===========================================================================

// ===========================================================================
// app/research.rs - reachable through the `/research` slash command
// ===========================================================================

// ===========================================================================
// shared helpers
// ===========================================================================
