//! Additional coverage for `codemason_tui::widgets::message_widget`.
//!
//! `test_widgets_and_events_extra.rs` covers the widget rendering arms and a
//! subset of the `tool_input_summary` / `tool_result_summary` match arms. This
//! file fills the remaining summary arms (git / gitlab / github / lsp / codeindex
//! / spec / team / cron / memory) and the message-widget render branches those
//! tests leave untouched (multiedit per-file stats, agent_complete summary,
//! wait_agents task table, error without detail, red-marker continuation lines,
//! thinking-placeholder edges, and the `Widget` impl).
//!
//! Tests only - no production logic is touched.

use std::collections::HashMap;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_tui::widgets::message_widget::{MessageWidget, tool_result_summary};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render a `Message` through `MessageWidget` and return the painted text.
fn render_message(msg: &Message, width: u16, height: u16) -> String {
    let map: HashMap<String, (String, u32, u32)> = HashMap::new();
    let widget = MessageWidget::new(msg, "/home/user/project", &map);
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| frame.render_widget(widget, Rect::new(0, 0, width, height)))
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

/// Flatten a `MessageWidget`'s lines into one string.
fn lines_text(msg: &Message) -> String {
    let map: HashMap<String, (String, u32, u32)> = HashMap::new();
    let widget = MessageWidget::new(msg, "/home/user/project", &map);
    widget
        .to_lines()
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn tool_call(tool: &str, call_id: &str, state: ToolCallState) -> Message {
    Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: tool.to_string(),
            call_id: call_id.to_string(),
            state: Box::new(state),
        }],
    )
}

fn completed(tool: &str, input: serde_json::Value, output: serde_json::Value) -> Message {
    tool_call(
        tool,
        "call-1",
        ToolCallState {
            status: ToolCallStatus::Completed,
            input,
            output: Some(output),
            error: None,
            duration_ms: None,
        },
    )
}

fn result_summary(tool: &str, out: serde_json::Value, input: serde_json::Value) -> String {
    tool_result_summary(tool, &Some(out), &input, "/home/user/project")
        .unwrap_or_else(|| format!("<none:{tool}>"))
}

// ===========================================================================
// tool_result_summary arms not covered by test_widgets_and_events_extra
// ===========================================================================

// ===========================================================================
// tool_input_summary arms not covered elsewhere
// ===========================================================================

#[test]
fn input_summary_git_arms() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("git_add", &json!({"all": true}), cwd),
        "add -A"
    );
    assert_eq!(
        tool_input_summary("git_add", &json!({"update": true}), cwd),
        "add -u"
    );
    assert_eq!(
        tool_input_summary("git_branch", &json!({"all": false}), cwd),
        "branch list (local)"
    );
    assert_eq!(
        tool_input_summary("git_checkout", &json!({"branch": "main"}), cwd),
        "checkout main"
    );
    assert_eq!(
        tool_input_summary(
            "git_clone",
            &json!({"url": "https://example.org/x.git"}),
            cwd
        ),
        "clone https://example.org/x.git"
    );
    assert_eq!(
        tool_input_summary("git_commit", &json!({"message": "fix"}), cwd),
        "commit: fix"
    );
    assert_eq!(
        tool_input_summary("git_diff", &json!({"target": "staged"}), cwd),
        "diff staged"
    );
    assert_eq!(
        tool_input_summary("git_push", &json!({"remote": "origin"}), cwd),
        "push origin"
    );
    assert_eq!(
        tool_input_summary("git_status", &json!({"short": true}), cwd),
        "status --short"
    );
    assert_eq!(
        tool_input_summary("git_show", &json!({"ref": "HEAD"}), cwd),
        "show HEAD"
    );
    assert_eq!(
        tool_input_summary("git_stash", &json!({"action": "pop"}), cwd),
        "stash pop"
    );
    assert_eq!(tool_input_summary("git_tag", &json!({}), cwd), "tag -l");
}

