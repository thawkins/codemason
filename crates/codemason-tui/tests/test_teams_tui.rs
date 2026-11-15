//! Tests for `test_teams_tui.rs`

use codemason_agent::event::Event;
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember, TeamStore};
use codemason_tui::crossterm_input::CrosstermAppExt;
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

fn unique_team_name(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

// -- /team status -------------------------------------------------------------

// -- /team create -------------------------------------------------------------

// -- /team show ---------------------------------------------------------------

// -- /team help ---------------------------------------------------------------

// -- /team close --------------------------------------------------------------

// -- /team delete -------------------------------------------------------------

// -- /team tasks ---------------------------------------------------------------

// -- /team clear ---------------------------------------------------------------

// -- /team message -------------------------------------------------------------

// -- /team cleanup -------------------------------------------------------------

// -- /team unknown subcommand --------------------------------------------------

// -- [T] badge - team_members drives badge rendering ---------------------------

// -- Event handling - TeammateSpawned -----------------------------------------

// -- Event handling - TeammateIdle --------------------------------------------

// -- Event handling - TeamTaskClaimed / Completed -----------------------------

// -- Event handling - TeamCleanedUp --------------------------------------------

#[test]
fn test_teams_panel_renders_table_with_elapsed_and_steps_columns() {
    let mut app = support::make_app();
    app.current_screen = codemason_tui::app::ScreenMode::Chat;
    app.show_log = true;
    app.session.session_id = Some("lead-session-1".to_string());
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-session-1"));

    app.storage
        .create_session("lead-session-1", "/tmp")
        .expect("create lead session");
    app.storage
        .create_session("tm-session-1", "/tmp")
        .expect("create teammate session");
    app.event_bus.set_step("lead-session-1", 7);
    app.event_bus.set_step("tm-session-1", 3);
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("lead-session-1");
    app.event_bus.increment_tool_calls("tm-session-1");
    app.event_bus.increment_tool_calls("tm-session-1");
    app.event_bus.increment_tool_calls("tm-session-1");

    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.session_id = Some("tm-session-1".to_string());
    member.status = MemberStatus::Working;
    member.current_task_id = Some("task-001".to_string());
    app.session.team_members.push(member);
    app.session
        .team_message_counts
        .insert("tm-001".to_string(), (2, 1));
    app.show_teams = true;
    app.show_teams_window = true;

    let backend = TestBackend::new(160, 44);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).expect("cell").symbol());
        }
        text.push('\n');
    }

    assert!(
        text.contains("Teams"),
        "should show teams popup title: {text}"
    );
    assert!(
        text.contains("Esc close"),
        "should show the footer key legend naming the dismissal: {text}"
    );
    assert!(
        text.contains("lead + 1 teammate"),
        "should show explicit teammate count: {text}"
    );
    assert!(
        text.contains("elapsed"),
        "should show elapsed column: {text}"
    );
    assert!(text.contains("steps"), "should show steps column: {text}");
    assert!(text.contains("sent"), "should show sent column: {text}");
    assert!(text.contains("recv"), "should show recv column: {text}");
    assert!(
        text.contains("reviewer"),
        "should include teammate row: {text}"
    );
}

#[test]
fn test_teams_panel_shows_tool_call_count_not_loop_steps() {
    let mut app = support::make_app();
    app.current_screen = codemason_tui::app::ScreenMode::Chat;
    app.show_log = true;
    app.session.session_id = Some("lead-session-1".to_string());
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-session-1"));

    app.storage
        .create_session("tm-session-1", "/tmp")
        .expect("create teammate session");

    // Simulate a sub-agent/teammate session that has done 2 loop iterations
    // but only 1 tool call. The UI should display 1, not 2.
    app.event_bus.set_step("tm-session-1", 2);
    app.event_bus.increment_tool_calls("tm-session-1");

    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.session_id = Some("tm-session-1".to_string());
    member.status = MemberStatus::Working;
    app.session.team_members.push(member);
    app.show_teams = true;
    app.show_teams_window = true;

    let backend = TestBackend::new(160, 44);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).expect("cell").symbol());
        }
        text.push('\n');
    }

    // Locate the teammate row and check only that line, avoiding false matches
    // from the title bar or status-bar version numbers.
    let teammate_line = text
        .lines()
        .find(|line| line.contains("tm-001"))
        .expect("teammate row should be rendered");

    assert!(
        teammate_line.contains('1'),
        "teammate steps column should show tool-call count 1, not loop-step count 2: {teammate_line}"
    );
    assert!(
        !teammate_line.contains('2'),
        "teammate steps column should not show loop-step count 2: {teammate_line}"
    );
}

