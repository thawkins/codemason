//! Tests for `test_text_selection.rs`

#[path = "support/mod.rs"]
mod support;

use ratatui::{Terminal, backend::TestBackend};

use codemason_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use codemason_agent::team::{TeamConfig, TeamMember};
use codemason_tui::App;
use codemason_tui::app::{OutputViewState, OutputViewTarget, ScreenMode};

/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

// ---------- Selection creation ----------

// ---------- Selection dragging ----------

/// Render one frame so the renderer records the window hit areas and the
/// painted-row click map (spec `agentui` A3: rows resolve through the
/// render-recorded map, not hand-set coordinates).
fn render(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, app))
        .expect("render frame");
}

#[test]
fn test_opening_active_agents_row_output_view_from_the_keyboard() {
    // The row-mouse model is retired: the sub-agent overlay opens through the
    // cursor (Down + Enter), the same destination chain the old row click
    // drove (FR-034).
    let mut app = make_app();
    app.session.session_id = Some("lead-s1".to_string());
    app.show_agents_window = true;
    app.active_tasks.push(codemason_agent::task::TaskEntry {
        id: "task-12345678".to_string(),
        parent_session_id: "lead-s1".to_string(),
        child_session_id: "child-s1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        detached: false,
        status: codemason_agent::task::TaskStatus::Running,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: codemason_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    });
    render(&mut app);
    app.move_agents_cursor(1);
    app.agents_cursor_open_output();
    assert!(app.output_view.is_some());
    assert_eq!(
        app.session.selected_agent_session_id.as_deref(),
        Some("child-s1")
    );
}

#[test]
fn test_opening_teams_row_output_view_from_the_keyboard() {
    // The teammate overlay opens through the cursor (Down + Enter), the same
    // destination chain the old row click drove (FR-038).
    let mut app = make_app();
    app.session.session_id = Some("lead-s1".to_string());
    app.show_teams_window = true;
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-s1"));
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("tm-s1".to_string());
    app.session.team_members.push(member);

    render(&mut app);
    app.move_teams_cursor(1);
    app.teams_cursor_focus();
    assert_eq!(app.session.focused_teammate.as_deref(), Some("tm-001"));
    let view = app.output_view.as_ref().expect("the overlay must open");
    match &view.target {
        codemason_tui::app::OutputViewTarget::TeamMember { session_id, .. } => {
            assert_eq!(session_id.as_deref(), Some("tm-s1"));
        }
        other => panic!("expected a TeamMember target, got {other:?}"),
    }
}

#[test]
fn test_output_view_overlay_renders_non_current_session_message() {
    let mut app = make_app();
    app.current_screen = ScreenMode::Chat;
    app.session.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&codemason_agent::message::Message::user_text(
            "child-s1",
            "hello from child",
        ))
        .expect("create child message");
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "child-s1".to_string(),
            label: "explore [abcd1234]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: codemason_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let mut all_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            all_text.push_str(cell.symbol());
        }
        all_text.push('\n');
    }

    assert!(
        all_text.contains("hello from child"),
        "output overlay should render child-session message, got:\n{all_text}"
    );
}

#[test]
fn test_output_view_overlay_renders_tool_calls_for_non_current_session() {
    let mut app = make_app();
    app.current_screen = ScreenMode::Chat;
    app.session.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("tm-s1", "/tmp")
        .expect("create teammate session");

    let tool_msg = Message::new(
        "tm-s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "Bash".to_string(),
            call_id: "c1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: serde_json::json!({"command":"echo hi"}),
                output: Some(serde_json::json!({"line_count": 2})),
                error: None,
                duration_ms: Some(12),
            }),
        }],
    );
    app.storage
        .create_message(&tool_msg)
        .expect("create tool-call message");
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "tm-s1".to_string(),
            label: "writer [abcd1234]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: codemason_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let mut all_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            all_text.push_str(cell.symbol());
        }
        all_text.push('\n');
    }

    assert!(
        all_text.contains("Bash"),
        "output overlay should include tool name, got:\n{all_text}"
    );
    assert!(
        all_text.contains("echo hi"),
        "output overlay should include tool input summary, got:\n{all_text}"
    );
    // The tool call should be annotated with a [step.substep] prefix derived
    // from the message transcript itself, matching the primary chat window.
    assert!(
        all_text.contains("[1.1]"),
        "output overlay should include [1.1] step indicator for tool call, got:\n{all_text}"
    );
}

#[test]
fn test_output_view_overlay_does_not_render_log_lines() {
    // The overlay must show the sub-agent transcript in the same chat format
    // as the Messages window; raw log-panel entries (timestamps, TUL badges)
    // must not leak in.
    let mut app = make_app();
    app.current_screen = ScreenMode::Chat;
    app.session.session_id = Some("lead-s1".to_string());
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&codemason_agent::message::Message::assistant_text(
            "child-s1",
            "child transcript text",
        ))
        .expect("create child message");

    // Log entries that previously leaked into the overlay: the task-start
    // marker and tool-call mirror lines, both stamped to the child session.
    app.log_entries.push(codemason_tui::app::LogEntry {
        timestamp: chrono::Utc::now(),
        level: codemason_tui::app::LogLevel::Info,
        message: "[bg] Background task started: explore-a1b2c3d4 (explore)".to_string(),
        session_id: Some("child-s1".to_string()),
        agent_id: None,
        seq: 1,
    });
    app.log_entries.push(codemason_tui::app::LogEntry {
        timestamp: chrono::Utc::now(),
        level: codemason_tui::app::LogLevel::Tool,
        message: "[explore-a1b2c3d4] [1.1] tool call: grep".to_string(),
        session_id: Some("child-s1".to_string()),
        agent_id: Some("explore-a1b2c3d4".to_string()),
        seq: 2,
    });

    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "child-s1".to_string(),
            label: "explore [a1b2c3d4]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: codemason_tui::app::OutputViewLineCache {
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render frame");

    // Extract just the overlay region text.
    let area = app.output_view_area;
    let buffer = terminal.backend().buffer().clone();
    let mut overlay_text = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buffer.cell((x, y)) {
                overlay_text.push_str(cell.symbol());
            }
        }
        overlay_text.push('\n');
    }

    let lines: Vec<&str> = overlay_text.lines().collect();
    // The chat-format transcript line must be present ("● " prefix).
    assert!(
        lines.iter().any(|l| l.contains("child transcript text")),
        "overlay should render the child transcript, got:\n{overlay_text}"
    );
    // No log-panel-style lines: timestamped level badges must not appear.
    for style in ["INF ", "TUL ", "WRN ", "ERR "] {
        assert!(
            !lines.iter().any(|l| l.contains(style)),
            "overlay must not contain log-panel badge {style:?}, got:\n{overlay_text}"
        );
    }
    assert!(
        !overlay_text.contains("tool call: grep"),
        "overlay must not mirror log-window tool lines, got:\n{overlay_text}"
    );
    assert!(
        !overlay_text.contains("Background task started"),
        "overlay must not mirror log-window task-start lines, got:\n{overlay_text}"
    );
}

// ---------- Selection normalization ----------

// ---------- Right-click context menu ----------

// ---------- extract_text_from_lines ----------

// ---------- Scrollbar click does NOT start selection ----------

// ---------- New click replaces old selection ----------

// ---------- Regression: Alt+I dismisses log panel, stale selection panics ----------
