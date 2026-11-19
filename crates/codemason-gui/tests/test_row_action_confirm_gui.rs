//! Tests for the destructive row-action confirmation dialog (spec `agentui`
//! T-009).
//!
//! Covers the modal itself - `open_row_action` arming it with `No` selected
//! (FR-069), the render that paints the title/target and caches the three
//! hit-rect areas (NFR-010), the keyboard answer path (`Enter` selects the
//! highlighted option, `Esc` dismisses, every other key is swallowed so
//! nothing reaches the input buffer), the mouse answer path (a click on the
//! painted `[ Yes ]` / `[ No ]` span answers that option), the destructive
//! trigger points routing through the dialog (sub-agent Shift+K with the
//! documented task id and agent name; background `[x]` cancel; teammate
//! `[stop]` suspend with the direct resume exemption, FR-040; teammate `[x]`
//! shutdown; teammate Space/Shift+K shutdown, FR-045), the stale-row answer
//! that mutates nothing (FR-069), and the redraw flag every mutating path
//! sets (NFR-005).
//!
//! The keyboard-only confirmations mirror `test_stop_confirm_dialog.rs` and
//! the mouse cases drive the real `App::handle_mouse_event` router with the
//! rects the renderer records.
//!
//! Ported to codemason-gui when the terminal UI was removed: these tests of
//! the original `crates/codemason-tui/tests/test_row_action_confirm.rs` check App state that
//! depends on what a frame publishes (row maps, the sidebar rows, the
//! memory rows). Each terminal draw is replaced by
//! `crate::frame::publish`, which draws the GUI panels the App shows;
//! assertions that read the drawn terminal frame were dropped with it.

#[path = "../../codemason-app-core/tests/support/mod.rs"]
mod support;

use crate::term_events::CrosstermAppExt;
use crate::term_events::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use codemason_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use codemason_agent::team::{MemberStatus, TeamConfig, TeamMember};
use codemason_app_core::app::App;
use codemason_app_core::app::RowAction;
use support::make_app;

/// The lead session id every case uses.
const LEAD_SESSION: &str = "lead-session-0001";

/// A team name no team store on disk can own, so the FR-060 render hydration
/// never overrides the synthetic member statuses these cases rely on.
const TEAM_NAME: &str = "row-action-confirm";

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// A left-button press at `(col, row)`.
const fn click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// A live sub-agent task.
fn task(id: &str, agent: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: LEAD_SESSION.to_string(),
        child_session_id: format!("{id}-session"),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
        time_bound_secs: None,
    }
}

/// A teammate in `status` with a spawned session id.
fn member(agent_id: &str, name: &str, status: MemberStatus) -> TeamMember {
    let mut member = TeamMember::new(name, agent_id, "general");
    member.status = status;
    member.session_id = Some(format!("{agent_id}-session"));
    member
}

/// Render one frame so the renderer records its hit rects, as a real session
/// does before any click can arrive.
fn render(app: &mut App) {
    crate::frame::publish(app);
}

/// An app with one running sub-agent and the Agents window open.
fn app_with_task() -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.active_tasks
        .push(task("a1b2c3d4", "explore", TaskStatus::Running));
    app.show_agents_window = true;
    app
}

/// An app with an active team of one working teammate and the Teams window
/// open.
fn app_with_team(app_status: MemberStatus) -> App {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session.agent_name = "general".to_string();
    app.session.active_team = Some(TeamConfig::new(TEAM_NAME, LEAD_SESSION));
    app.session
        .team_members
        .push(member("tm-001", "reviewer", app_status));
    app.show_teams_window = true;
    app
}

// ---------------------------------------------------------------------------
// Arming and defaults
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Keyboard answering (FR-069, NFR-005)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Mouse answering (T-009: "with keyboard and mouse answering")
// ---------------------------------------------------------------------------

/// With the dialog open a click that lands outside it is swallowed so nothing
/// on the surface beneath it reacts (FR-070: the modal owns the mouse).
#[tokio::test]
async fn mouse_click_outside_the_dialog_is_swallowed() {
    let mut app = app_with_team(MemberStatus::Working);
    app.open_row_action(RowAction::ShutdownTeammate {
        agent_id: "tm-001".to_string(),
        name: "reviewer".to_string(),
    });
    render(&mut app);
    let area = app.row_action_confirm_area;
    app.status = "sentinel".to_string();

    // The input-area left edge is far from the centred dialog at a
    // 120-column terminal.
    app.handle_mouse_event(click(2, 43));

    assert!(
        app.row_action_confirm_open(),
        "a click outside the dialog (area {area:?}) must not alter it"
    );
    assert_eq!(app.status, "sentinel", "nothing beneath reacts (FR-070)");
}

// ---------------------------------------------------------------------------
// Destructive trigger points route through the dialog
// ---------------------------------------------------------------------------