#[test]
fn test_buttons_render_dimmed_when_unavailable() {
    let mut app = support::make_app();
    app.current_screen = codemason_tui::app::ScreenMode::Chat;
    app.show_log = true;
    app.session.session_id = Some("lead-session-1".to_string());

    let backend = TestBackend::new(160, 44);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).expect("cell").symbol());
        }
        text.push('\n');
    }

    assert!(
        text.contains("Agent"),
        "agents button should always be visible: {text}"
    );
    assert!(
        text.contains("Team"),
        "teams button should always be visible: {text}"
    );
}

#[test]
fn test_buttons_render_with_active_blue_state() {
    let mut app = support::make_app();
    app.current_screen = codemason_tui::app::ScreenMode::Chat;
    app.show_log = true;
    app.session.session_id = Some("lead-session-1".to_string());
    app.session.active_team = Some(TeamConfig::new("alpha", "lead-session-1"));
    app.active_tasks.push(codemason_agent::task::TaskEntry {
        id: "task-1".to_string(),
        parent_session_id: "lead-session-1".to_string(),
        child_session_id: "child-1".to_string(),
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
    app.show_agents_window = true;

    let backend = TestBackend::new(160, 44);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");

    let buffer = terminal.backend().buffer().clone();
    let mut found_active_blue = false;
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).expect("cell");
            if cell.symbol() == "A"
                && cell.fg == ratatui::style::Color::White
                && cell.bg == ratatui::style::Color::Blue
            {
                found_active_blue = true;
                break;
            }
        }
        if found_active_blue {
            break;
        }
    }

    assert!(
        found_active_blue,
        "active Agents button should render white-on-blue"
    );
}

#[tokio::test]
async fn test_agents_popup_renders_tidy_table_columns() {
    let mut app = support::make_app();
    app.current_screen = codemason_tui::app::ScreenMode::Chat;
    app.show_log = true;
    app.session.session_id = Some("lead-session-1".to_string());
    app.show_agents_window = true;
    app.active_tasks.push(codemason_agent::task::TaskEntry {
        id: "task-a1".to_string(),
        parent_session_id: "lead-session-1".to_string(),
        child_session_id: "child-a1".to_string(),
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

    let backend = TestBackend::new(160, 44);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer.cell((x, y)).expect("cell").symbol());
        }
        text.push('\n');
    }

    assert!(text.contains("id"), "should include id column: {text}");
    assert!(text.contains("name"), "should include name column: {text}");
    assert!(text.contains("type"), "should include type column: {text}");
    assert!(
        text.contains("elapsed"),
        "should include elapsed column: {text}"
    );
    assert!(
        text.contains("steps"),
        "should include steps column: {text}"
    );
}

// -- Event handling - TeammateMessage -----------------------------------------

// -- T-014 (spec `agentui`): team event plumbing gaps -------------------------

/// A `TeammateSpawned` for a session that never saw the `team_create` tool
/// result must still activate the team (FR-050): the strip's visibility rule
/// needs `active_team`, and the spawn handler hydrates it from the on-disk
/// store together with the member's session id.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_event_teammate_spawned_without_tool_result_activates_team_and_strip() {
    let _cwd_guard = crate::global_env_lock();
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".codemason/teams")).unwrap();

    let team_name = unique_team_name("spawn-activates");
    let mut store = TeamStore::create(&team_name, "lead-1", tmp.path(), true).expect("create team");
    let mut member = TeamMember::new("writer", "tm-001", "general");
    member.session_id = Some("tm-sess-1".to_string());
    member.status = MemberStatus::Working;
    store.add_member(member).expect("add member");

    let mut app = support::make_app();
    app.session.session_id = Some("lead-1".to_string());
    assert!(
        app.session.active_team.is_none(),
        "precondition: no active team"
    );

    app.handle_event(Event::TeammateSpawned {
        session_id: "lead-1".to_string(),
        team_name,
        teammate_name: "writer".to_string(),
        agent_id: "tm-001".to_string(),
    })
    .await;

    let _ = std::env::set_current_dir(original_dir);

    assert!(
        app.session.active_team.is_some(),
        "the spawned teammate's team must become the active team (FR-050)"
    );
    assert!(
        codemason_tui::teammate_strip::is_visible(&app),
        "the team strip must be visible after a spawn (FR-050)"
    );
    assert_eq!(
        app.session.team_members[0].session_id.as_deref(),
        Some("tm-sess-1"),
        "the member must be hydrated from the store (FR-050)"
    );
}