#[test]
fn input_summary_gitlab_arms() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("gitlab_list_issues", &json!({"state": "opened"}), cwd),
        "issues (opened)"
    );
    assert_eq!(
        tool_input_summary("gitlab_get_mr", &json!({"iid": 7}), cwd),
        "MR #7"
    );
    assert_eq!(
        tool_input_summary("gitlab_create_mr", &json!({"title": "Feat"}), cwd),
        "new MR: Feat"
    );
    assert_eq!(
        tool_input_summary("gitlab_merge_mr", &json!({"iid": 8}), cwd),
        "merge !8"
    );
    assert_eq!(
        tool_input_summary("gitlab_get_job", &json!({"job_id": 9}), cwd),
        "job #9"
    );
    assert_eq!(
        tool_input_summary("gitlab_cancel_pipeline", &json!({"pipeline_id": 5}), cwd),
        "cancel pipeline #5"
    );
}

#[test]
fn input_summary_github_arms() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary("github_list_issues", &json!({"state": "open"}), cwd),
        "issues (open)"
    );
    assert_eq!(
        tool_input_summary("github_get_issue", &json!({"number": 5}), cwd),
        "issue #5"
    );
    assert_eq!(
        tool_input_summary("github_create_issue", &json!({"title": "Bug"}), cwd),
        "new issue: Bug"
    );
    assert_eq!(
        tool_input_summary("github_create_pr", &json!({"title": "Feat"}), cwd),
        "new PR: Feat"
    );
    assert_eq!(
        tool_input_summary("github_merge_pr", &json!({"number": 9}), cwd),
        "merge #9"
    );
    assert_eq!(
        tool_input_summary("github_review_pr", &json!({"number": 9}), cwd),
        "review #9"
    );
}

#[test]
fn input_summary_lsp_codeindex_spec_team_arms() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(
        tool_input_summary(
            "lsp_definition",
            &json!({"path": "src/lib.rs", "line": 3, "column": 5}),
            cwd
        ),
        "src/lib.rs L3:5"
    );
    assert_eq!(
        tool_input_summary("lsp_symbols", &json!({"path": "src/lib.rs"}), cwd),
        "src/lib.rs"
    );
    assert_eq!(
        tool_input_summary("lsp_diagnostics", &json!({}), cwd),
        "analyze"
    );
    assert_eq!(
        tool_input_summary(
            "codeindex_search",
            &json!({"query": "foo", "kind": "function", "language": "rust"}),
            cwd
        ),
        "\"foo\" kind:function lang:rust"
    );
    assert_eq!(
        tool_input_summary("codeindex_status", &json!({"enabled": false}), cwd),
        "status: off"
    );
    assert_eq!(
        tool_input_summary("codeindex_reindex", &json!({}), cwd),
        "reindex"
    );
    assert_eq!(
        tool_input_summary("codeindex_godnodes", &json!({"n": 5}), cwd),
        "god nodes (top 5)"
    );
    assert_eq!(
        tool_input_summary("codeindex_communities", &json!({}), cwd),
        "communities"
    );
    assert_eq!(
        tool_input_summary("spec_list", &json!({"status": "draft"}), cwd),
        "list (draft)"
    );
    assert_eq!(
        tool_input_summary("spec_read", &json!({"spec_id": "auth"}), cwd),
        "read auth"
    );
    assert_eq!(
        tool_input_summary("spec_search", &json!({"query": "login"}), cwd),
        "search \"login\""
    );
    assert_eq!(
        tool_input_summary("spec_coverage", &json!({"spec_id": "auth"}), cwd),
        "coverage auth"
    );
    assert_eq!(
        tool_input_summary("team_create", &json!({"name": "alpha"}), cwd),
        "create alpha"
    );
    assert_eq!(tool_input_summary("team_status", &json!({}), cwd), "status");
    assert_eq!(
        tool_input_summary("team_task_list", &json!({}), cwd),
        "list tasks"
    );
    assert_eq!(
        tool_input_summary("team_read_messages", &json!({}), cwd),
        "read messages"
    );
    assert_eq!(
        tool_input_summary("team_submit_plan", &json!({}), cwd),
        "submit plan"
    );
    assert_eq!(
        tool_input_summary("cancel_agent", &json!({"task_id": "abcdef0123456789"}), cwd),
        "cancel abcdef01"
    );
    assert_eq!(
        tool_input_summary("list_agents", &json!({"status": "running"}), cwd),
        "filter: running"
    );
}