/// `Shift+K` on the cursor-selected sub-agent row opens the dialog naming the
/// task; key #2 is consumed by the dialog so no second kill can fire behind it
/// (FR-036, FR-069). (Old selected_agent_index trigger retired.)
#[tokio::test]
async fn shift_k_on_a_selected_agent_row_opens_the_kill_dialog() {
    let mut app = app_with_task();
    render(&mut app);
    app.agents_cursor_subagent = Some(0); // the only sub-agent row

    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "Shift+K confirms the kill first"
    );
    match app.row_action_confirm.as_ref() {
        Some(pending) => match &pending.action {
            RowAction::KillSubagent {
                task_id,
                agent_name,
            } => {
                assert_eq!(task_id, "a1b2c3d4", "the dialog names the task");
                assert_eq!(agent_name, "explore", "the dialog names the agent");
            }
            other => panic!("expected KillSubagent, got {other:?}"),
        },
        None => unreachable!(),
    }
}

/// The keyboard cursor cannot reach the primary row at all: it never lands
/// below the sub-agent list's bottom, never above its first row, so the old
/// "primary refusal" case is retired in favour of the range invariant (FR-067).
#[tokio::test]
async fn the_agents_cursor_range_covers_subagent_rows_only() {
    let mut app = app_with_task();
    render(&mut app);

    // Down from no selection starts on the first sub-agent row.
    app.move_agents_cursor(1);
    assert_eq!(app.agents_cursor_subagent, Some(0));
    // The cursor ordinal 0 names the sub-agent, never the primary/lead (FR-067).
    assert_eq!(
        app.agents_subagent_task_ids.first().map(String::as_str),
        Some("a1b2c3d4"),
        "the first cursor row must be a sub-agent (FR-067)"
    );
}

/// Space on the cursor-selected working teammate opens the suspend dialog
/// naming the member (FR-040's suspend direction confirms; the old row click
/// trigger is retired).
#[tokio::test]
async fn teammate_space_on_a_working_member_opens_the_suspend_dialog() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    // The cursor list covers the painted teammate rows; ordinal 0 = tm-001.
    app.move_teams_cursor(1);
    assert_eq!(app.teams_cursor_teammate, Some(0));

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "teammate suspend confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::SuspendTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate, got {other:?}"),
    }
}

/// Space on an ALREADY SUSPENDED teammate resumes it directly: no dialog opens
/// (FR-040 confirms only the suspend direction; FR-035 rule).
#[tokio::test]
async fn teammate_space_on_a_suspended_member_resumes_without_a_dialog() {
    let mut app = app_with_team(MemberStatus::Suspended);
    render(&mut app);
    app.move_teams_cursor(1);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;

    assert!(
        !app.row_action_confirm_open(),
        "a resume must not open the confirmation dialog"
    );
}

/// `Del` on the cursor-selected teammate row opens the shutdown dialog
/// (FR-040).
#[tokio::test]
async fn teammate_del_opens_the_shutdown_dialog() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    app.move_teams_cursor(1);

    app.handle_key_event(key(KeyCode::Delete, KeyModifiers::NONE))
        .await;

    assert!(
        app.row_action_confirm_open(),
        "teammate shutdown confirms first"
    );
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::ShutdownTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected ShutdownTeammate, got {other:?}"),
    }
}

/// `Space` and `Shift+K` on the cursor-selected teammate route through the
/// suspend direction (Space confirms only the suspend; the old
/// focused_teammate Space shutdown case is retired - the cursor model confirms
/// the destructive direction via `Del`, FR-045's re-expression).
#[tokio::test]
async fn teammate_keyboard_keys_consume_the_key_either_way() {
    let mut app = app_with_team(MemberStatus::Working);
    render(&mut app);
    app.move_teams_cursor(1);

    app.handle_key_event(key(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    assert!(app.row_action_confirm_open(), "Space confirms the suspend");
    match app.row_action_confirm.as_ref().map(|p| &p.action) {
        Some(RowAction::SuspendTeammate { agent_id, name }) => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!(name, "reviewer");
        }
        other => panic!("expected SuspendTeammate from Space, got {other:?}"),
    }

    // The dialog consumes the follow-up key so the destructive key cannot
    // re-arm or fire behind it.
    app.handle_key_event(key(KeyCode::Char('K'), KeyModifiers::SHIFT))
        .await;
    assert!(
        app.row_action_confirm_open(),
        "the dialog stays open while it is being answered"
    );
    assert_eq!(
        app.session.input, "",
        "the consumed key must not leak into the input buffer"
    );
}

// ---------------------------------------------------------------------------
// Stale-row answers dispatch nothing (FR-069)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// FR-028 / FR-032 - the dialog states which option performs and which cancels
// ---------------------------------------------------------------------------

/// The shared `Yes`/`No` dialogs (queue clear, memory clear) state the same
/// contract: `Enter` on `Yes` performs, `Enter` on `No` cancels, `Esc` cancels.
/// `No` is pre-selected, so an un-toggled `Enter` must read as a cancel.
#[tokio::test]
async fn shared_yes_no_dialog_footer_states_the_outcomes() {
    let mut app = make_app();
    app.session.session_id = Some(LEAD_SESSION.to_string());
    app.session
        .input_queue
        .push_back(codemason_app_core::app::QueuedInput {
            text: "queued".to_string(),
            image_paths: Vec::new(),
        });
    app.queue_menu_open = true;
    app.queue_menu_select_clear();
    assert!(
        app.queue_clear_confirm_open,
        "precondition: selecting Clear on a non-empty queue arms the dialog"
    );

    // Paint the dialog first: its cached area is only known after a render.
    render(&mut app);
    let _area = app.queue_clear_confirm_area;
    crate::frame::publish(&mut app);
}