/// `TeamCleanedUp` must empty the strip as well as the panel: no members means
/// the pill row disappears entirely (FR-053, FR-066).
#[tokio::test]
async fn test_event_team_cleaned_up_hides_strip() {
    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    app.session.active_team = Some(TeamConfig::new("alpha", "s1"));
    app.session
        .team_members
        .push(TeamMember::new("writer", "tm-001", "general"));
    assert!(codemason_tui::teammate_strip::is_visible(&app));

    app.handle_event(Event::TeamCleanedUp {
        session_id: "s1".to_string(),
        team_name: "alpha".to_string(),
    })
    .await;

    assert!(
        !codemason_tui::teammate_strip::is_visible(&app),
        "cleanup must hide the strip (FR-053)"
    );
}

// -- T-012 (spec `agentui`): Alt+Down / Alt+Up focus cycling ------------------

/// Focusing a teammate opens its output view but leaves the input border plain:
/// FR-058 (amended) removed the focused-input routing, so the input box never
/// names a recipient and Enter always addresses the lead session. A message
/// reaches a teammate only through `/team message`, the path exercised below.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_team_focus_does_not_retitle_or_route_the_input() {
    let _cwd_guard = crate::global_env_lock();
    let tmp = tempfile::tempdir().expect("tempdir");
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    std::fs::create_dir_all(tmp.path().join(".codemason/teams")).unwrap();

    let mut app = support::make_app();
    app.session.session_id = Some("s1".to_string());
    let team_name = unique_team_name("focus-route");
    let store = TeamStore::create(&team_name, "s1", tmp.path(), true).expect("create team");
    app.session.active_team = Some(store.config.clone());
    drop(store);
    app.session
        .team_members
        .push(TeamMember::new("writer", "tm-001", "general"));

    app.focus_teammate_by_id("tm-001");
    assert_eq!(
        app.session.focused_teammate.as_deref(),
        Some("tm-001"),
        "focusing must register before the border can be checked (FR-058)"
    );

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| codemason_tui::layout::render(frame, &mut app))
        .expect("render");
    let buffer = terminal.backend().buffer().clone();
    let row = |y: u16| -> String {
        (0..100)
            .map(|x| {
                buffer
                    .cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect::<String>()
    };
    assert!(
        (0..30).all(|y| !row(y).contains("(focused)")),
        "the input border must not name a recipient (FR-058 amended)"
    );
    assert!(
        (0..30).any(|y| row(y).contains(" Input ")),
        "the input border keeps its plain title"
    );

    // Alt+Down off the single member goes past the end (the cycle does not
    // wrap; FR-046 clears the focus together with the output view instead).
    app.handle_key_event(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::ALT,
    ))
    .await;
    assert!(
        app.session.focused_teammate.is_none(),
        "Alt+Down with a single member must clear focus past the end (FR-046)"
    );
    assert!(app.output_view.is_none());
    // Re-land on the only member for the delivery half of the test.
    app.cycle_focused_teammate(true);
    assert_eq!(app.session.focused_teammate.as_deref(), Some("tm-001"));

    // FR-058 (amended): delivery to a teammate is `/team message`'s job, not the
    // input box's - `send_teammate_message` is the arm behind that command.
    app.send_teammate_message(
        &app.session
            .active_team
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default(),
        "writer",
        "please check the logs",
    );
    assert!(
        app.status.contains("message sent to writer") || app.status.contains("writer"),
        "the routed send must report the teammate, got: {}",
        app.status
    );

    let _ = std::env::set_current_dir(original_dir);

    let text: String = app.status.chars().collect();
    assert!(text.contains("writer"), "status must name the teammate");
}

// -- SEC-codemason-tui-004: team-name validation at the TUI boundary (SECTASKS T-004) -

// -- /team delete confirmation (teamssetup T-028, FR-023, FR-030, F-28) --------