#[test]
fn input_summary_misc_arms() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    assert_eq!(tool_input_summary("metadata", &json!({}), cwd), "metadata");
    assert_eq!(tool_input_summary("truncate", &json!({}), cwd), "truncate");
    assert_eq!(
        tool_input_summary("read_line_range", &json!({"start_line": 1}), cwd),
        "line range"
    );
    assert_eq!(
        tool_input_summary(
            "pdf_write",
            &json!({"path": "/home/user/project/out.pdf"}),
            cwd
        ),
        "out.pdf"
    );
    assert_eq!(
        tool_input_summary("memory_recall", &json!({"query": "fact"}), cwd),
        "recall \"fact\""
    );
    assert_eq!(
        tool_input_summary("memory_forget", &json!({"id": "3"}), cwd),
        "forget 3"
    );
    assert_eq!(
        tool_input_summary(
            "cron_add",
            &json!({"id": "nightly", "schedule": "every 1h"}),
            cwd
        ),
        "add `nightly` every 1h"
    );
    assert_eq!(
        tool_input_summary("cron_remove", &json!({"id": "nightly"}), cwd),
        "remove `nightly`"
    );
    assert_eq!(
        tool_input_summary("cron_list", &json!({}), cwd),
        "list events"
    );
    assert_eq!(
        tool_input_summary("cron_enable", &json!({"id": "nightly"}), cwd),
        "enable `nightly`"
    );
    assert_eq!(
        tool_input_summary("cron_disable", &json!({"id": "nightly"}), cwd),
        "disable `nightly`"
    );
    assert_eq!(
        tool_input_summary("model_info", &json!({"format": "json"}), cwd),
        "model info (json)"
    );
}

#[test]
fn input_summary_codeindex_dependencies_and_explain() {
    use codemason_tui::widgets::message_widget::tool_input_summary;
    let cwd = "/home/user/project";
    let s = tool_input_summary(
        "codeindex_dependencies",
        &json!({"path": "src/lib.rs", "direction": "dependents"}),
        cwd,
    );
    assert_eq!(s, "dependents: src/lib.rs");
    let s = tool_input_summary("codeindex_explain", &json!({"symbol": "Foo"}), cwd);
    assert_eq!(s, "explain Foo");
    let s = tool_input_summary("codeindex_path", &json!({"from": "A", "to": "B"}), cwd);
    assert_eq!(s, "path A → B");
    let s = tool_input_summary("codeindex_references", &json!({"symbol": "Foo"}), cwd);
    assert_eq!(s, "refs: Foo");
}

// ===========================================================================
// MessageWidget render arms not covered elsewhere
// ===========================================================================

#[test]
fn render_multiedit_per_file_stats_table() {
    let msg = completed(
        "multiedit",
        json!({"edits": []}),
        json!({
            "edits": 2,
            "file_count": 2,
            "file_stats": [
                {"path": "/home/user/project/src/alpha.rs", "added": 5, "removed": 2},
                {"path": "/home/user/project/src/beta.rs", "added": 1, "removed": 0}
            ]
        }),
    );
    let text = lines_text(&msg);
    assert!(text.contains("alpha.rs"), "got:\n{text}");
    assert!(text.contains("beta.rs"), "got:\n{text}");
    assert!(text.contains("+5"), "got:\n{text}");
    assert!(text.contains("-2"), "got:\n{text}");
}

#[test]
fn render_multiedit_falls_back_to_summary_without_file_stats() {
    let msg = completed(
        "multiedit",
        json!({"edits": []}),
        json!({"edits": 3, "file_count": 2}),
    );
    let text = lines_text(&msg);
    assert!(text.contains("3 edits across 2 files"), "got:\n{text}");
}

#[test]
fn render_agent_complete_summary_block() {
    let msg = completed(
        "agent_complete",
        json!({}),
        json!({"summary": "line one\nline two"}),
    );
    let text = lines_text(&msg);
    assert!(text.contains("line one"), "got:\n{text}");
    assert!(text.contains("line two"), "got:\n{text}");

    // The result-summary fallback reports the completion label.
    assert_eq!(
        result_summary("agent_complete", json!({}), json!({})),
        "Task complete"
    );
}

#[test]
fn render_wait_agents_task_table_with_statuses() {
    let msg = completed(
        "wait_agents",
        json!({"task_ids": ["a", "b"]}),
        json!({
            "count": 2,
            "tasks": [
                {"agent": "explore", "id": "abcdef0123456789", "elapsed_ms": 1500, "output_lines": 12, "status": "completed"},
                {"agent": "build", "id": "deadbeefdeadbeef", "elapsed_ms": 800, "output_lines": 4, "status": "failed"}
            ]
        }),
    );
    let text = lines_text(&msg);
    assert!(text.contains("explore"), "got:\n{text}");
    assert!(text.contains("build"), "got:\n{text}");
    assert!(text.contains("line(s)"), "got:\n{text}");
}

#[test]
fn render_tool_error_without_detail_uses_default_message() {
    let msg = tool_call(
        "Bash",
        "call-err",
        ToolCallState {
            status: ToolCallStatus::Error,
            input: json!({"command": "false"}),
            output: None,
            error: None,
            duration_ms: None,
        },
    );
    let text = lines_text(&msg);
    assert!(text.contains("Tool execution failed"), "got:\n{text}");
}

#[test]
fn render_tool_error_with_detail() {
    let msg = tool_call(
        "Bash",
        "call-err2",
        ToolCallState {
            status: ToolCallStatus::Error,
            input: json!({"command": "false"}),
            output: None,
            error: Some("exit status 1".to_string()),
            duration_ms: None,
        },
    );
    let text = lines_text(&msg);
    assert!(text.contains("exit status 1"), "got:\n{text}");
}

#[test]
fn render_red_marker_first_and_continuation_lines() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "[red]disabled tool[/red]\nplain continuation\n[red]second[/red]".to_string(),
        }],
    );
    let text = lines_text(&msg);
    assert!(text.contains("disabled tool"), "got:\n{text}");
    assert!(text.contains("plain continuation"), "got:\n{text}");
}

#[test]
fn render_user_red_marker_line() {
    // A `[red]` marker on a User message exercises the `i == 0` blank-line +
    // dot-prefix branch inside `parse_red_marker` handling.
    let msg = Message::new(
        "s",
        Role::User,
        vec![MessagePart::Text {
            text: "first\n[red]marked[/red]".to_string(),
        }],
    );
    let text = lines_text(&msg);
    assert!(text.contains("marked"), "got:\n{text}");
}

#[test]
fn render_thinking_placeholder_edges() {
    // Exactly six dots renders the thinking indicator; seven does not.
    let six = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "......".to_string(),
        }],
    );
    assert!(lines_text(&six).contains("Thinking ..."));

    let seven = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: ".......".to_string(),
        }],
    );
    assert!(!lines_text(&seven).contains("Thinking ..."));

    // A user-authored "..." is not a thinking placeholder.
    let user = Message::new(
        "s",
        Role::User,
        vec![MessagePart::Text {
            text: "...".to_string(),
        }],
    );
    assert!(!lines_text(&user).contains("Thinking ..."));
}

#[test]
fn render_empty_text_part_is_processed() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: String::new(),
        }],
    );
    let _ = lines_text(&msg);
}

#[test]
fn render_pending_tool_call_status() {
    let msg = tool_call(
        "read",
        "call-pending",
        ToolCallState {
            status: ToolCallStatus::Pending,
            input: json!({"path": "/home/user/project/src/main.rs"}),
            output: None,
            error: None,
            duration_ms: None,
        },
    );
    let text = lines_text(&msg);
    assert!(text.contains("Read"), "got:\n{text}");
}

#[test]
fn render_compaction_role_text() {
    let msg = Message::new(
        "s",
        Role::Compaction,
        vec![MessagePart::Text {
            text: "## Objective\nsummary body".to_string(),
        }],
    );
    let text = lines_text(&msg);
    assert!(text.contains("[compaction]"), "got:\n{text}");
    assert!(text.contains("summary body"), "got:\n{text}");
}

#[test]
fn render_multiline_assistant_indents_continuation() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "line one\nline two\nline three".to_string(),
        }],
    );
    let text = lines_text(&msg);
    assert!(text.contains("line one"));
    assert!(text.contains("line two"));
    assert!(text.contains("line three"));
}

#[test]
fn render_through_test_backend_wraps_long_content() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "a very long assistant output line that must wrap across several narrow rows"
                .to_string(),
        }],
    );
    let painted = render_message(&msg, 20, 8);
    assert!(painted.contains("assistant"), "got:\n{painted}");
}

#[test]
fn render_reasoning_multiline() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Reasoning {
            text: "reason one\nreason two".to_string(),
            signature: None,
        }],
    );
    let text = lines_text(&msg);
    assert!(text.contains("reason one"));
    assert!(text.contains("reason two"));
}

#[test]
fn render_read_tool_line_range_and_inline_diff() {
    let msg = completed(
        "read",
        json!({"path": "/home/user/project/src/main.rs"}),
        json!({"lines": 3, "start_line": 1, "end_line": 40}),
    );
    let text = lines_text(&msg);
    assert!(text.contains("lines 1-40"), "got:\n{text}");
}

#[test]
fn render_edit_tool_inline_diff_stats() {
    let msg = completed(
        "Edit",
        json!({"file_path": "/home/user/project/src/main.rs"}),
        json!({"path": "/home/user/project/src/main.rs", "old_lines": 4, "new_lines": 9}),
    );
    let text = lines_text(&msg);
    assert!(text.contains("+9"), "got:\n{text}");
    assert!(text.contains("-4"), "got:\n{text}");
}

#[test]
fn render_think_tool_falls_back_to_input_thought() {
    // to_lines renders the thought from the output; an output-less think call
    // still renders its tool name.
    let msg = completed("think", json!({"thought": "input thought"}), json!({}));
    let text = lines_text(&msg);
    assert!(text.contains("Think"), "got:\n{text}");

    // The tool_result_summary fallback reads the thought from the *input*.
    assert_eq!(
        result_summary("think", json!({}), json!({"thought": "input thought"})),
        "input thought"
    );
}

#[test]
fn render_wait_agents_without_tasks_array_uses_summary() {
    let msg = completed(
        "wait_agents",
        json!({"task_ids": []}),
        json!({"count": 0, "success": true}),
    );
    let text = lines_text(&msg);
    assert!(
        text.contains("0 tasks") || text.contains("wait"),
        "got:\n{text}"
    );
}

#[test]
fn render_image_without_file_name() {
    let msg = Message::new(
        "s",
        Role::Assistant,
        vec![MessagePart::Image(Box::new(
            codemason_types::message::ImageData {
                mime_type: "image/png".to_string(),
                path: std::path::PathBuf::from("/"),
            },
        ))],
    );
    let text = lines_text(&msg);
    assert!(text.contains("[image:"), "got:\n{text}");
}
